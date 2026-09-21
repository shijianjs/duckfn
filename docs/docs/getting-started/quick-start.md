---
title: Quick start
sidebar_position: 3
description: Write, build and load a minimal duckfn extension, then call it from SQL.
---

# Quick start

This page builds a minimal extension called `my_ext`. The repository's own example extension is
`rusty_quack` — see [the example extension](../examples/rusty-quack.md) for one that covers every
feature.

To start from a working skeleton with CI already in place instead, see
[Create a project](./create-a-project.md).

## 1. Create the crate

```toml title="Cargo.toml"
[package]
name = "my_ext"
version = "0.1.0"
edition = "2024"
rust-version = "1.86"

[lib]
crate-type = ["cdylib"]

[dependencies]
duckfn = "{{DUCKFN_VERSION}}"
quack-rs = "0.16.0"
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

## 2. Write the extension

```rust title="src/lib.rs"
use duckfn::{duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult};

#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

duckfn_entrypoint!("my_ext");
```

Three things are worth noticing:

- The attribute both **registers** the function and generates its FFI wrapper — there is no manual
  registration step.
- `Option<i64>` in and `DuckOptionResult<i64>` out are the way to express `NULL`. See
  [Errors and panics](../guide/errors-and-panics.md) for the full set of return shapes.
- `duckfn_entrypoint!("my_ext")` must match the extension name; see below.

## 3. Build it

The official DuckDB flow builds a `cdylib`, appends extension metadata, and lays the result out where
DuckDB expects it:

```bash
make configure   # one-time: creates the Python venv used by the test runner
make debug       # -> build/debug/extension/my_ext/my_ext.duckdb_extension
```

`make release` does the same with optimisations. Both come from DuckDB's `extension-ci-tools`
makefiles, which the repository includes.

Alternatively, the `cargo-duckdb-ext-tools` plugin packages the extension straight from Cargo:

```bash
cargo duckdb-ext build   # -> target/debug/my_ext.duckdb_extension
```

The repository wraps both flows in its `Justfile`, for example
`just sql "SELECT double_it(21);"`.

## 4. Load and call it

```bash
duckdb -unsigned -c "
LOAD './build/debug/extension/my_ext/my_ext.duckdb_extension';
SELECT double_it(21);
"
```

```text
42
```

| SQL | Result |
| --- | --- |
| `SELECT double_it(21);` | `42` |
| `SELECT double_it(NULL);` | `NULL` |
| `SELECT double_it(13);` | error: `unlucky input` |

`-unsigned` is required because the extension is built against DuckDB's unstable C API.

## The entry point name

`duckfn_entrypoint!("my_ext")` generates the symbol **`my_ext_init_c_api`**, which is the symbol
DuckDB looks up when the extension is loaded. The name must be non-empty and may only contain
lowercase ASCII letters, digits and underscores — anything else fails at compile time, and a wrong
symbol name makes the extension unloadable.

## Next steps

- [Attributes](../guide/attributes.md) — every attribute and macro argument in one place.
- [Scalar functions](../guide/scalar-functions.md) — return shapes, overloads and function sets.
- [Type mapping](../guide/types.md) — the Rust types accepted as arguments and results.
- [The example extension](../examples/rusty-quack.md) — runnable SQL for every feature.
