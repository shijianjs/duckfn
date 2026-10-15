//! 动态查询结果：`DuckQueryResult`。
//!
//! The dynamic query result: `DuckQueryResult`.

use crate::{
    DuckDynamicRow, DuckDynamicTable, DuckDynamicValue, DuckOptionResult, DuckResult,
    DuckResultSchema, DuckTypeDesc, duck_error,
};
use quack_rs::query::QueryResult;

use super::query_error;

/// 一条查询的完整结果：bind 阶段定下的 schema + 已经读进内存的动态行。
///
/// The full result of one query: the schema pinned down from the result's own column types, plus the
/// dynamic rows already read into memory.
///
/// 它不是迭代器：`query` 会把每个数据块立刻读成 [`DuckDynamicRow`]，因为 `duckdb_result` 的句柄
/// 不能带出那条自有连接（见模块文档的「代价与限制」）。想交给表函数时用 [`Self::into_table`]，
/// 表函数自己会按 chunk 批量写出。
///
/// It is not an iterator: `query` reads every chunk into [`DuckDynamicRow`] straight away, because a
/// `duckdb_result` handle cannot leave the owned connection (see "Cost and limits" in the module
/// docs). Hand it to a table function with [`Self::into_table`]; the adapter then writes it out chunk
/// by chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct DuckQueryResult {
    /// 列定义（顺序即输出列顺序）。
    ///
    /// The column definitions (the order is the output column order).
    schema: DuckResultSchema,
    /// 已经物化的行。
    ///
    /// The materialised rows.
    rows: Vec<DuckDynamicRow>,
}

impl DuckQueryResult {
    /// 由 schema 与行构造结果（自己算出的两半也可以用同一个类型装）。
    ///
    /// Builds a result from a schema and rows (a pair you worked out yourself fits this type too).
    #[must_use]
    pub fn new(schema: DuckResultSchema, rows: Vec<DuckDynamicRow>) -> Self {
        Self { schema, rows }
    }

    /// 读取一个 DuckDB 查询结果：先按列逻辑类型定出 schema，再把所有块读成动态行。
    ///
    /// Reads a DuckDB query result: the schema comes from the columns' logical types, then every
    /// chunk is read into dynamic rows.
    ///
    /// 列名缺失或不是 UTF-8 时退化成 `column1` / `column2`...（不会因此失败）；某列的类型无法用
    /// [`DuckTypeDesc`] 表达时返回错误，并把列名写进消息。
    ///
    /// A missing or non-UTF-8 column name degrades to `column1` / `column2` ... (never a failure);
    /// a column whose type cannot be expressed as a [`DuckTypeDesc`] is an error naming the column.
    ///
    /// # Errors
    ///
    /// 取列的逻辑类型失败、某列类型无法表达、或读取某个数据块失败时返回错误。
    ///
    /// Returns an error when a column's logical type is unavailable, a column's type cannot be
    /// expressed, or reading a chunk fails.
    pub(crate) fn read(mut result: QueryResult) -> DuckResult<Self> {
        let schema = schema_of(&result)?;
        let mut rows = Vec::new();
        while let Some(chunk) = result
            .next_chunk()
            .map_err(|error| query_error("query", error))?
        {
            rows.extend(DuckDynamicRow::read_batch(&chunk, &schema)?);
        }
        Ok(Self { schema, rows })
    }

    /// 输出 schema（列名与列类型）。
    ///
    /// The output schema (column names and types).
    #[must_use]
    pub fn schema(&self) -> &DuckResultSchema {
        &self.schema
    }

    /// 所有行。
    ///
    /// Every row.
    #[must_use]
    pub fn rows(&self) -> &[DuckDynamicRow] {
        &self.rows
    }

