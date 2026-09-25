//! DuckDB 扩展入口（`duckfn_entrypoint!` 的展开结果）。
//!
//! 单独一个文件而不是写在 `mod.rs` 里，是为了让 CLI 能只编模块树、跳过入口符号：CLI
//! （`src/bin/duckfn.rs`）用 `#[path]` 把 `extension/mod.rs` 编进自己的二进制（原因见那个文件的
//! 注释），同时又会链接本包的 lib —— lib 里已经带着一份入口符号，再定义一次就是重复定义
//! （Windows 上直接 LNK2005）。所以 CLI 只包含 `extension/mod.rs`，绝不包含这个文件。
//!
//! 入口符号本身由本 crate 的 lib 提供一份就够：同一个 lib 既产出原生扩展的 cdylib，也产出
//! WebAssembly 用的 staticlib（见 Cargo.toml 的 crate-type），两者都需要它。
//!
//! The DuckDB extension entry point (the `duckfn_entrypoint!` expansion).
//!
//! It is a file of its own rather than part of `mod.rs` so that the CLI can compile the module tree
//! without the entry symbol: the CLI (`src/bin/duckfn.rs`) includes `extension/mod.rs` through
//! `#[path]` (for why, see the note in that file) and links this package's lib, which already carries
//! one copy of the entry symbol. Defining it a second time is a duplicate definition — an outright
//! LNK2005 on Windows — so the CLI includes `extension/mod.rs` only and never this file.
//!
//! The symbol itself only needs one definition, from this crate's lib: the same lib produces both the
//! native extension's cdylib and the WebAssembly staticlib (see the crate-type in Cargo.toml), and both
//! need it.

use duckfn::duckfn_entrypoint;

// {extension_name}: 全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
// 必须与根 Makefile 的 EXTENSION_NAME、CI 的 extension_name 以及 test/sql/**/*.test 里的
// `require` 一致。
//
// Must match EXTENSION_NAME in the root Makefile, `extension_name` in CI and the `require` lines of
// test/sql/**/*.test.
duckfn_entrypoint!("duckfn");
