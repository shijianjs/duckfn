# duckfn-macro

[English](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/README.md) | [简体中文](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/duckfn-macro.svg)](https://crates.io/crates/duckfn-macro)
[![docs.rs](https://docs.rs/duckfn-macro/badge.svg)](https://docs.rs/duckfn-macro)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)

[`duckfn`](https://crates.io/crates/duckfn) 的过程宏实现 —— 用普通 Rust 代码生成 DuckDB 扩展。

本 crate 属于 `duckfn` 的实现细节：通常只需依赖 `duckfn` 并使用它重新导出的宏；只有想脱离运行时
单独使用宏时，才需要直接依赖 `duckfn-macro`。

## 宏列表

| 宏 | 作用 |
| --- | --- |
| `#[duck_scalar_function]` | 由 Rust 函数生成 DuckDB 标量函数。 |
| `#[duck_aggregate_function]` | 生成 DuckDB 聚合函数。 |
| `#[duck_table_function]` | 生成 DuckDB 表函数。 |
| `#[duck_sql_macro]` | 把 Rust 函数暴露为 DuckDB SQL 宏。 |
| `#[duck_custom_register]` | 手动注册函数，签名为 `fn(&Connection) -> DuckResult<()>`。 |
| `#[derive(DuckStruct)]` | 把结构体映射为 DuckDB `STRUCT`（支持嵌套 struct 和 list）。 |
| `duckfn_entrypoint!("name")` | 生成扩展入口符号。 |

常用参数：

- `auto_register = false` —— 只生成 builder，不自动注册。
- `named_param_from = "field"` —— 表函数命名参数从哪个字段开始。

## 示例

```rust
use duckfn::{duck_sql_macro, duck_table_function};
use quack_rs::prelude::SqlMacro;

#[duck_table_function]
fn count_down(start: i64) -> impl Iterator<Item = CountDownOutput> {
    (0..start).rev().map(|n| CountDownOutput { n })
}

#[derive(Default, Debug, Clone, duckfn::DuckStruct)]
pub struct CountDownOutput {
    n: i64,
}

#[duck_sql_macro]
pub fn clamp_macro() -> duckfn::DuckResult<SqlMacro> {
    SqlMacro::scalar("clamp", &["x", "lo", "hi"], "greatest(lo, least(hi, x))")
}
```

## 参见

完整文档、类型映射和示例扩展见主 crate：

<https://github.com/shijianjs/duckfn>

## 协议

基于 [MIT 协议](https://github.com/shijianjs/duckfn/blob/main/LICENSE) 开源。
