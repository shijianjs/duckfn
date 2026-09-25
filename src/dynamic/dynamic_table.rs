//! 动态结果集：`DuckDynamicIterator` / `DuckDynamicTable`。
//!
//! Dynamic result table: `DuckDynamicIterator` / `DuckDynamicTable`.

use crate::DuckOptionResult;

use super::dynamic_row::DuckDynamicRow;
use super::result_schema::DuckResultSchema;

/// 动态行迭代器：与静态表函数的 `DuckFullIterator` 同形（可发送、逐行可错可为空）。
///
/// The dynamic-row iterator: the same shape as the static table functions' `DuckFullIterator`
/// (sendable, every row may fail or be NULL).
pub type DuckDynamicIterator = Box<dyn Iterator<Item = DuckOptionResult<DuckDynamicRow>> + Send>;

/// 动态结果集：bind 阶段算出的 schema + 行迭代器。
///
/// A dynamic result table: the schema computed during bind plus the row iterator.
///
/// 它就是用户 `bind` 函数的返回值（见
/// [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter)）：bind 负责「读外部
/// 元数据、给出 schema、造出迭代器」，scan 负责按 schema 批量写出。
///
/// This is what a user `bind` function returns (see
/// [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter)): bind reads the external
/// metadata, publishes the schema and builds the iterator; scan writes rows out according to that
/// schema.
pub struct DuckDynamicTable {
    /// 输出列定义。
    ///
    /// The output column definitions.
    schema: DuckResultSchema,
    /// 行迭代器。
    ///
    /// The row iterator.
    rows: DuckDynamicIterator,
}

impl DuckDynamicTable {
    /// 由 schema 与行迭代器构造结果集。
    ///
    /// Builds a result table from a schema and a row iterator.
    #[must_use]
    pub fn new(schema: DuckResultSchema, rows: DuckDynamicIterator) -> Self {
        Self { schema, rows }
    }

    /// 输出 schema。
    ///
    /// The output schema.
    #[must_use]
    pub fn schema(&self) -> &DuckResultSchema {
        &self.schema
    }

    /// 拆成 `(schema, 行迭代器)`，供适配层放进 scan 状态。
    ///
    /// Splits into `(schema, row iterator)` so the adapter can move them into the scan state.
    #[must_use]
    pub fn into_parts(self) -> (DuckResultSchema, DuckDynamicIterator) {
        (self.schema, self.rows)
    }
}

impl std::fmt::Debug for DuckDynamicTable {
    /// 只打印 schema —— 迭代器无法 Debug，也不该被打印。
    ///
    /// Prints the schema only — the iterator is not `Debug` and should not be printed anyway.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuckDynamicTable")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}
