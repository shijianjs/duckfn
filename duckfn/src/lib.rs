//! `duckfn`：用普通 Rust 编写 DuckDB 扩展。
//!
//! [![GitHub](https://img.shields.io/badge/GitHub-181717?logo=github&logoColor=white)](https://github.com/shijianjs/duckfn)
//! [![Docs](https://img.shields.io/badge/docs-duckfn-14459b?logo=docusaurus&logoColor=white)](https://shijianjs.github.io/duckfn/)
//! [![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
//! [![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
//! [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
//! [![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)
//!
//! 本 crate 提供一组过程宏（`#[duck_scalar_function]`、`#[duck_aggregate_function]`、
//! `#[duck_table_function]`、`#[duck_copy_function]`、`#[duck_cast_function]`、`#[duck_sql_macro]`、
//! `#[duck_replacement_scan]`、`#[duck_custom_register]`、`#[derive(DuckStruct)]`、
//! `#[derive(DuckEnum)]`、`duckfn_entrypoint!`、`duck_sql_macro_files!`）以及配套的适配层和
//! 值类型工具，把普通的 Rust 函数/结构体/枚举直接变成可注册到 DuckDB 的标量函数、聚合函数、
//! 表函数、COPY 函数、cast 函数、SQL 宏，以及可映射到 LIST / MAP / ARRAY / STRUCT / ENUM 的值类型。
//!
//! `duckfn`: write DuckDB extensions in plain Rust.
//!
//! This crate ships a set of procedural macros (`#[duck_scalar_function]`,
//! `#[duck_aggregate_function]`, `#[duck_table_function]`, `#[duck_copy_function]`,
//! `#[duck_cast_function]`, `#[duck_sql_macro]`, `#[duck_replacement_scan]`,
//! `#[duck_custom_register]`, `#[derive(DuckStruct)]`, `#[derive(DuckEnum)]`,
//! `duckfn_entrypoint!`, `duck_sql_macro_files!`) together with the runtime adapter layer and
//! value-type helpers that turn ordinary Rust functions, structs and enums into DuckDB
//! scalar/aggregate/table/copy functions, casts, SQL macros and LIST / MAP / ARRAY / STRUCT / ENUM
//! value types.

/// 列集合读写抽象：`DuckColumns`。
///
/// Column read/write abstraction: [`DuckColumns`].
pub(crate) mod duck_columns;
/// 运行时动态列：`DuckTypeDesc` / `DuckDynamicValue` / `DuckResultSchema` /
/// `DuckDynamicRow` / `DuckDynamicTable`。
///
/// Runtime dynamic columns: [`DuckTypeDesc`] / [`DuckDynamicValue`] / [`DuckResultSchema`] /
/// [`DuckDynamicRow`] / [`DuckDynamicTable`].
pub(crate) mod duck_dynamic;
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
// 运行时动态列能力。
//
// Runtime dynamic-column capability.
pub use duck_dynamic::*;
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
// quack-rs 的类型再导出：自定义 `DuckValueType` 实现与宏生成的代码都要用到它们，
// 从这里走可以不必直接依赖 quack-rs。
//
// Re-exported quack-rs types: custom `DuckValueType` implementations and macro-generated code both
// need them, and going through `duckfn` means not having to depend on quack-rs directly.
pub use quack_rs::connection::Connection;
pub use quack_rs::prelude::{DataChunk, LogicalType, TypeId, Value};
// COPY 函数（`COPY ... TO (FORMAT xxx)`）相关的 quack-rs 类型：DuckDB 1.5.0+ 的
// C API 才提供，因此跟随 `duckdb-1-5` feature 一起开关。
//
// quack-rs types for copy functions (`COPY ... TO (FORMAT xxx)`): only the DuckDB 1.5.0+ C API
// provides them, so they follow the `duckdb-1-5` feature.
#[cfg(feature = "duckdb-1-5")]
pub use quack_rs::prelude::{
    CopyBindInfo, CopyFinalizeInfo, CopyFunctionBuilder, CopyGlobalInitInfo, CopySinkInfo,
};
