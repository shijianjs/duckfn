---
title: Table functions
sidebar_position: 4
description: Row structs, the three iterator return shapes, named and optional parameters, and streaming.
---

# Table functions

A table function returns a row struct, and the struct's fields become the output columns.

```rust
#[duck_table_function]
fn dfn_table_range(n: i64) -> impl Iterator<Item = RangeRow> {
    (0..n.max(0)).map(|i| RangeRow {
        n: i,
        square: i * i,
    })
}

#[derive(Default, Debug, Clone, DuckStruct)]
pub struct RangeRow {
    n: i64,
    square: i64,
}
```

```sql
SELECT * FROM dfn_table_range(3);
```

```text
┌───────┬────────┐
│   n   │ square │
├───────┼────────┤
│     0 │      0 │
│     1 │      1 │
│     2 │      4 │
└───────┴────────┘
```

The row struct is a plain `#[derive(DuckStruct)]` type, so columns may be nullable (`Option<T>`),
lists, maps, arrays — or nested structs.

## The three return shapes

| Shape | Use when |
| --- | --- |
| `impl Iterator<Item = Row>` | Binding cannot fail and no row can be `NULL`. |
| `DuckResult<impl Iterator<Item = Row>>` | Argument validation may fail before any row is produced. |
| `DuckFullIteratorResult<Row>` | Individual rows can be `NULL`, or can fail. |

`DuckFullIteratorResult<Row>` is
`DuckResult<Box<dyn Iterator<Item = DuckOptionResult<Row>> + Send>>`, which gives the iterator three
outcomes per row:

```rust
#[duck_table_function]
fn dfn_table_full(n: i64) -> DuckFullIteratorResult<RangeRow> {
    let rows = (0..n.max(0)).map(|i| -> DuckOptionResult<RangeRow> {
        match i {
            1 => Ok(None),                                    // this row is all NULL
            2 => Err(duck_error("dfn_table_full: bad row 2")), // this row errors
            _ => Ok(Some(RangeRow { n: i, square: i * i })),
        }
    });
    Ok(Box::new(rows))
}
```

```sql
SELECT * FROM dfn_table_full(2);   -- 0 0  then  NULL NULL
SELECT * FROM dfn_table_full(5);   -- error: dfn_table_full: bad row 2
```

Arguments are validated before the first row, so the middle shape is the natural place for it:

```rust
#[duck_table_function]
fn dfn_table_checked(n: i64) -> DuckResult<impl Iterator<Item = RangeRow>> {
    if n < 0 {
        return Err(duck_error("dfn_table_checked: n must be >= 0"));
    }
    Ok((0..n).map(|i| RangeRow { n: i, square: i * i }))
}
```

```sql
SELECT * FROM dfn_table_checked(-1);  -- error: dfn_table_checked: n must be >= 0
```

## Positional and named parameters

By default every argument is positional. `named_param_from` marks the argument from which on
everything is passed as a named parameter:

```rust
#[duck_table_function(named_param_from = "start")]
fn dfn_table_countdown(step: i64, start: i64, count: i64) -> impl Iterator<Item = RangeRow> {
    (0..count.max(0)).map(move |i| {
        let n = start - i * step;
        RangeRow { n, square: n * n }
    })
}
```

```sql
SELECT * FROM dfn_table_countdown(2, start=10, count=3);        -- 10, 8, 6
SELECT * FROM dfn_table_countdown(step=2, start=10, count=3);   -- error: No function matches
SELECT * FROM dfn_table_countdown(2, start=10, count=3, foo=1); -- error: Invalid named parameter "foo"
```

Parameters *before* the marker cannot be passed by name, and an unknown name is rejected rather than
ignored.

## Optional and required parameters

`Option<T>` makes a named parameter optional — `NULL` and omission both arrive as `None`:

```rust
#[duck_table_function(named_param_from = "start")]
fn dfn_table_opt(start: i64, step: Option<i64>, count: Option<i64>) -> impl Iterator<Item = RangeRow> {
    let step = step.unwrap_or(1);
    let count = count.unwrap_or(3);
    /* … */
}
```

```sql
SELECT * FROM dfn_table_opt(start=1);                        -- 1, 2, 3
SELECT * FROM dfn_table_opt(start=1, step=10, count=2);      -- 1, 11
SELECT * FROM dfn_table_opt(start=1, step=NULL, count=NULL);  -- 1, 2, 3
```

A plain `T` parameter is required. Omitting it, or passing `NULL`, fails at bind time:

```sql
SELECT * FROM dfn_table_req();           -- error: Parameter start cannot be null
SELECT * FROM dfn_table_req(start=NULL); -- error: Parameter start cannot be null
SELECT * FROM dfn_table_req(start=5);    -- 5, 25
```

## Columns

Any field type from the [type mapping](./types.md) may be a column, including nullable fields,
lists and nested structs:

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct TypedRow {
    id: i64,
    name: String,
    score: Option<f64>,
    tags: Vec<Option<i64>>,
}
```

```sql
DESCRIBE SELECT * FROM dfn_table_typed(3);
-- id     BIGINT
-- name   VARCHAR
-- score  DOUBLE
-- tags   BIGINT[]

SELECT id, score FROM dfn_table_typed(3);
-- 0 0.0
-- 1 NULL
-- 2 1.0
```

Nested structs become `STRUCT` columns and are addressable field by field:

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct ShapeRow {
    name: String,
    from_point: Point,
    to_point: Point,
}
```

```sql
SELECT name, from_point.x, to_point.y FROM dfn_table_nested(2);
-- s0 0 0
-- s1 1 3
```

## Arguments

A table function may take no argument at all, and it may take complex ones: a `LIST` argument
(`Vec<i64>`) or a `MAP` argument (`IndexMap<String, i64>`):

```sql
SELECT * FROM dfn_table_zero_args();                   -- 0 0 / 1 1 / 2 4
SELECT * FROM dfn_table_from_list([3, 1, 2]);          -- 3 9 / 1 1 / 2 4
SELECT * FROM dfn_table_from_map(MAP {'a': 1, 'b': 2}); -- a 1 / b 2
```

:::note Limitations
- Arguments are *bind* parameters, so a `NULL` inside a `Vec<T>` (element type not nullable) is an
  error rather than a skipped element.
- `ARRAY` types are not supported as bind parameters:
  `SELECT * FROM dfn_table_echo_array_integer_param([1,2,3]::INTEGER[3])` fails with
  `Bind value to array type is not supported`.
- Table function columns cannot be used as lateral join parameters.
:::

## Streaming

Rows are produced lazily, one DuckDB vector at a time, so large result sets do not have to be
materialised. `SELECT count(*) FROM dfn_table_range(2048)` returns `2048`, and so does the next
matching size — the iterator is simply pulled until it is exhausted.

## Next

- [Casts and replacement scans](./casts-and-scans.md)
- [Type mapping](./types.md)
