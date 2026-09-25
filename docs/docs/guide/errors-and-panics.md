---
title: Errors and panics
sidebar_position: 10
description: Reporting a query error, returning NULL, and how Rust panics become DuckDB errors.
---

# Errors and panics

## Errors

`DuckOptionResult<T>` is `Result<Option<T>, ExtensionError>`, which gives a function three outcomes
at once:

```rust
#[duck_scalar_function]
fn dfn_scalar_ret_checked(i: i32) -> DuckOptionResult<i32> {
    if i == 0 {
        return Err(duck_error("dfn_scalar_ret_checked: division by zero"));
    }
    if i < 0 {
        return Ok(None);
    }
    Ok(Some(100 / i))
}
```

| Return value | SQL |
| --- | --- |
| `Ok(Some(v))` | `v` |
| `Ok(None)` | `NULL` |
| `Err(e)` | The query fails with the message of `e`. |

```sql
SELECT dfn_scalar_ret_checked(4);    -- 25
SELECT dfn_scalar_ret_checked(-1);   -- NULL
SELECT dfn_scalar_ret_checked(0);    -- error: dfn_scalar_ret_checked: division by zero
```

`duck_error("…")` constructs the error. `ExtensionError` is
`quack_rs::error::ExtensionError`, so `?` also propagates errors from `quack-rs` APIs directly, and
`format!` is the usual way to put values into the message.

Conventions used throughout the example extension, and worth following:

- Start the message with the name of the function, so it is obvious where the failure came from:
  `dfn_cast_str_to_int: not an integer: "abc"`, `dfn_table_checked: n must be >= 0`.
- Reserve `Ok(None)` for "no value for this row", and use `Err` for real failures.

Note that `Result<T, ExtensionError>` is *not* one of the shapes the macros accept — use
`DuckOptionResult<T>` and wrap successful values in `Some`.

## Panics

A panic inside a duckfn function does not unwind across the FFI boundary. It is caught and reported
as a query error:

```rust
#[duck_scalar_function]
fn dfn_scalar_ret_panic(i: i32) -> i32 {
    if i == 13 {
        panic!("unlucky input: {i}");
    }
    i
}
```

```sql
SELECT dfn_scalar_ret_panic(1);    -- 1
SELECT dfn_scalar_ret_panic(13);   -- error: unlucky input: 13
```

The same holds for every registration kind:

| Kind | Panics caught in | Example |
| --- | --- | --- |
| Scalar | The function body | `SELECT dfn_scalar_ret_panic(13);` |
| Aggregate | The row handler | `SELECT dfn_agg_panic(x) FROM (VALUES (13)) t(x);` |
| Cast | The function body | `SELECT CAST('NaN'::DOUBLE AS BIGINT);` → `dfn_cast_double_to_bigint: not a finite number: NaN` |
| Table function | The iterator, and the bind step | `SELECT * FROM dfn_table_full(5);` → `dfn_table_full: bad row 2` |
| Replacement scan | The callback | `SELECT * FROM 'boom.panic';` → `dfn_scan_points: panic while handling boom.panic` |

:::caution[Panics are a safety net, not a control-flow tool]
Being caught keeps the query, not the developer, in charge: a panic aborts the whole query and loses
the error type you would otherwise choose. Prefer `Ok(None)` and `Err(duck_error(…))`.
:::

## Where each failure surfaces

**Table functions.** The return shape decides when a failure is reported:

| Failure | Shape | Result |
| --- | --- | --- |
| Invalid arguments | `DuckResult<impl Iterator<Item = Row>>` | The query fails before any row is produced. |
| A `NULL` row | `Ok(None)` in a `DuckFullIteratorResult` | Every column of that row is `NULL`. |
| A row that cannot be produced | `Err(…)` in a `DuckFullIteratorResult` | The query fails. |

**Casts.** The same `Err` behaves differently depending on how the cast was reached:

| SQL | Result |
| --- | --- |
| `CAST('abc' AS INTEGER)` | The query fails. |
| `TRY_CAST('abc' AS INTEGER)` | `NULL` for that row, and the query continues. |

**Aggregates.** An `Err` returned from the row handler fails the query; there is no per-row `NULL`
channel any more, because a row only updates the state.

## Source and tests

- [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) — `duck_error`, `Ok(None)` and panic cases
- [`test/sql/functions/aggregate_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/aggregate_function.test) — the same for aggregates
- [`test/sql/functions/cast_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/cast_function.test) — `CAST` versus `TRY_CAST`
- [`src/functions/table_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/table_function_adapter.rs) — where bind and scan catch panics

## Next

- [Attributes](./attributes.md) — which return shapes each macro accepts.
- [The example extension](../examples/duckfn.md) — all of these error paths in runnable SQL.