    /// 行数。
    ///
    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// 是否一行都没有（schema 仍可能是非空的）。
    ///
    /// Whether there is no row at all (the schema may still be non-empty).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 列数。
    ///
    /// The number of columns.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.schema.len()
    }

    /// 所有列名，顺序即输出列顺序。
    ///
    /// Every column name, in output order.
    pub fn column_names(&self) -> impl Iterator<Item = &str> {
        self.schema.columns().iter().map(|(name, _)| name.as_str())
    }

    /// 按列名找列下标（大小写敏感，与 DuckDB 的输出列名一致）。
    ///
    /// Finds a column index by name (case-sensitive, matching DuckDB's output column names).
    #[must_use]
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.schema
            .columns()
            .iter()
            .position(|(column, _)| column == name)
    }

    /// 取第 `index` 行；越界返回 `None`。
    ///
    /// Returns row `index`; `None` when it is out of range.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&DuckDynamicRow> {
        self.rows.get(index)
    }

    /// 按**列名**取单元格；列名不存在或行越界时返回错误。
    ///
    /// Reads a cell **by column name**; an unknown column or an out-of-range row is an error.
    ///
    /// 返回 `Ok(None)` 表示这个单元格是 SQL NULL —— 与「列名写错了」区分开，后者是 `Err`。
    ///
    /// `Ok(None)` means the cell is SQL NULL, which is kept apart from "the column name is wrong"
    /// (that is an `Err`).
    ///
    /// # Errors
    ///
    /// 结果里没有这一列、或行下标越界时返回错误（消息里带上可选的列名 / 行数）。
    ///
    /// Returns an error when the result has no such column, or the row index is out of range (the
    /// message lists the available columns / the row count).
    pub fn cell(&self, row: usize, name: &str) -> DuckResult<Option<&DuckDynamicValue>> {
        let column = self.column_index(name).ok_or_else(|| {
            duck_error(format!(
                "duckfn::sql: the result has no column `{name}`; it has: {}",
                self.column_list()
            ))
        })?;
        let target = self.rows.get(row).ok_or_else(|| {
            duck_error(format!(
                "duckfn::sql: row {row} is out of range; the result has {} row(s)",
                self.rows.len()
            ))
        })?;
        Ok(target.value(column))
    }

    /// 取唯一一行；行数不是 1 时返回错误。
    ///
    /// Returns the only row; an error when the row count is not 1.
    ///
    /// 单体查询（`SELECT count(*)`、`SELECT max(x)`）的常用形态。
    ///
    /// The usual shape for a single-row query (`SELECT count(*)`, `SELECT max(x)`).
    ///
    /// # Errors
    ///
    /// 行数不是 1 时返回错误（消息带上实际行数）。
    ///
    /// Returns an error when the row count is not 1 (the message carries the actual count).
    pub fn single_row(&self) -> DuckResult<&DuckDynamicRow> {
        match self.rows.as_slice() {
            [row] => Ok(row),
            other => Err(duck_error(format!(
                "duckfn::sql: single_row() needs exactly one row, but the result has {}",
                other.len()
            ))),
        }
    }

    /// 取唯一一行唯一一列的值；`Ok(None)` 表示它是 SQL NULL。
    ///
    /// Returns the value of the only row and column; `Ok(None)` means it is SQL NULL.
    ///
    /// # Errors
    ///
    /// 行数不是 1、或列数不是 1 时返回错误。
    ///
    /// Returns an error when the row count is not 1 or the column count is not 1.
    pub fn single_value(&self) -> DuckResult<Option<DuckDynamicValue>> {
        let row = self.single_row()?;
        if self.schema.len() != 1 {
            return Err(duck_error(format!(
                "duckfn::sql: single_value() needs exactly one column, but the result has {} ({}); \
                 use cell() or rows() instead",
                self.schema.len(),
                self.column_list()
            )));
        }
        Ok(row.value(0).cloned())
    }

    /// 取唯一一行唯一一列的**文本**（[`DuckDynamicValue::to_text`]，用该列自己的类型渲染）。
    ///
    /// Returns the **text** of the only row and column ([`DuckDynamicValue::to_text`], rendered with
    /// that column's own type).
    ///
    /// 适合把 `SELECT ...` 的结果直接当字符串回给 SQL（标量函数只能返回固定 Rust 类型，文本是最
    /// 通用的一种）。它不是无歧义的往返编码，要结构化取值请用 [`Self::single_value`]。
    ///
    /// Handy for handing a `SELECT ...` result back to SQL as a string (a scalar function can only
    /// return a fixed Rust type, and text is the most general one). It is not an unambiguous
    /// round-trip encoding — use [`Self::single_value`] for a structured read.
    ///
    /// # Errors
    ///
    /// 同 [`Self::single_value`]。
    ///
    /// Same as [`Self::single_value`].
    pub fn single_value_text(&self) -> DuckResult<Option<String>> {
        let Some(value) = self.single_value()? else {
            return Ok(None);
        };
        let (_, desc) = self
            .schema
            .column(0)
            .ok_or_else(|| duck_error("duckfn::sql: single_value_text() lost its only column"))?;
        Ok(Some(value.to_text(desc)))
    }

    /// 拆成 `(schema, 行)`，供调用方自己保管两半。
    ///
    /// Splits into `(schema, rows)` for callers that want to keep the two halves.
    #[must_use]
    pub fn into_parts(self) -> (DuckResultSchema, Vec<DuckDynamicRow>) {
        (self.schema, self.rows)
    }

    /// 只取行。
    ///
    /// Takes just the rows.
    #[must_use]
    pub fn into_rows(self) -> Vec<DuckDynamicRow> {
        self.rows
    }

    /// 转成动态表（schema + 行迭代器），可直接交给
    /// [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter) 的 `bind`。
    ///
    /// Converts into a dynamic table (schema plus a row iterator), ready to be returned from a
    /// [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter)'s `bind`.
    #[must_use]
    pub fn into_table(self) -> DuckDynamicTable {
        let rows: Vec<DuckOptionResult<DuckDynamicRow>> =
            self.rows.into_iter().map(|row| Ok(Some(row))).collect();
        DuckDynamicTable::new(self.schema, Box::new(rows.into_iter()))
    }

    /// 列名列表的渲染（错误信息用）。
    ///
    /// The rendered list of column names (for error messages).
    fn column_list(&self) -> String {
        if self.schema.is_empty() {
            return String::from("no columns");
        }
        self.column_names()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// 从查询结果读出动态 schema：逐列取名字与逻辑类型，再反推 [`DuckTypeDesc`]。
///
/// Reads the dynamic schema off a query result: name and logical type per column, then back to a
/// [`DuckTypeDesc`].
///
/// # Errors
///
/// 某列取不到逻辑类型、或该类型无法用 [`DuckTypeDesc`] 表达时返回错误。
///
/// Returns an error when a column's logical type is unavailable or cannot be expressed as a
/// [`DuckTypeDesc`].
fn schema_of(result: &QueryResult) -> DuckResult<DuckResultSchema> {
    let count = result.column_count();
    let mut columns = Vec::with_capacity(count);
    for index in 0..count {
        // 列名缺失或不是 UTF-8 时退化成一个稳定可读的名字，而不是让整条查询失败。
        //
        // A missing or non-UTF-8 name degrades to a stable readable one rather than failing the
        // whole query.
        let name = result
            .column_name(index)
            .unwrap_or_else(|| format!("column{}", index + 1));
        let logical_type = result.column_logical_type(index).ok_or_else(|| {
            duck_error(format!(
                "duckfn::sql: the result has no logical type for column {index} (`{name}`)"
            ))
        })?;
        let desc = DuckTypeDesc::from_logical_type(&logical_type).map_err(|error| {
            duck_error(format!(
                "duckfn::sql: column {index} (`{name}`) cannot be turned into a dynamic column: {}",
                error.as_str()
            ))
        })?;
        columns.push((name, desc));
    }
    Ok(DuckResultSchema::new(columns))
}

#[cfg(test)]
mod tests {
    use super::DuckQueryResult;
    use crate::{DuckDynamicRow, DuckDynamicValue, DuckResultSchema, DuckTypeDesc};
    use quack_rs::prelude::TypeId;

    // 只测纯逻辑：`read` 要 DuckDB 运行时（见 `test/sql/functions/sql_exec.test`），
    // 单测自己拼 schema + 行来覆盖结果处理那半边。
    //
    // Pure logic only: `read` needs the DuckDB runtime (see `test/sql/functions/sql_exec.test`);
    // these tests build the schema and rows by hand to cover the result-handling half.

    fn sample() -> DuckQueryResult {
        let schema = DuckResultSchema::new(vec![
            ("id".to_string(), DuckTypeDesc::scalar(TypeId::BigInt)),
            ("name".to_string(), DuckTypeDesc::scalar(TypeId::Varchar)),
        ]);
        DuckQueryResult::new(
            schema,
            vec![
                DuckDynamicRow::new(vec![
                    Some(DuckDynamicValue::BigInt(1)),
                    Some(DuckDynamicValue::Varchar("a".to_string())),
                ]),
                DuckDynamicRow::new(vec![Some(DuckDynamicValue::BigInt(2)), None]),
            ],
        )
    }

    #[test]
    fn cells_are_readable_by_column_name_and_null_is_not_missing() {
        let result = sample();
        assert_eq!(result.len(), 2);
        assert_eq!(result.column_count(), 2);
        assert_eq!(result.column_names().collect::<Vec<_>>(), ["id", "name"]);
        assert_eq!(result.column_index("name"), Some(1));
        assert_eq!(result.column_index("nope"), None);

        let first = result.cell(0, "name").expect("column exists");
        assert_eq!(first, Some(&DuckDynamicValue::Varchar("a".to_string())));
        // 第二个 `name` 是 SQL NULL：`Ok(None)`，与「列名不存在」的 `Err` 不同。
        //
        // The second `name` is SQL NULL: `Ok(None)`, which is not the `Err` of a missing column.
        assert_eq!(result.cell(1, "name").expect("column exists"), None);

        let missing = result.cell(0, "nope").expect_err("no such column");
        assert!(missing.as_str().contains("no column `nope`"), "{missing}");
        assert!(missing.as_str().contains("`id`"), "{missing}");

        let out_of_range = result.cell(2, "id").expect_err("row out of range");
        assert!(out_of_range.as_str().contains("2 row(s)"), "{out_of_range}");
    }

    #[test]
    fn single_row_needs_exactly_one_row() {
        let result = sample();
        let error = result.single_row().expect_err("two rows");
        assert!(error.as_str().contains("exactly one row"), "{error}");

        let one = DuckQueryResult::new(
            DuckResultSchema::from_scalar_types([("n", TypeId::BigInt)]),
            vec![DuckDynamicRow::new(vec![Some(DuckDynamicValue::BigInt(7))])],
        );
        assert_eq!(
            one.single_value().expect("one row, one column"),
            Some(DuckDynamicValue::BigInt(7))
        );
        assert_eq!(
            one.single_value_text().expect("one row, one column"),
            Some(String::from("7"))
        );
    }

    #[test]
    fn single_value_rejects_more_than_one_column() {
        // 单行、两列：行数没问题，卡在列数上。
        //
        // One row, two columns: the row count is fine, so the column count is what refuses.
        let result = DuckQueryResult::new(
            DuckResultSchema::from_scalar_types([("a", TypeId::BigInt), ("b", TypeId::BigInt)]),
            vec![DuckDynamicRow::new(vec![
                Some(DuckDynamicValue::BigInt(1)),
                Some(DuckDynamicValue::BigInt(2)),
            ])],
        );
        let error = result
            .single_value()
            .expect_err("two columns must be refused");
        assert!(error.as_str().contains("exactly one column"), "{error}");
        assert!(error.as_str().contains("`b`"), "{error}");
        assert!(result.single_value_text().is_err());
    }

    #[test]
    fn single_value_of_null_is_none() {
        let result = DuckQueryResult::new(
            DuckResultSchema::from_scalar_types([("n", TypeId::BigInt)]),
            vec![DuckDynamicRow::new(vec![None])],
        );
        assert_eq!(result.single_value().expect("one row"), None);
        assert_eq!(result.single_value_text().expect("one row"), None);
    }

    #[test]
    fn empty_result_keeps_its_schema() {
        let result = DuckQueryResult::new(
            DuckResultSchema::from_scalar_types([("n", TypeId::BigInt)]),
            Vec::new(),
        );
        assert!(result.is_empty());
        assert_eq!(result.column_count(), 1);
        assert!(result.column_names().eq(["n"]));
        let (schema, rows) = result.into_parts();
        assert_eq!(schema.len(), 1);
        assert!(rows.is_empty());
    }
}
