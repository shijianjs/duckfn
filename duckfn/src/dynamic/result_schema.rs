//! 动态结果集 schema：`DuckResultSchema`。
//!
//! Dynamic result schema: `DuckResultSchema`.

use quack_rs::prelude::{BindInfo, LogicalType, TypeId};

use super::type_desc::DuckTypeDesc;

/// 动态结果集 schema：有序的 `(列名, 列类型)` 列表。
///
/// A dynamic result schema: an ordered list of `(column name, column type)` pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuckResultSchema {
    /// 列定义，顺序即输出列顺序。
    ///
    /// The column definitions; the order is the output column order.
    columns: Vec<(String, DuckTypeDesc)>,
}

impl DuckResultSchema {
    /// 由 `(列名, 列类型)` 列表构造 schema。
    ///
    /// Builds a schema from a list of `(column name, column type)` pairs.
    #[must_use]
    pub fn new(columns: Vec<(String, DuckTypeDesc)>) -> Self {
        Self { columns }
    }

    /// 由 `(&str, TypeId)` 列表构造「全标量列」schema 的便捷方法。
    ///
    /// A convenience constructor for an all-scalar schema, from a list of `(&str, TypeId)` pairs.
    #[must_use]
    pub fn from_scalar_types<'a, I: IntoIterator<Item = (&'a str, TypeId)>>(columns: I) -> Self {
        Self {
            columns: columns
                .into_iter()
                .map(|(name, type_id)| (name.to_owned(), DuckTypeDesc::Scalar(type_id)))
                .collect(),
        }
    }

    /// 列定义（顺序即输出列顺序）。
    ///
    /// The column definitions (the order is the output column order).
    #[must_use]
    pub fn columns(&self) -> &[(String, DuckTypeDesc)] {
        &self.columns
    }

    /// 取第 `index` 列的定义。
    ///
    /// Returns the definition of column `index`.
    #[must_use]
    pub fn column(&self, index: usize) -> Option<&(String, DuckTypeDesc)> {
        self.columns.get(index)
    }

    /// 列数。
    ///
    /// The number of columns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// 是否没有列。
    ///
    /// Whether there are no columns at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// 把 schema 登记到 bind 信息上：逐列 `add_result_column_with_type`。
    ///
    /// 逻辑类型只在本次调用内构造，声明完即释放 —— 不跨阶段持有。
    ///
    /// Registers the schema on the bind info by calling `add_result_column_with_type` per column.
    /// The logical types are built inside the call and released right after — nothing is kept
    /// across phases.
    pub fn declare(&self, bind: &BindInfo) {
        for (name, desc) in &self.columns {
            let logical_type = desc.to_logical_type();
            bind.add_result_column_with_type(name, &logical_type);
        }
    }

    /// 各列的逻辑类型（顺序与 [`Self::columns`] 一致）。
    ///
    /// The logical types of every column (same order as [`Self::columns`]).
    #[must_use]
    pub fn to_logical_types(&self) -> Vec<LogicalType> {
        self.columns.iter().map(|(_, desc)| desc.to_logical_type()).collect()
    }
}

impl From<Vec<(String, DuckTypeDesc)>> for DuckResultSchema {
    /// `Vec<(列名, 列类型)>` → schema。
    ///
    /// `Vec<(column name, column type)>` → schema.
    fn from(columns: Vec<(String, DuckTypeDesc)>) -> Self {
        Self::new(columns)
    }
}
