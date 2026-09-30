//! DuckDB 扩展入口（`duckfn_entrypoint!` 的展开结果）。
//!
//! 单独一个文件而不是写在 `mod.rs` 里，是为了让 CLI 能只编模块树、跳过入口符号：CLI
//! （`src/bin/duckfn.rs`）用 `#[path]` 把 `extension/mod.rs` 编进自己的二进制（原因见那个文件的
//! 注释），而它是个 executable，并不需要入口符号。
//!
//! 入口符号本身只需要一份，由 `examples/duckfn.rs` 提供：那个 target 同时产出原生扩展的 cdylib
//! 与 WebAssembly 用的 staticlib（见 Cargo.toml 的 `[[example]]`），两者都需要它。
//!
//! The DuckDB extension entry point (the `duckfn_entrypoint!` expansion).
//!
//! It is a file of its own rather than part of `mod.rs` so that the CLI can compile the module tree
//! without the entry symbol: the CLI (`src/bin/duckfn.rs`) includes `extension/mod.rs` through
//! `#[path]` (for why, see the note in that file), and being an executable it has no use for the
//! entry symbol.
//!
//! One definition is enough, and it comes from `examples/duckfn.rs`: that target produces both the
//! native extension's cdylib and the WebAssembly staticlib (see `[[example]]` in Cargo.toml), and both
//! need it.

use duckfn::duckfn_entrypoint;

// {extension_name}: 全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
// 必须与根 Makefile 的 EXTENSION_NAME、CI 的 extension_name 以及 test/sql/**/*.test 里的
// `require` 一致。
//
// Must match EXTENSION_NAME in the root Makefile, `extension_name` in CI and the `require` lines of
// test/sql/**/*.test.
duckfn_entrypoint!("duckfn");
