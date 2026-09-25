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
//! `#[duck_table_function]`、`#[duck_copy_function]`、`#[duck_copy_from_function]`、
//! `#[duck_cast_function]`、`#[duck_sql_macro]`、`#[duck_replacement_scan]`、
//! `#[duck_custom_register]`、`#[derive(DuckStruct)]`、`#[derive(DuckEnum)]`、`duckfn_entrypoint!`、
//! `duck_sql_macro_files!`）以及配套的适配层和值类型工具，把普通的 Rust 函数/结构体/枚举直接变成
//! 可注册到 DuckDB 的标量函数、聚合函数、表函数、COPY TO / COPY FROM 格式、cast 函数、SQL 宏，
//! 以及可映射到 LIST / MAP / ARRAY / STRUCT / ENUM 的值类型。
//!
//! `duckfn`: write DuckDB extensions in plain Rust.
//!
//! This crate ships a set of procedural macros (`#[duck_scalar_function]`,
//! `#[duck_aggregate_function]`, `#[duck_table_function]`, `#[duck_copy_function]`,
//! `#[duck_copy_from_function]`, `#[duck_cast_function]`, `#[duck_sql_macro]`,
//! `#[duck_replacement_scan]`, `#[duck_custom_register]`, `#[derive(DuckStruct)]`,
//! `#[derive(DuckEnum)]`, `duckfn_entrypoint!`, `duck_sql_macro_files!`) together with the runtime
//! adapter layer and value-type helpers that turn ordinary Rust functions, structs and enums into
//! DuckDB scalar/aggregate/table/copy formats, casts, SQL macros and LIST / MAP / ARRAY / STRUCT /
//! ENUM value types.

// 示例源码（src/extension/**）里写的是 `use duckfn::…` 与 `#[duckfn::duck_scalar_function(…)]`，
// 也就是把它当外部依赖来用 —— 这正是下游项目要抄的写法。在 crate 内部这些路径只有在把自己别名成
// `duckfn` 之后才解析得开，所以加这一行；示例源码与文档里的片段都不用改。
//
// The example sources (src/extension/**) spell the crate as an external dependency — `use duckfn::…`,
// `#[duckfn::duck_scalar_function(…)]` — which is exactly what a downstream project copies. Inside
// this crate those paths only resolve once the crate is aliased to `duckfn`, hence this line; the
// example sources and the snippets in the docs need no change.
extern crate self as duckfn;

