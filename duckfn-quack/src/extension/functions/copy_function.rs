// 示例：为 COPY ... TO 提供自定义 TSV 格式（动态列版本）。
//
// Example: a custom TSV format for `COPY ... TO` (the dynamic-column flavour).
//
//   四个生命周期阶段（由 duckfn 适配层驱动）分别对应：
//     bind        -> 把查询结果各列的类型反推成动态 schema，并读出 COPY 选项
//     global_init -> TsvWriter::open(path, schema, options)：建文件（需要时写表头）
//     sink        -> 被标注的函数：每个数据块调用一次，按 schema 把动态行写成文本
//     finalize    -> TsvWriter::finish()：flush / 关闭
//
//   与旧版（只支持标量）的关键区别：这里的每一行都是 `DuckDynamicRow`，`LIST` / `STRUCT` / `MAP`
//   与 NULL 都在其中，写出逻辑不需要（也无法）针对编译期已知的列类型。
//
//   函数签名固定为「writer + 行批」两参，返回 DuckResult<()>；
//   函数名 dfn_copy_tsv 就是 FORMAT 后面的格式名。
//
// The four life-cycle phases (driven by duckfn's adapter): bind reconstructs every result column's
// type into a dynamic schema and reads the COPY options; global init calls
// `TsvWriter::open(path, schema, options)` (creating the file, header and all); sink runs once per
// data chunk and writes the dynamic rows as text; finalize flushes and closes. Unlike the scalar-only
// previous version, every row here is a `DuckDynamicRow` carrying `LIST` / `STRUCT` / `MAP` and NULL,
// so the writing logic neither needs nor can rely on compile-time column types.

use super::tsv_format;
use duckfn::{
    DuckCopyOptions, DuckCopyToWriter, DuckDynamicRow, DuckResult, DuckResultSchema,
    duck_copy_function, duck_error,
};
use std::fs::File;
use std::io::{BufWriter, Write};

/// writer 状态：一个打开的输出文件 + bind 阶段定下的动态 schema。
///
/// The writer state: an open output file plus the dynamic schema fixed during bind.
///
/// schema 用来把每列渲染成正确的文本（`LIST` / `STRUCT` / `MAP` 的嵌套形状由它决定），也是写表头
/// 时的列名来源。注意 COPY 的输出列本身没有名字，适配层给的是合成的 `column_0`、`column_1`…
///
/// The schema renders each column into the right text (it decides the nested shape of
/// `LIST` / `STRUCT` / `MAP`) and supplies the column names for a header. Note that COPY output
/// columns are unnamed, so the adapter synthesises `column_0`, `column_1`, ...
pub struct TsvWriter {
    file: BufWriter<File>,
    schema: DuckResultSchema,
}

impl DuckCopyToWriter for TsvWriter {
    /// 打开输出目标；`options` 里可以读到 `COPY ... TO (...)` 的选项。
    ///
    /// ```sql
    /// COPY (SELECT 1 AS i) TO 'out.tsv' (FORMAT dfn_copy_tsv, HEADER true);
    /// ```
    ///
    /// Opens the output target; `options` carries the `COPY ... TO (...)` options.
    fn open(path: &str, schema: &DuckResultSchema, options: &DuckCopyOptions) -> DuckResult<Self> {
        let file = File::create(path)
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot create {path}: {e}")))?;
        let mut file = BufWriter::new(file);

        // `HEADER true`：第一行写列名。只做一次的事情放在 open 里最合适。
        //
        // `HEADER true`: write the column names as the first line. One-off work belongs in open.
        if options.get_bool("header").unwrap_or(false) {
            let names: Vec<&str> = schema
                .columns()
                .iter()
                .map(|(name, _)| name.as_str())
                .collect();
            let line = format!("{}\n", names.join(&tsv_format::DELIMITER.to_string()));
            file.write_all(line.as_bytes())
                .map_err(|e| duck_error(format!("dfn_copy_tsv: header write failed: {e}")))?;
        }

        Ok(Self {
            file,
            // schema 是 `Send` 友好的描述，可以直接留着跨数据块复用。
            //
            // The schema is a `Send`-friendly description, so it is kept and reused across chunks.
            schema: schema.clone(),
        })
    }

    /// flush 缓冲区（错误不会被 `Drop` 吞掉）。
    ///
    /// Flushes the buffer (an error is not swallowed by `Drop`).
    fn finish(&mut self) -> DuckResult<()> {
        self.file
            .flush()
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot flush: {e}")))
    }
}

/// 把一个数据块的动态行写成一 Tab 分隔的文本；NULL 写作 `\N`。
///
/// 每行格式：`列1 \t 列2 \t ... \t 列N \n`；单元格文本来自
/// [`DuckDynamicValue::to_text`](duckfn::DuckDynamicValue::to_text)，再按 TSV 规则转义，因此可以直接
/// 用 `read_csv(..., delim = '\t', header = false, nullstr = '\N')` 读回标量列，也可以用
/// [`super::copy_from_function`] 的同名读取器把嵌套列原样读回来。
///
/// ```sql
/// COPY (SELECT 1 AS i, 'a' AS s) TO 'out.tsv' (FORMAT dfn_copy_tsv);
/// COPY (SELECT [1, 2] AS l, {'k': 3} AS st, MAP {'m': 4} AS mp) TO 'out.tsv' (FORMAT dfn_copy_tsv);
/// ```
///
/// Writes one chunk's dynamic rows as tab-separated text with NULL as `\N`. Each line is
/// `col1 \t col2 \t ... \n`; the cell text comes from `DuckDynamicValue::to_text`, escaped for TSV,
/// so scalar columns read back with `read_csv(..., delim = '\t', header = false, nullstr = '\N')` and
/// nested columns read back through the reader in [`super::copy_from_function`].
#[duck_copy_function]
fn dfn_copy_tsv(writer: &mut TsvWriter, rows: &[DuckDynamicRow]) -> DuckResult<()> {
    let columns = writer.schema.columns();
    for row in rows {
        let mut line = String::new();
        for (index, (_, desc)) in columns.iter().enumerate() {
            if index > 0 {
                line.push(tsv_format::DELIMITER);
            }
            let cell = row.values().get(index).and_then(Option::as_ref);
            line.push_str(&tsv_format::format_cell(cell, desc));
        }
        line.push('\n');
        writer
            .file
            .write_all(line.as_bytes())
            .map_err(|e| duck_error(format!("dfn_copy_tsv: write failed: {e}")))?;
    }
    Ok(())
}
