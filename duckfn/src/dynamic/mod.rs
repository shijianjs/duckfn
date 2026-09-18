//! 运行时动态列：把「列名 + 列类型」从编译期搬到 bind 阶段。
//!
//! Runtime dynamic columns: moving column names and column types from compile time to the bind
//! phase.
//!
//! 普通表函数的输出 schema 由 `#[derive(DuckStruct)]` 结构体在编译期固定
//! （见 [`TableFunctionAdapter`](crate::TableFunctionAdapter)）。当列本身要在 bind 阶段读
//! 外部元数据（文件头、字典表、远端 schema）才确定时，就需要这条动态通路：
//!
//! - [`DuckTypeDesc`]：可跨线程保存的递归类型描述。它能在 bind 里转成 DuckDB 逻辑类型用来
//!   声明输出列，也能从外部逻辑类型反推，因此「按外部元数据构造 schema」是直接的；
//! - [`DuckDynamicValue`]：运行时值枚举，覆盖标量与 `LIST` / `STRUCT` / `MAP`，并携带 SQL NULL；
//!   值只带数据、不带类型 —— 类型一律来自 schema，避免两处真相不一致；
//! - [`DuckResultSchema`] / [`DuckDynamicRow`] / [`DuckDynamicTable`]：动态 schema、动态行与
//!   「schema + 行迭代器」的结果集，供
//!   [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter) 在 bind / scan 两阶段使用。
//!
//! The output schema of an ordinary table function is fixed at compile time by its
//! `#[derive(DuckStruct)]` row struct. When the columns can only be known at bind time — after
//! reading external metadata such as a file header, a dictionary table or a remote schema — this
//! dynamic path is what you need. [`DuckTypeDesc`] is a `Send`-friendly recursive type
//! description that converts to a DuckDB logical type (to declare result columns) and back (to
//! build a schema from external logical types). [`DuckDynamicValue`] is a runtime value enum
//! covering scalars plus `LIST` / `STRUCT` / `MAP` with SQL NULL; values carry data only, never
//! types — the schema is the single source of truth. [`DuckResultSchema`], [`DuckDynamicRow`] and
//! [`DuckDynamicTable`] then carry that schema and a row iterator through the bind/scan phases.

/// 运行时类型描述：`DuckTypeDesc`。
///
/// Runtime type description: `DuckTypeDesc`.
pub(crate) mod type_desc;
/// 运行时值：`DuckDynamicValue`。
///
/// Runtime values: `DuckDynamicValue`.
pub(crate) mod dynamic_value;
/// 动态结果集 schema：`DuckResultSchema`。
///
/// Dynamic result schema: `DuckResultSchema`.
pub(crate) mod result_schema;
/// 动态行：`DuckDynamicRow`。
///
/// Dynamic rows: `DuckDynamicRow`.
pub(crate) mod dynamic_row;
/// 动态结果集：`DuckDynamicIterator` / `DuckDynamicTable`。
///
/// Dynamic result table: `DuckDynamicIterator` / `DuckDynamicTable`.
pub(crate) mod dynamic_table;
/// 列写入器树：`DynColumnWriter` 及批量取值辅助。
///
/// Column writer tree: `DynColumnWriter` and the batch-value helpers.
pub(crate) mod dyn_column_writer;
/// 读取器树的构建辅助：`prepare_dynamic_reader` / `child_reader_at`。
///
/// Reader-tree construction helpers: `prepare_dynamic_reader` / `child_reader_at`.
pub(crate) mod reader;

pub use dynamic_row::*;
pub use dynamic_table::*;
pub use dynamic_value::*;
pub use result_schema::*;
pub use type_desc::*;
