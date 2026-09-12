mod demo;
mod functions;
mod types;

use duckfn::duckfn_entrypoint;

// {extension_name}: 全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
duckfn_entrypoint!("rusty_quack");