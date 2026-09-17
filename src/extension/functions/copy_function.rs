use duckfn::{
    DataChunk, DuckCopyWriter, DuckResult, LogicalType, TypeId, duck_copy_function, duck_error,
};
use quack_rs::prelude::VectorReader;
use std::fs::File;
use std::io::{BufWriter, Write};

// ============================================================================
// duck_copy_function：为 COPY ... TO 提供自定义文件格式
//
//   COPY (SELECT ...) TO 'out.tsv' (FORMAT dfn_copy_tsv);
//
//   四个生命周期阶段（由 duckfn 适配层生成）分别对应：
//     bind        -> 从 COPY 的 bind info 读出输出列逻辑类型（存在 bind data 里）
//     global_init -> TsvWriter::open(path, columns)：建文件、记住列类型
//     sink        -> 被标注的函数：每个数据块调用一次，把 chunk 写成文本
//     finalize    -> TsvWriter::finish()：flush / 关闭
//
//   函数签名固定为「writer + chunk」两参（顺序可互换），返回 DuckResult<()>；
//   函数名 dfn_copy_tsv 就是 FORMAT 后面的格式名。
// ============================================================================

/// writer 状态：一个打开的输出文件 + 输出列的逻辑类型。
///
/// 逻辑类型在 bind 阶段被记下、在 `open` 里换成 [`TypeId`]（`TypeId` 是 `Copy`，
/// 读数据块时用来分派「这一列该按什么类型读」）。
pub struct TsvWriter {
    file: BufWriter<File>,
    /// 输出列的逻辑类型，顺序与数据块的列一致。
    column_types: Vec<TypeId>,
}

impl DuckCopyWriter for TsvWriter {
    /// 打开输出目标；`columns` 是 COPY 查询的输出列逻辑类型。
    fn open(path: &str, columns: &[LogicalType]) -> DuckResult<Self> {
        let file = File::create(path)
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot create {path}: {e}")))?;
        let column_types = columns
            .iter()
            // SAFETY: 本函数由 DuckDB 在 global init 阶段回调，运行时已就绪。
            .map(|column| unsafe { column.get_type_id() })
            .collect();
        Ok(Self {
            file: BufWriter::new(file),
            column_types,
        })
    }

    /// flush 缓冲区（错误不会被 `Drop` 吞掉）。
    fn finish(&mut self) -> DuckResult<()> {
        self.file
            .flush()
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot flush: {e}")))
    }
}

/// 把 `SELECT` 的结果写成「制表符分隔、每行一条记录」的文本；NULL 写作 `\N`。
///
/// 每行格式：`列1 \t 列2 \t ... \t 列N \n`，既没有表头也不带引号，因此可以直接
/// `read_csv(..., delim = '\t', header = false, nullstr = '\N')` 读回来。
///
/// ```sql
/// COPY (SELECT 1 AS i, 'a' AS s) TO 'out.tsv' (FORMAT dfn_copy_tsv);
/// ```
#[duck_copy_function]
fn dfn_copy_tsv(writer: &mut TsvWriter, chunk: &DataChunk) -> DuckResult<()> {
    let rows = chunk.size();
    let columns = chunk.column_count();
    // 每列建一次 reader；下面按行按列读取。
    let readers: Vec<VectorReader> = (0..columns)
        // SAFETY: col < column_count()，且 chunk 在本次回调期间有效。
        .map(|col| unsafe { chunk.reader(col) })
        .collect();
    // 先拷一份列类型，避免读 writer.file 时与 writer 的可变借用冲突。
    let column_types = writer.column_types.clone();

    for row in 0..rows {
        for (col, reader) in readers.iter().enumerate() {
            if col > 0 {
                write_bytes(writer, b"\t")?;
            }
            let type_id = column_types.get(col).copied().ok_or_else(|| {
                duck_error(format!(
                    "dfn_copy_tsv: chunk has more columns ({columns}) than the query declared ({})",
                    column_types.len()
                ))
            })?;
            match format_cell(reader, row, type_id)? {
                Some(text) => write_bytes(writer, text.as_bytes())?,
                // NULL：写成 read_csv 能识别的空标记。
                None => write_bytes(writer, b"\\N")?,
            }
        }
        write_bytes(writer, b"\n")?;
    }
    Ok(())
}

/// 按列逻辑类型把一个单元格读成文本；`None` 表示该值为 NULL。
///
/// 这里只覆盖常见标量类型；遇到 LIST / STRUCT / MAP 等复杂类型会报错，让整条 `COPY` 失败，
/// 而不是写出一份不可读的文件。
fn format_cell(reader: &VectorReader, row: usize, type_id: TypeId) -> DuckResult<Option<String>> {
    // SAFETY: row < chunk.size()，reader 来自本次回调的 chunk。
    if !unsafe { reader.is_valid(row) } {
        return Ok(None);
    }
    // SAFETY: 下面每个 read_* 都要求「列类型与该方法一致」且值非 NULL，两者都由
    // type_id 分派与上面的 is_valid 检查保证。
    let text = match type_id {
        TypeId::Boolean => unsafe { reader.read_bool(row) }.to_string(),
        TypeId::TinyInt => unsafe { reader.read_i8(row) }.to_string(),
        TypeId::SmallInt => unsafe { reader.read_i16(row) }.to_string(),
        TypeId::Integer => unsafe { reader.read_i32(row) }.to_string(),
        TypeId::BigInt => unsafe { reader.read_i64(row) }.to_string(),
        TypeId::UTinyInt => unsafe { reader.read_u8(row) }.to_string(),
        TypeId::USmallInt => unsafe { reader.read_u16(row) }.to_string(),
        TypeId::UInteger => unsafe { reader.read_u32(row) }.to_string(),
        TypeId::UBigInt => unsafe { reader.read_u64(row) }.to_string(),
        TypeId::HugeInt => unsafe { reader.read_i128(row) }.to_string(),
        TypeId::UHugeInt => unsafe { reader.read_u128(row) }.to_string(),
        TypeId::Float => unsafe { reader.read_f32(row) }.to_string(),
        TypeId::Double => unsafe { reader.read_f64(row) }.to_string(),
        TypeId::Varchar => escape(unsafe { reader.read_str(row) }),
        other => {
            return Err(duck_error(format!(
                "dfn_copy_tsv: unsupported column type: {}",
                other.sql_name()
            )));
        }
    };
    Ok(Some(text))
}

/// 把字符串里的反斜杠、制表符与换行转义掉，避免破坏「一行一条记录」的格式。
fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// 往输出文件写一段字节，失败时带上格式名报错。
fn write_bytes(writer: &mut TsvWriter, bytes: &[u8]) -> DuckResult<()> {
    writer
        .file
        .write_all(bytes)
        .map_err(|e| duck_error(format!("dfn_copy_tsv: write failed: {e}")))
}
