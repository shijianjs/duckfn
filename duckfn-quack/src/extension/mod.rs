mod demo;
mod functions;
mod types;

use duckfn::duckfn_entrypoint;

// {extension_name}: 全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
// 必须与根 Makefile 的 EXTENSION_NAME、CI 的 extension_name 以及
// duckfn-quack/test/sql/**/*.test 里的 `require` 一致。
//
// Must match EXTENSION_NAME in the root Makefile, `extension_name` in CI, and the `require` lines
// of duckfn-quack/test/sql/**/*.test.
duckfn_entrypoint!("duckfn_quack");