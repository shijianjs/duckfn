# duckfn-macro

[English](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/README.md) | [简体中文](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/duckfn-macro.svg)](https://crates.io/crates/duckfn-macro)
[![docs.rs](https://docs.rs/duckfn-macro/badge.svg)](https://docs.rs/duckfn-macro)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)

Procedural macros for [`duckfn`](https://crates.io/crates/duckfn) — generate DuckDB extensions
from plain Rust code.

This crate is an implementation detail of `duckfn`: you normally depend on `duckfn` and use the
macros it re-exports. Depend on `duckfn-macro` directly only if you want the macros without the
runtime.

## Macros

| Macro | Purpose |
| --- | --- |
| `#[duck_scalar_function]` | Generate a DuckDB scalar function from a Rust function. |
| `#[duck_aggregate_function]` | Generate a DuckDB aggregate function. |
| `#[duck_table_function]` | Generate a DuckDB table function. |
| `#[duck_sql_macro]` | Expose a Rust function as a DuckDB SQL macro. |
| `#[duck_custom_register]` | Register a function manually with signature `fn(&Connection) -> DuckResult<()>`. |
| `#[derive(DuckStruct)]` | Map a struct to a DuckDB `STRUCT` (nested structs and lists supported). |
| `duckfn_entrypoint!("name")` | Generate the extension entry point symbol. |

Common macro arguments:

- `auto_register = false` — only generate builders instead of registering automatically.
- `named_param_from = "field"` — where named arguments start for table functions.

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
```

## See also

Full documentation, type mapping and a complete example extension live in the main crate:

<https://github.com/shijianjs/duckfn>

## License

Licensed under the [MIT License](https://github.com/shijianjs/duckfn/blob/main/LICENSE).
