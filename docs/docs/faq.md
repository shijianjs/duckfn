---
title: FAQ
sidebar_position: 8
description: Why no DuckDB build is needed, how this differs from duckdb-rs, and how to debug the errors people hit most often.
---

# FAQ

### Why doesn't duckfn need a local DuckDB build?

Because nothing is linked. `libduckdb-sys` is compiled with the `loadable-extension` feature, which
uses DuckDB's headers but resolves every API function through a pointer table that the host DuckDB
installs when it loads the extension. The trade-off is that the extension is tied to the DuckDB
version it was built against — see [Architecture](./internals/architecture.md#4-dispatch).

### Why do I have to load the extension with `-unsigned`?

The extension is not signed by DuckDB, and it is marked as using DuckDB's unstable C API, so DuckDB
refuses to load it unless unsigned extensions are allowed. Passing `-unsigned` on the command line
does that.

### How is this different from `duckdb-rs`?

They work in opposite directions. `duckdb-rs` (the `duckdb` crate) is a *client* binding: you embed a
DuckDB database inside a Rust program and call it. `duckfn` produces an *extension*: a shared library
that an existing DuckDB process loads with `LOAD`, adding functions to that process.

`duckfn` is built on [`quack-rs`](https://crates.io/crates/quack-rs) and `libduckdb-sys` rather than
on `duckdb-rs`.

### My function does not exist in SQL. What now?

Check, in order:

1. `LOAD` succeeded — a failed `LOAD` is easy to miss in a script.
2. The function is not registered with `auto_register = false`. Those only exist once something calls
   their builder; see [Attributes](./guide/attributes.md#automatic-vs-manual-registration).
3. The entry point symbol matches the extension name: `duckfn_entrypoint!("my_ext")` exports
   `my_ext_init_c_api`. A mismatch means the extension loads but registers nothing.
4. With `overloads_name = "…"`, the branch functions are **not** registered under their own names —
   only the set name exists.
5. The parameter or return types are ones the macro accepts. An unsupported shape is a compile error,
   but an unsupported *argument* combination shows up as
   `No function matches the given name and argument types`.

### Why doesn't my constant `NULL` reach the function body?

DuckDB folds constant expressions at bind time, so `NULL::INTEGER` and `NULL::INTEGER + 0` never
reach the callback. Set `special_null_handling = true` if the body needs to see them; column values
already arrive as `None`. See [Scalar functions](./guide/scalar-functions.md#special_null_handling).

### Why does a `NULL` inside `Vec<i32>` behave differently in a scalar and a table function?

A scalar function reads arguments row by row, so a `NULL` element makes the whole row `NULL` — unless
you declare `Vec<Option<i32>>`. A table function's arguments are *bind* parameters read from a
`Value`, and there `Vec<T>` with a `NULL` element is an error (`Vec<T> value is None`). Again, use
the `Option` variant when `NULL` elements are expected.

### Why can't an `ARRAY` be a table function argument?

DuckDB cannot bind a `Value` to an `ARRAY` type: `Bind value to array type is not supported`. Arrays
work fine as scalar function arguments, as list elements, and as struct fields.

### Can a function return `Result<T, ExtensionError>`?

No. The accepted shapes are `T`, `Option<T>` and `DuckOptionResult<T>` for scalar-like functions;
return `Ok(Some(value))` instead of `Ok(value)`.

### How do I return a `STRUCT`?

Derive `DuckStruct` on the type you return:

```rust
#[derive(Clone, Debug, DuckStruct)]
pub struct Point {
    x: i64,
    y: i64,
}
```

Its fields become the struct's fields at every level, including inside lists, maps and arrays.

### Why are named arguments ignored on scalar functions?

duckfn registers scalar functions by position, so DuckDB binds the values in the order written and
ignores the names — `f(b := 2, a := 1)` passes `2` first. `named_param_from` only takes effect for
table functions.

### How do I test an extension?

With sqllogictest files under `test/sql/`, run by `make test` (`just test`). The file sets up the
extension with `require rusty_quack`, then pairs statements with their expected output. See
[Contributing](./contributing.md#tests).

### The extension loads but calls fail with a version error

The extension is compiled against a specific DuckDB version (`TARGET_DUCKDB_VERSION`, currently
v1.5.5) and uses the unstable C API, so it only works with a compatible DuckDB. Load it into the
matching version, or rebuild against the version you are running.
