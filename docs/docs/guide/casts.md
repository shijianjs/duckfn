---
title: Type casts
sidebar_position: 7
description: Override DuckDB's CAST behaviour for one source/target pair, including TRY_CAST and implicit conversion costs.
---

# Type casts

A cast function takes exactly one argument — the source value — and its return type is the target
type. Both are inferred from the signature:

```rust
#[duck_cast_function]
fn dfn_cast_str_to_int(s: String) -> DuckOptionResult<i32> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        // Overrides the built-in behaviour, where CAST('' AS INTEGER) is an error.
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| {
        duck_error(format!("dfn_cast_str_to_int: not an integer: {s:?}"))
    })
}
```

```sql
SELECT CAST('42' AS INTEGER);       -- 42
SELECT CAST(' 7 ' AS INTEGER);      -- 7     (trimmed)
SELECT CAST('' AS INTEGER);         -- NULL  (the built-in error is overridden)
SELECT TRY_CAST('abc' AS INTEGER);  -- NULL
SELECT CAST('abc' AS INTEGER);      -- error: dfn_cast_str_to_int: not an integer: "abc"
```

The last two lines are the same function reached through two different cast modes:

| Mode | On error |
| --- | --- |
| `CAST(x AS T)` | The error is reported and the query fails. |
| `TRY_CAST(x AS T)` | The error becomes `NULL` for that row and the query continues. |

## NULL handling

The usual argument rule applies: with `T` the row short-circuits, with `Option<T>` the body receives
the `NULL`.

```rust
#[duck_cast_function]
fn dfn_cast_bigint_to_double(v: Option<i64>) -> Option<f64> {
    match v {
        Some(v) => Some(v as f64 / 2.0),
        None => Some(-1.0),   // the NULL reached the body
    }
}
```

```sql
SELECT CAST(3::BIGINT AS DOUBLE);     -- 1.5
SELECT CAST(NULL::BIGINT AS DOUBLE);  -- -1.0
```

## Types

Anything in the [type mapping](./types.md) can be used on either side, containers included:

```sql
SELECT CAST(CAST(['1', '2'] AS INTEGER[]) AS VARCHAR);  -- [1, 2]
SELECT TRY_CAST(['1', 'x'] AS INTEGER[]);               -- NULL
SELECT CAST(['1', 'x'] AS INTEGER[]);                   -- error: not an integer: "x"
```

Registering a cast for a pair DuckDB already handles *replaces* the built-in behaviour — which is why
`CAST('' AS INTEGER)` above yields `NULL` instead of an error.

## Implicit conversion cost

Set `implicit_cost` to let DuckDB use the cast for implicit conversions, with the given cost:

```rust
/// VARCHAR -> HUGEINT, usable as an implicit conversion with cost 100.
#[duck_cast_function(implicit_cost = 100)]
fn dfn_cast_str_to_hugeint(s: String) -> i128 { /* … */ }
```

```sql
SELECT CAST('41' AS HUGEINT);              -- 41
SELECT CAST('41' AS VARCHAR) + 1::HUGEINT; -- 42
```

## Source and tests

- [`duckfn-quack/src/extension/functions/cast_function.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/src/extension/functions/cast_function.rs) — the example casts
- [`duckfn-quack/test/sql/functions/cast_function.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/functions/cast_function.test) — the expected results
- [`src/functions/cast_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/cast_function_adapter.rs) — the runtime side

## Next

- [Replacement scans](./replacement-scans.md)
- [Type mapping](./types.md)