/// 列集合读写抽象：`DuckColumns`。
///
/// Column read/write abstraction: [`DuckColumns`].
pub(crate) mod duck_columns;
/// 运行时动态列：`DuckTypeDesc` / `DuckDynamicValue` / `DuckResultSchema` /
/// `DuckDynamicRow` / `DuckDynamicTable`。
///
/// Runtime dynamic columns: [`DuckTypeDesc`] / [`DuckDynamicValue`] / [`DuckResultSchema`] /
/// [`DuckDynamicRow`] / [`DuckDynamicTable`].
pub(crate) mod dynamic;
/// DuckDB 值类型映射：基础类型、`LIST` / `MAP` / `ARRAY` / `STRUCT` 及包装类型。
///
/// DuckDB value-type mappings: primitives, `LIST` / `MAP` / `ARRAY` / `STRUCT` and wrappers.
pub(crate) mod value_types;
/// 函数注册收集器：汇聚宏提交的注册项并在初始化时统一注册。
///
/// Registration collector: gathers entries submitted by the macros and registers them at
/// initialisation time.
pub(crate) mod register;
/// 函数级附加数据（DuckDB C API 的 `extra_info`）：`DuckExtraInfo` 与读取辅助。
///
/// Function-level extra data (the DuckDB C API's `extra_info`): [`DuckExtraInfo`] and the read
/// helpers.
pub(crate) mod extra_info;
/// 函数文档元数据的收集（社区扩展文档页需要的 `function_descriptions.csv` 的来源）。
///
/// Collection of the function documentation metadata (the source of the
/// `function_descriptions.csv` the community-extension doc pages need).
pub(crate) mod doc;
/// 命令行工具：不参与插件运行时的逻辑（导出 `function_descriptions.csv`），挂在 `cli` feature 后面。
///
/// The command-line tool: the logic that is not part of the extension runtime (exporting
/// `function_descriptions.csv`), behind the `cli` feature.
#[cfg(feature = "cli")]
pub mod cli;
/// 各类函数（标量/聚合/表/cast/SQL 宏/replacement scan）的适配层 trait。
///
/// Adapter traits for every function kind (scalar/aggregate/table/cast/SQL macro/
/// replacement scan).
pub(crate) mod functions;
/// 内部工具：错误转换、panic 捕获、builder 扩展。
///
/// Internal utilities: error conversion, panic catching and builder extensions.
pub(crate) mod utils;
// 宿主文件系统（DuckDB 的 VFS）访问与便捷文件读写：模块文档见 `src/duck_vfs/mod.rs`。
// 这里用普通注释而不是 `///`，避免外层文档与模块内文档合并后，内层的 intra-doc 链接在
// crate 根作用域里解析失败。需要 DuckDB 1.5.0+ 与 `duckdb-1-5` feature。
//
// Host file system (DuckDB's VFS) access plus convenience file reads and writes: the module docs
// live in `src/duck_vfs/mod.rs`. A plain comment instead of `///` keeps the outer doc from
// merging with the inner one, which would make the inner intra-doc links resolve in the crate root
// scope and fail. Requires DuckDB 1.5.0+ and the `duckdb-1-5` feature.
#[cfg(feature = "duckdb-1-5")]
pub mod duck_vfs;

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
pub use dynamic::*;
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
// 函数级附加数据（`extra_info`）。
//
// Function-level extra data (`extra_info`).
pub use extra_info::*;
// 函数文档元数据的收集。
//
// Collection of the function documentation metadata.
pub use doc::*;
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
// `BindInfo` 也在再导出之列：`DuckBindArgs::read_bind_args` 的签名里就有它，手写该 trait 时
// 不必再直接依赖 quack-rs。
//
// `BindInfo` is re-exported too: it appears in `DuckBindArgs::read_bind_args`'s signature, so
// hand-writing that trait does not require depending on quack-rs directly.
pub use quack_rs::prelude::{BindInfo, DataChunk, LogicalType, TypeId, Value};
// COPY 函数（`COPY ... TO (FORMAT xxx)`）相关的 quack-rs 类型：DuckDB 1.5.0+ 的
// C API 才提供，因此跟随 `duckdb-1-5` feature 一起开关。
//
// quack-rs types for copy functions (`COPY ... TO (FORMAT xxx)`): only the DuckDB 1.5.0+ C API
// provides them, so they follow the `duckdb-1-5` feature.
#[cfg(feature = "duckdb-1-5")]
pub use quack_rs::prelude::{
    CopyBindInfo, CopyFinalizeInfo, CopyFunctionBuilder, CopyGlobalInitInfo, CopySinkInfo,
};

// 示例扩展：并进本包后由 `quack` feature 打开，模块树在 src/extension/。这份源码只由本 crate 编一遍：
// 本 lib 同时产出原生扩展的 cdylib 与 WebAssembly 用的 staticlib（见 Cargo.toml 的 crate-type），
// CLI（src/bin/duckfn.rs）再自带一份以便收集注册项。feature 关闭时示例源码随包发布但不参与编译，
// 下游依赖树因此完全不受影响。
//
// The example extension: folded into this package and switched on by the `quack` feature, with its
// module tree in src/extension/. That source is compiled exactly once, by this crate: the lib produces
// both the native cdylib and the WebAssembly staticlib (see the crate-type in Cargo.toml), and the CLI
// (src/bin/duckfn.rs) carries its own copy so it can collect the registrations. With the feature off
// the example sources ship in the package without being compiled, so a downstream dependency tree is
// unaffected.
#[cfg(feature = "quack")]
mod extension;
// 入口符号（`duckfn_entrypoint!`）单独放，CLI 编同一棵树时要能跳过它 —— 原因见该文件与
// src/bin/duckfn.rs 的注释。
//
// The entry point (`duckfn_entrypoint!`) is kept apart so the CLI can skip it while compiling the same
// tree — see the note in that file and in src/bin/duckfn.rs.
#[cfg(feature = "quack")]
#[path = "extension/entry.rs"]
mod extension_entry;
