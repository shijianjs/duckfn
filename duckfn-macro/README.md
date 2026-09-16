<div align="center">
  <img src="https://raw.githubusercontent.com/shijianjs/duckfn/main/docs/static/img/duckfn-logo.svg" alt="duckfn logo" width="180" />
</div>

# duckfn-macro

[English](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/README.md) | [简体中文](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/duckfn-macro.svg)](https://crates.io/crates/duckfn-macro)
[![docs.rs](https://docs.rs/duckfn-macro/badge.svg)](https://docs.rs/duckfn-macro)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)

Procedural macros for [`duckfn`](https://crates.io/crates/duckfn) — generate DuckDB extensions
from plain Rust code.

This crate is an implementation detail of `duckfn`: you normally depend on `duckfn` and use the
macros it re-exports. Depend on `duckfn-macro` directly only if you want the macros without the
runtime.

## Macros

| Macro | Purpose |
| --- | --- |
| `#[duck_scalar_function]` | A DuckDB scalar function. |
| `#[duck_aggregate_function]` | A DuckDB aggregate function. |
| `#[duck_cast_function]` | A cast (`CAST(x AS T)` / `TRY_CAST`); the argument is the source, the return type the target. |
| `#[duck_table_function]` | A DuckDB table function. |
| `#[duck_replacement_scan]` | Redirect an unresolved table name (usually a file path) to a table function. |
| `#[duck_sql_macro]` | A SQL macro, either as a `SqlMacro` or as raw SQL to execute. |
| `#[duck_custom_register]` | Manual registration, signature `fn(&Connection) -> DuckResult<()>`. |
| `#[derive(DuckStruct)]` | Map a struct to a DuckDB `STRUCT` (optionally creating the type at load time with `create_type = true`). |
| `#[derive(DuckEnum)]` | Map a unit-variant enum to a DuckDB `ENUM` (optionally creating the type at load time with `create_type = true`). |
| `duckfn_entrypoint!("name")` | The extension entry point symbol. |
| `duck_sql_macro_files!("a.sql", …)` | Register SQL macros kept in `.sql` files. |

Common arguments: `auto_register = false` (generate the builders without registering),
`named_param_from = "field"` (where named parameters start in a table function),
`special_null_handling`, `implicit_cost` and `overloads_name`.

## Example

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

// Or just return the raw SQL string, executed on registration.
#[duck_sql_macro]
pub fn add_two_macro() -> duckfn::DuckResult<String> {
    Ok("CREATE MACRO add_two(x) AS x + 2".to_string())
}
```

## Documentation

Every macro, its arguments and the items it generates are documented at
**<https://shijianjs.github.io/duckfn/>**:

- [Attributes](https://shijianjs.github.io/duckfn/docs/guide/attributes) — the full attribute and argument reference.
- [Scalar](https://shijianjs.github.io/duckfn/docs/guide/scalar-functions) · [Aggregate](https://shijianjs.github.io/duckfn/docs/guide/aggregate-functions) · [Table](https://shijianjs.github.io/duckfn/docs/guide/table-functions) functions
- [SQL macros](https://shijianjs.github.io/duckfn/docs/guide/sql-macros) · [Type casts](https://shijianjs.github.io/duckfn/docs/guide/casts) · [Replacement scans](https://shijianjs.github.io/duckfn/docs/guide/replacement-scans)
- [Architecture](https://shijianjs.github.io/duckfn/docs/internals/architecture) — what each macro expands to.

中文文档：<https://shijianjs.github.io/duckfn/zh-Hans/>

## License

Licensed under the [MIT License](https://github.com/shijianjs/duckfn/blob/main/LICENSE).
