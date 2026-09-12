# duckfn

[English](README.md) | [简体中文](README.zh-CN.md)

**Write DuckDB extensions in plain Rust.**

`duckfn` is a Rust framework for building [DuckDB](https://duckdb.org) extensions on top of
DuckDB's C Extension API. A single attribute turns an ordinary Rust function into a DuckDB
**scalar**, **aggregate** or **table function**, a SQL macro, or a nested type — no C/C++ glue
code, and no local DuckDB build required.

- Repository: <https://github.com/shijianjs/duckfn>
- Crates: [`duckfn`](https://crates.io/crates/duckfn) · [`duckfn-macro`](https://crates.io/crates/duckfn-macro)
- Built on: [`quack-rs`](https://crates.io/crates/quack-rs) · [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys)
- License: [MIT](LICENSE)

> Status: early / experimental. APIs may change before `1.0`.

## Repository layout

This repository is a Cargo workspace:

| Path | Description | Published to crates.io |
| --- | --- | --- |
| [`duckfn/`](duckfn/) | Runtime framework: traits, type adapters, function registration. | Yes |
| [`duckfn-macro/`](duckfn-macro/) | Procedural macros: `#[duck_scalar_function]`, `#[derive(DuckStruct)]`, ... | Yes |
| `/` (`rusty_quack`) | Example extension built with `duckfn`. Kept here to reuse DuckDB's official multi-platform CI. | No, example only |

## Installation

```toml
[dependencies]
duckfn = "0.0.1"

# duckfn is built on these two crates; add them explicitly when you use their
# types or builders directly (the example extension below does).
quack-rs = "0.16.0"
# `loadable-extension` dispatches through DuckDB's API table instead of linking
# libduckdb — that is what keeps a local DuckDB build unnecessary. (headers only)
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

The macros are re-exported by `duckfn`; add
[`duckfn-macro`](https://crates.io/crates/duckfn-macro) directly only if you want them without the
runtime.

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

// Generate the DuckDB extension entry point (extension name must be lowercase + underscores).
duckfn_entrypoint!("my_ext");
```

`#[duck_scalar_function]` generates the DuckDB wrapper, the logical types, and — by default —
registers the function through `inventory`. `duckfn_entrypoint!` emits the `*_init_c_api` symbol
DuckDB looks for when loading the extension.

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
  `scalar_overload_builder()`, ...), don't auto-register; pair it with `#[duck_custom_register]`.
- `named_param_from = "field"` — where named arguments start for table functions.

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

Nullability follows the Rust signature: a non-`Option` argument short-circuits the row to `NULL`
when the input is `NULL` (the body is not called), while an `Option<T>` argument receives `None`
and decides the semantics itself.

## Error handling and panics

Return `DuckOptionResult<T>` (i.e. `Result<Option<T>, ExtensionError>`) to emit `NULL` or fail the
query via `duck_error("...")`. Panics inside a function body are caught and converted into a
DuckDB error instead of unwinding across the FFI boundary.

## Running the example extension

The workspace root contains a full example extension (`rusty_quack`) covering every feature:

```shell
make configure
make debug
duckdb -unsigned -c "
LOAD './build/debug/extension/rusty_quack/rusty_quack.duckdb_extension';
SELECT rusty_echo('Jane');
"
```

See [`demo.sh`](demo.sh) for a long list of runnable SQL examples, and
[`duckfn/README.md`](duckfn/README.md) for the full library documentation.

## License

Licensed under the [MIT License](LICENSE).
