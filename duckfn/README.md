# duckfn

[English](https://github.com/shijianjs/duckfn/blob/main/duckfn/README.md) | [简体中文](https://github.com/shijianjs/duckfn/blob/main/duckfn/README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
[![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)

**Write DuckDB extensions in plain Rust.**

`duckfn` is a runtime framework for building [DuckDB](https://duckdb.org) extensions on top of
DuckDB's C Extension API. Together with [`duckfn-macro`](https://crates.io/crates/duckfn-macro),
a single attribute turns an ordinary Rust function into a DuckDB **scalar**, **aggregate** or
**table function**, a SQL macro, or a nested type — no C/C++ glue code, and no local DuckDB build
required.

- Repository: <https://github.com/shijianjs/duckfn>
- Built on [`quack-rs`](https://crates.io/crates/quack-rs) · [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys)
- No `unsafe` to write: no `unsafe fn`, no raw pointers in your function bodies
- No DuckDB build required, no C/C++ code
- Attribute-driven registration through `inventory`
- Panic-safe: Rust panics become DuckDB errors instead of unwinding across the FFI boundary
- Works with DuckDB's official multi-platform extension CI

> Status: early / experimental. APIs may change before `1.0`.

## Installation

```toml
[dependencies]
duckfn = "0.0.1"

# duckfn itself is built on these two crates; add them explicitly when you use
# their types or builders directly.
quack-rs = "0.16.0"
# `loadable-extension` dispatches through DuckDB's API table instead of linking
# libduckdb, which is what keeps a local DuckDB build unnecessary.
# (headers only — no linked library)
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

If you prefer the macros without the runtime, depend on
[`duckfn-macro`](https://crates.io/crates/duckfn-macro) directly; otherwise the macros are
re-exported by `duckfn` and no extra dependency is needed.

## Quick start

```rust
use duckfn::{duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult};

/// ```sql
/// SELECT double_it(21);    -- 42
/// SELECT double_it(NULL);  -- NULL
/// SELECT double_it(13);    -- error: unlucky input
/// ```
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

// Generate the extension entry point (name must be lowercase + underscores).
duckfn_entrypoint!("my_ext");
```

The wrapper, the logical types and the registration are all generated, so the snippet above is
entirely safe Rust — no `unsafe fn`, no raw pointers, no DuckDB C types. The only place `unsafe`
shows up is manual registration: `#[duck_custom_register]` calls quack-rs'
`unsafe fn register_scalar` / `register_aggregate` / `register_table`.

## Attributes

| Attribute | Purpose |
| --- | --- |
| `#[duck_scalar_function]` | Register a scalar function. |
| `#[duck_aggregate_function]` | Register an aggregate function. |
| `#[duck_table_function]` | Register a table function. |
| `#[duck_sql_macro]` | Register a SQL macro. |
| `#[duck_custom_register]` | Manually register builders, signature `fn(&Connection) -> DuckResult<()>`. |
| `#[derive(DuckStruct)]` | Map a struct to a DuckDB `STRUCT`. |
| `duckfn_entrypoint!("name")` | Generate the extension entry point. |

Common macro arguments:

- `auto_register = false` — only generate builders (`scalar_function_builder()`,
  `scalar_overload_builder()`, ...) instead of auto-registering; pair it with
  `#[duck_custom_register]`.
- `named_param_from = "field"` — where named arguments start for table functions.

`#[duck_scalar_function]` makes the function available as a module of the same name, exposing the
generated builders so you can register overloads or function sets yourself.

## Type mapping

| DuckDB | Rust |
| --- | --- |
| `BOOLEAN` | `bool` |
| `TINYINT` / `SMALLINT` / `INTEGER` / `BIGINT` | `i8` / `i16` / `i32` / `i64` |
| `UTINYINT` / `USMALLINT` / `UINTEGER` / `UBIGINT` | `u8` / `u16` / `u32` / `u64` |
| `HUGEINT` / `UHUGEINT` | `i128` / `u128` |
| `FLOAT` / `DOUBLE` | `f32` / `f64` |
| `VARCHAR` | `String` |
| `NULL` | `Option<T>` |
| `LIST(T)` | `Vec<T>`, nestable (`Vec<Option<Vec<Option<T>>>>` ...) |
| `MAP(K, V)` | `IndexMap<K, V>` |
| `ARRAY(T, N)` | `[T; N]` (`DuckArray`) / `[Option<T>; N]` (`DuckOptionArray`) |
| `STRUCT(...)` | `#[derive(DuckStruct)]`, nested structs and lists supported |

Nullability follows the Rust signature:

- a non-`Option` argument short-circuits the row to `NULL` when the input is `NULL` — the function
  body is not called;
- an `Option<T>` argument receives `None` and decides the semantics itself.

## Error handling and panics

Return `DuckOptionResult<T>` (i.e. `Result<Option<T>, ExtensionError>`) to emit `NULL` or fail the
query with `duck_error("...")`. Panics inside a function body are caught and converted into a
DuckDB error rather than unwinding across the FFI boundary.

A scalar function may use any of these return shapes:

```rust
#[duck_scalar_function] fn plain(i: i32) -> i32 { i * 2 }                 // never NULL
#[duck_scalar_function] fn maybe(i: i32) -> Option<i32> { Some(i) }       // None -> SQL NULL
#[duck_scalar_function] fn checked(i: i32) -> duckfn::DuckOptionResult<i32> { Ok(Some(i)) }
```

## Example extension

A complete example extension (`rusty_quack`) covering every feature lives in the repository:

<https://github.com/shijianjs/duckfn>

## License

Licensed under the [MIT License](https://github.com/shijianjs/duckfn/blob/main/LICENSE).
