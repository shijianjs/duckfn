---
title: Casts and replacement scans
sidebar_position: 5
description: Override DuckDB's CAST behaviour, and make SELECT * FROM 'data.points' resolve to your own table function.
---

# Casts and replacement scans

## Type casts

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

`NULL` handling follows the usual argument rule. With `Option<T>` the body sees the `NULL`:

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

Supported return shapes are the scalar ones (`T`, `Option<T>`, `DuckOptionResult<T>`), and complex
types work on both sides — `Vec<Option<String>>` to `Vec<Option<i32>>`, for example:

```sql
SELECT CAST(CAST(['1', '2'] AS INTEGER[]) AS VARCHAR);  -- [1, 2]
SELECT TRY_CAST(['1', 'x'] AS INTEGER[]);               -- NULL
SELECT CAST(['1', 'x'] AS INTEGER[]);                   -- error: not an integer: "x"
```

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

## Replacement scans

A replacement scan gets a chance to resolve any table name DuckDB could not find — typically a file
path. The callback returns the **name** of a table function to call instead; duckfn passes the
original name to it as the first `VARCHAR` parameter.

```rust
#[duck_replacement_scan]
fn dfn_scan_points(path: &str) -> DuckOptionResult<String> {
    if path.ends_with(".panic") {
        panic!("dfn_scan_points: panic while handling {path}");
    }
    if path.ends_with(".error") {
        return Err(duck_error(format!("dfn_scan_points: refuses {path}")));
    }
    if path.ends_with(".points") {
        return Ok(Some("dfn_scan_read_points".to_string()));
    }
    Ok(None)
}
```

The target is an ordinary table function:

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct ScanPoint {
    x: i64,
    y: i64,
}

#[duck_table_function]
fn dfn_scan_read_points(path: String) -> DuckResult<impl Iterator<Item = ScanPoint>> {
    let n = scan_parse_points(&path)?;
    Ok((0..n).map(|i| ScanPoint { x: i, y: i * i }))
}
```

```sql
SELECT * FROM '3.points';                  -- x 0 y 0 / 1 1 / 2 4
SELECT * FROM dfn_scan_read_points('2.points');  -- the same function, called directly
SELECT * FROM 'nope.txt';                  -- error: Table with name nope.txt does not exist
```

### Return shapes

| Shape | Meaning |
| --- | --- |
| `Option<String>` | `Some(table_fn)` takes over, `None` declines. |
| `Option<&'static str>` | The same, with a static name. |
| `DuckOptionResult<String>` | Adds an error path. |
| `DuckOptionResult<&'static str>` | Adds an error path, with a static name. |

An `Err` fails the query; a panic inside the callback is caught and reported as a query error too.

:::caution Two rules
1. The callback runs for **every** unresolved table name, so anything it does not handle must return
   `Ok(None)`. Never take over a name you do not own.
2. Matching is your own code: the example is case-sensitive, so `'3.POINTS'` is left to DuckDB and
   fails with `Table with name 3.POINTS does not exist`. Non-UTF-8 table names are passed through
   untouched.

If the returned table function does not exist, the query fails with
`Table Function with name dfn_scan_no_such_function does not exist`.
:::

## Next

- [SQL macros](./sql-macros.md)
- [Table functions](./table-functions.md) — what the target of a scan looks like.
