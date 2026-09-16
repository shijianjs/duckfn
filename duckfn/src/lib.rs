//! `duckfn`：用普通 Rust 编写 DuckDB 扩展。
//!
//! [![GitHub](https://img.shields.io/badge/GitHub-shijianjs%2Fduckfn-181717?logo=github&logoColor=white)](https://github.com/shijianjs/duckfn)
//! [![Docs](https://img.shields.io/badge/docs-shijianjs.github.io%2Fduckfn-2e8555?logo=readthedocs&logoColor=white)](https://shijianjs.github.io/duckfn/)
//!
//! 本 crate 提供一组过程宏（`#[duck_scalar_function]`、`#[duck_aggregate_function]`、
//! `#[duck_table_function]`、`#[duck_cast_function]`、`#[duck_sql_macro]`、
//! `#[duck_replacement_scan]`、`#[duck_custom_register]`、`#[derive(DuckStruct)]`、
//! `duckfn_entrypoint!`、`duck_sql_macro_files!`）以及配套的适配层和值类型工具，
//! 把普通的 Rust 函数/结构体直接变成可注册到 DuckDB 的标量函数、聚合函数、表函数、
//! cast 函数、SQL 宏，以及可映射到 LIST / MAP / ARRAY / STRUCT 的嵌套值类型。
//!
//! `duckfn`: write DuckDB extensions in plain Rust.
//!
//! This crate ships a set of procedural macros (`#[duck_scalar_function]`,
//! `#[duck_aggregate_function]`, `#[duck_table_function]`, `#[duck_cast_function]`,
//! `#[duck_sql_macro]`, `#[duck_replacement_scan]`, `#[duck_custom_register]`,
//! `#[derive(DuckStruct)]`, `duckfn_entrypoint!`, `duck_sql_macro_files!`) together with
//! the runtime adapter layer and value-type helpers that turn ordinary Rust functions and
//! structs into DuckDB scalar/aggregate/table functions, casts, SQL macros and nested
//! LIST / MAP / ARRAY / STRUCT types.

/// 列集合读写抽象：`DuckColumns`。
///
/// Column read/write abstraction: [`DuckColumns`].
pub(crate) mod duck_columns;
/// DuckDB 值类型映射：基础类型、`LIST` / `MAP` / `ARRAY` / `STRUCT` 及包装类型。
///
/// DuckDB value-type mappings: primitives, `LIST` / `MAP` / `ARRAY` / `STRUCT` and wrappers.
pub(crate) mod value_types;
/// 函数注册收集器：汇聚宏提交的注册项并在初始化时统一注册。
///
/// Registration collector: gathers entries submitted by the macros and registers them at
/// initialisation time.
pub(crate) mod register;
/// 各类函数（标量/聚合/表/cast/SQL 宏/replacement scan）的适配层 trait。
///
/// Adapter traits for every function kind (scalar/aggregate/table/cast/SQL macro/
/// replacement scan).
pub(crate) mod functions;
/// 内部工具：错误转换、panic 捕获、builder 扩展。
///
/// Internal utilities: error conversion, panic catching and builder extensions.
pub(crate) mod utils;

// 过程宏（属性宏、derive、函数式宏）的再导出；由 `duckfn-macro` crate 提供。
//
// Re-export of the procedural macros (attribute, derive and function-like macros) provided
// by the `duckfn-macro` crate.
pub use duckfn_macro::*;
// 列集合读写抽象。
//
// Column read/write abstraction.
pub use duck_columns::*;
// 各类函数适配层 trait。
//
// Adapter traits for every function kind.
pub use functions::*;
// DuckDB 值类型映射（基础类型、嵌套类型、包装类型）。
//
// DuckDB value-type mappings (primitives, nested types, wrappers).
pub use value_types::*;
// 内部工具函数与类型别名。
//
// Internal helper functions and type aliases.
pub use utils::*;
// 注册入口与注册项类型。
//
// Registration entry point and registration-item types.
pub use register::*;
// `inventory::submit!` 的再导出，供宏生成的代码调用。
//
// Re-export of `inventory::submit!` so that macro-generated code can call it.
pub use inventory::submit as inventory_submit;
