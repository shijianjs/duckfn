// 示例：为 COPY ... FROM 提供自定义 TSV 读取格式（与 copy_function.rs 里的 dfn_copy_tsv 成对）。
//
// Example: a custom TSV reading format for `COPY ... FROM` (the counterpart of `dfn_copy_tsv` in
// `copy_function.rs`).
//
//   三个回调（由 duckfn 适配层驱动）分别对应：
//     bind  -> 解析参数（路径 + SKIP_ROWS 等命名选项）、读**目标表 schema**，建 TsvReader
//     scan  -> 被标注的函数：每次取一批动态行，适配层把它们写进目标表
//     （表函数没有 finalize 回调；`DuckCopyFromReader::finish` 在查询结束时被调用）
//
//   两点与 COPY TO 不同、必须注意：
//     * reader **不能**声明结果列 —— schema 已由目标表固定，只能读；
//     * 位置参数必须**恰好一个**（文件路径），其余的 COPY 选项都是命名参数。
//
// The three callbacks (driven by duckfn's adapter): bind parses the arguments (the path plus named
// options such as `SKIP_ROWS`) and reads the **target table's schema** to build a `TsvReader`; scan
// runs the annotated function to take one batch of dynamic rows, which the adapter writes into the
// target table. (A table function has no finalize callback; `DuckCopyFromReader::finish` runs when
// the query ends.) Two things differ from COPY TO and must be kept in mind: the reader **must not**
// declare result columns (the schema is fixed by the target table and can only be read), and the
// positional parameters must be **exactly one** (the file path) with every COPY option arriving as a
// named parameter.

use super::tsv_format;
use duckfn::{
    DuckCopyFromReader, DuckDynamicRow, DuckResult, DuckResultSchema, DuckStruct,
    duck_copy_from_function, duck_error,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};

/// bind 参数：位置参数 0 是文件路径，`skip_rows` 起是命名 COPY 选项。
///
/// The bind arguments: positional parameter 0 is the file path, and `skip_rows` (onwards) are named
/// COPY options.
///
/// 字段名就是选项名（大小写不敏感），类型就是选项的类型 —— DuckDB 通过我们自己声明的参数表来解析
/// `COPY ... FROM 'f' (FORMAT dfn_copy_tsv_from, SKIP_ROWS 1)`。
///
/// The field name is the option name (case-insensitively) and the field type is the option's type:
/// DuckDB parses `COPY ... FROM 'f' (FORMAT dfn_copy_tsv_from, SKIP_ROWS 1)` through the parameter
/// list we declare.
#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "skip_rows")]
pub struct TsvFromArgs {
    /// 位置参数 0：要读的文件路径（COPY FROM 要求恰好一个位置参数）。
    ///
    /// Positional parameter 0: the file to read (COPY FROM requires exactly one positional
    /// parameter).
    pub path: String,
    /// 命名 COPY 选项 `SKIP_ROWS`：跳过文件开头的若干行（例如 `HEADER true` 写出的表头）。
    ///
    /// The named COPY option `SKIP_ROWS`: the number of leading lines to skip (a header written by
    /// `HEADER true`, for instance).
    pub skip_rows: Option<i64>,
}

/// 读取器状态：按行读的文件 + 目标表 schema（用来逐列解析）。
///
/// The reader state: a line reader plus the target schema (used to parse column by column).
pub struct TsvReader {
    lines: Lines<BufReader<File>>,
    schema: DuckResultSchema,
}

impl DuckCopyFromReader for TsvReader {
    type Args = TsvFromArgs;

    /// bind 阶段：打开文件、按 `SKIP_ROWS` 跳过开头，并记住目标表 schema。
    ///
    /// `schema` 是目标表的列名与类型（含 `LIST` / `STRUCT` / `MAP`），由适配层从
    /// `duckdb_table_function_bind_get_result_column_*` 读出 —— reader 不声明结果列。
    ///
    /// Bind phase: opens the file, skips the leading `SKIP_ROWS` lines and remembers the target
    /// schema. `schema` holds the target table's column names and types (nested ones included), read
    /// by the adapter from `duckdb_table_function_bind_get_result_column_*` — the reader declares no
    /// result columns.
    ///
    /// ```sql
    /// CREATE TABLE t(i BIGINT, l BIGINT[], st STRUCT(k BIGINT), mp MAP(VARCHAR, BIGINT));
    /// COPY t FROM 'out.tsv' (FORMAT dfn_copy_tsv_from);
    /// COPY t FROM 'out.tsv' (FORMAT dfn_copy_tsv_from, SKIP_ROWS 1);
    /// ```
    fn open(args: Self::Args, schema: &DuckResultSchema) -> DuckResult<Self> {
        let file = File::open(&args.path).map_err(|e| {
            duck_error(format!("dfn_copy_tsv_from: cannot open {}: {e}", args.path))
        })?;
        let mut lines = BufReader::new(file).lines();
        for _ in 0..args.skip_rows.unwrap_or(0).max(0) {
            lines.next();
        }
        Ok(Self {
            lines,
            schema: schema.clone(),
        })
    }
}

/// 取下一批行：每行按制表符切成单元格，再按目标表 schema 逐列解析；空批表示文件读完。
///
/// 行的列数或某列的值与目标表不符时会返回错误，整条 `COPY` 失败 —— 而不是写进一张错位的表。
///
/// Takes the next batch: each line is split on tabs and parsed column by column against the target
/// schema; an empty batch means the file is exhausted. A line whose width or types disagree with the
/// target table reports an error and fails the whole `COPY` instead of writing a misaligned row.
#[duck_copy_from_function]
fn dfn_copy_tsv_from(reader: &mut TsvReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> {
    let columns = reader.schema.columns();
    let mut rows = Vec::new();
    while rows.len() < limit {
        let Some(line) = reader.lines.next() else {
            break;
        };
        let line = line.map_err(|e| duck_error(format!("dfn_copy_tsv_from: read failed: {e}")))?;
        // 空行直接跳过（写入端不会产生空行，这里只是容错）。
        //
        // Blank lines are skipped (the writer never produces one; this is tolerance).
        if line.is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split(tsv_format::DELIMITER).collect();
        if cells.len() != columns.len() {
            return Err(duck_error(format!(
                "dfn_copy_tsv_from: line has {} columns but the target table has {}",
                cells.len(),
                columns.len()
            )));
        }
        let mut values = Vec::with_capacity(cells.len());
        for (cell, (_, desc)) in cells.iter().zip(columns) {
            values.push(tsv_format::parse_cell(cell, desc)?);
        }
        rows.push(DuckDynamicRow::new(values));
    }
    Ok(rows)
}
