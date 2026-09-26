---
title: Replacement scans
sidebar_position: 8
description: Resolve a table name DuckDB cannot find — usually a file path — to one of your own table functions.
---

# Replacement scans

A replacement scan gets a chance to resolve any table name DuckDB could not find, which is usually a
file path such as `data.points`. The callback returns the **name** of a table function to call
instead; duckfn passes the original name to it as the first `VARCHAR` parameter.

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
SELECT * FROM '3.points';                        -- x 0 y 0 / 1 1 / 2 4
SELECT * FROM dfn_scan_read_points('2.points');  -- the same function, called directly
SELECT * FROM 'nope.txt';                        -- error: Table with name nope.txt does not exist
```

## Return shapes

| Shape | Meaning |
| --- | --- |
| `Option<String>` | `Some(table_fn)` takes over, `None` declines. |
| `Option<&'static str>` | The same, with a static name. |
| `DuckOptionResult<String>` | Adds an error path. |
| `DuckOptionResult<&'static str>` | Adds an error path, with a static name. |

An `Err` fails the query, and a panic inside the callback is caught and reported as a query error too.

:::caution[Two rules]

1. The callback runs for **every** unresolved table name, so anything it does not handle must return
   `Ok(None)`. Never take over a name you do not own.
2. Matching is your own code: the example is case-sensitive, so `'3.POINTS'` is left to DuckDB and
   fails with `Table with name 3.POINTS does not exist`. Non-UTF-8 table names are passed through
   untouched.

If the returned table function does not exist, the query fails with
`Table Function with name dfn_scan_no_such_function does not exist`.
:::

## Source and tests

- [`test/extension/functions/replacement_scan.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/replacement_scan.rs) — the callback and its target table functions
- [`test/sql/functions/replacement_scan.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/replacement_scan.test) — the expected results
- [`src/functions/replacement_scan_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/replacement_scan_adapter.rs) — the runtime side

## Next

- [Type mapping](./types.md)
- [Table functions](./table-functions.md) — what the target of a scan looks like.
