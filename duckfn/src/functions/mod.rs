//! 各类 DuckDB 函数的适配层：把 quack-rs 的向量级 C 回调拆成「逐行」的 Rust 代码。
//!
//! Adapter layer for every DuckDB function kind: splits quack-rs' vector-level C callbacks
//! into per-row Rust code (and, for `COPY ... TO`, into per-chunk Rust code).

/// 聚合函数适配层：`AggregateFunctionAdapter` / `DuckAggregateState` 等。
///
/// Aggregate-function adapter: `AggregateFunctionAdapter`, `DuckAggregateState`, ...
pub(crate) mod aggregate_function_adapter;
/// cast 函数适配层：`CastFunctionAdapter`。
///
/// Cast-function adapter: `CastFunctionAdapter`.
pub(crate) mod cast_function_adapter;
/// 标量函数适配层：`ScalarFunctionAdapter`。
///
/// Scalar-function adapter: `ScalarFunctionAdapter`.
pub(crate) mod scalar_function_adapter;
/// COPY FROM 适配层：`CopyFromFunctionAdapter` / `DuckCopyFromReader`（需要 `duckdb-1-5`）。
///
/// Copy-from adapter: `CopyFromFunctionAdapter`, `DuckCopyFromReader` (requires `duckdb-1-5`).
///
/// COPY 函数（`COPY ... TO` / `COPY ... FROM` 的自定义格式）走 DuckDB 1.5.0+ 的 C API，
/// 与 quack-rs 的 `copy_function` 模块一起跟随 `duckdb-1-5` feature 开关。
///
/// Copy functions (custom formats for `COPY ... TO` / `COPY ... FROM`) use the DuckDB 1.5.0+ C API
/// and follow the `duckdb-1-5` feature, like quack-rs' own `copy_function` module.
#[cfg(feature = "duckdb-1-5")]
pub(crate) mod copy_from_adapter;
/// COPY TO 适配层：`CopyToFunctionAdapter` / `DuckCopyToWriter`（需要 `duckdb-1-5`）。
///
/// Copy-to adapter: `CopyToFunctionAdapter`, `DuckCopyToWriter` (requires `duckdb-1-5`).
#[cfg(feature = "duckdb-1-5")]
pub(crate) mod copy_to_adapter;
/// 表函数适配层：`TableFunctionAdapter` / `DuckBindArgs` 等。
///
/// Table-function adapter: `TableFunctionAdapter`, `DuckBindArgs`, ...
pub(crate) mod table_function_adapter;
/// SQL 宏适配层：执行 SQL 文本注册宏。
///
/// SQL-macro adapter: registers macros from raw SQL text.
pub(crate) mod sql_macro_adapter;
/// replacement scan 适配层：`ReplacementScanAdapter`。
///
/// Replacement-scan adapter: `ReplacementScanAdapter`.
pub(crate) mod replacement_scan_adapter;

pub use aggregate_function_adapter::*;
pub use cast_function_adapter::*;
#[cfg(feature = "duckdb-1-5")]
pub use copy_from_adapter::*;
#[cfg(feature = "duckdb-1-5")]
pub use copy_to_adapter::*;
pub use scalar_function_adapter::*;
pub use sql_macro_adapter::*;
pub use table_function_adapter::*;
pub use replacement_scan_adapter::*;
