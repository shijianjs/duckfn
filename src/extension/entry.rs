//! DuckDB 扩展入口（`duckfn_entrypoint!` 的展开结果）。
//!
//! 单独一个文件而不是写在 `mod.rs` 里，是因为它要被声明两次 —— 原生 cdylib（`src/lib.rs`）与
//! WebAssembly 目标（`src/wasm_lib.rs`）各一次 —— 而 CLI（`src/bin/duckfn.rs`）虽然也要编同一棵
//! 模块树（它用 `#[path]` 把 `extension/mod.rs` 编进自己的二进制，原因见那个文件的注释），却会
//! 链接本包的 lib：lib 里已经带着一份入口符号，再定义一次就是重复定义（Windows 上直接 LNK2005）。
//! 所以 CLI 只包含 `extension/mod.rs`，绝不包含这个文件。
//!
//! The DuckDB extension entry point (the `duckfn_entrypoint!` expansion).
//!
//! It is a file of its own rather than part of `mod.rs` because two targets have to declare it — the
//! native cdylib (`src/lib.rs`) and the WebAssembly target (`src/wasm_lib.rs`) — while the CLI
//! (`src/bin/duckfn.rs`) includes the same module tree (through `#[path]`, see the note in that file)
//! but links this package's lib, which already carries one copy of the entry symbol. Defining it a
//! second time is a duplicate definition — an outright LNK2005 on Windows — so the CLI includes
//! `extension/mod.rs` only and never this file.

use duckfn::duckfn_entrypoint;

// {extension_name}: 全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
// 必须与根 Makefile 的 EXTENSION_NAME、CI 的 extension_name 以及 test/sql/**/*.test 里的
// `require` 一致。
//
// Must match EXTENSION_NAME in the root Makefile, `extension_name` in CI and the `require` lines of
// test/sql/**/*.test.
duckfn_entrypoint!("duckfn");
