---
title: Scalar functions
sidebar_position: 2
description: Return shapes, NULL handling, arity, overloads and named parameters for duckfn scalar functions.
---

# Scalar functions

```rust
#[duck_scalar_function]
fn dfn_scalar_ret_plain(i: i32) -> i32 {
    i * 2
}
```

The arguments become DuckDB parameters and the return type becomes the DuckDB return type. Nothing
else is needed — the function is registered automatically.

## Return shapes

| Shape | Meaning |
| --- | --- |
| `T` | Always a value. |
| `Option<T>` | `None` becomes SQL `NULL`. |
| `DuckOptionResult<T>` | `Ok(None)` becomes `NULL`, `Err(e)` fails the query. |

```rust
#[duck_scalar_function]
fn dfn_scalar_ret_plain(i: i32) -> i32 {
    i * 2
}

#[duck_scalar_function]
fn dfn_scalar_ret_option(i: i32) -> Option<i32> {
    if i == 0 { None } else { Some(100 / i) }
}

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

```sql
SELECT dfn_scalar_ret_plain(21);              -- 42
SELECT typeof(dfn_scalar_ret_plain(21));      -- INTEGER
SELECT dfn_scalar_ret_plain(NULL::INTEGER);   -- NULL

SELECT dfn_scalar_ret_option(4);              -- 25
SELECT dfn_scalar_ret_option(0);              -- NULL

SELECT dfn_scalar_ret_checked(4);             -- 25
SELECT dfn_scalar_ret_checked(-1);            -- NULL
SELECT dfn_scalar_ret_checked(0);             -- error: dfn_scalar_ret_checked: division by zero
```

A fourth shape, `Result<T, ExtensionError>`, is *not* accepted — use `DuckOptionResult<T>` and return
`Ok(Some(value))`.

## Arity

Any arity works, including zero:

```rust
#[duck_scalar_function]
fn dfn_scalar_arity_zero() -> i32 {
    42
}
```

```sql
SELECT dfn_scalar_arity_zero();            -- 42
SELECT dfn_scalar_arity_zero(1);           -- error: No function matches
```

Arguments map to parameters positionally, in the order they are declared.

## NULL handling

Whether `NULL` reaches your function body depends on how the argument is typed:

| Argument type | `NULL` in a column | Constant `NULL` |
| --- | --- | --- |
| `T` | The whole row short-circuits to `NULL`; the body is **not** called. | `NULL` |
| `Option<T>` | `None` is passed to the body. | Folded to `NULL` by DuckDB — the body is not called — unless `special_null_handling = true`. |

```rust
#[duck_scalar_function]
fn dfn_scalar_null_arg_plain(a: i32, b: i32) -> i32 { a + b }

#[duck_scalar_function]
fn dfn_scalar_null_arg_option(a: Option<i32>) -> i64 {
    a.map(i64::from).unwrap_or(-1)
}
```

```sql
SELECT dfn_scalar_null_arg_plain(1, 2);       -- 3
SELECT dfn_scalar_null_arg_plain(NULL, 2);    -- NULL, body not called
SELECT dfn_scalar_null_arg_plain(1, NULL);    -- NULL, body not called
SELECT dfn_scalar_null_arg_option(NULL);      -- -1, body called with None
```

### `special_null_handling`

DuckDB evaluates constant expressions at bind time: a literal `NULL` argument, or any expression
that folds to one such as `NULL::INTEGER + 0`, is folded to `NULL` before the callback runs. Setting
`special_null_handling = true` asks DuckDB not to fold, so the body still sees `None`:

```rust
#[duck_scalar_function]
fn dfn_scalar_null_handling_default(a: Option<i32>) -> i64 {
    a.map(i64::from).unwrap_or(-1)
}

#[duck_scalar_function(special_null_handling = true)]
fn dfn_scalar_null_handling_special(a: Option<i32>) -> i64 {
    a.map(i64::from).unwrap_or(-1)
}
```

```sql
SELECT dfn_scalar_null_handling_default(NULL::INTEGER);   -- NULL  (folded)
SELECT dfn_scalar_null_handling_special(NULL::INTEGER);   -- -1    (not folded)
SELECT dfn_scalar_null_handling_special(NULL::INTEGER + 0); -- -1
```

This is the only observable difference. For a column of values both settings behave identically, and
for a non-`Option` argument the flag changes nothing at all — the reader still short-circuits the row
to `NULL` before the body is reached.

### `volatile`

A volatile function is re-evaluated for every row even when it is called with the same arguments.
DuckDB may otherwise fold a constant-argument call into a single execution, which is wrong for
functions such as `random()`. `volatile = true` makes registration call
`duckdb_scalar_function_set_volatile`:

```rust
#[duck_scalar_function(volatile = true)]
fn dfn_scalar_volatile_random(seed: i32) -> i64 {
    i64::from(seed).wrapping_mul(2_654_435_761).wrapping_add(1)
}

#[duck_scalar_function(volatile = true, special_null_handling = true)]
fn dfn_scalar_volatile_special(a: Option<i32>) -> i64 {
    a.map(i64::from).unwrap_or(-1)
}
```

```sql
SELECT dfn_scalar_volatile_random(1);               -- 2654435762
SELECT typeof(dfn_scalar_volatile_random(1));       -- BIGINT
SELECT dfn_scalar_volatile_random(1) FROM range(3); -- re-evaluated for every row
SELECT dfn_scalar_volatile_special(NULL::INTEGER);  -- -1 (the constant NULL is not folded)
```

The switch requires duckfn's `duckdb-1-5` feature (the DuckDB 1.5.0+ C API); without it the flag is
ignored. It applies to standalone scalar functions only — quack-rs' `ScalarOverloadBuilder` exposes
no volatile switch, so combining `volatile = true` with `overloads_name` is rejected at compile
time. The functions above are deterministic, so their values do not depend on the flag; what
changes is how often DuckDB calls them.

### Variadic arguments

`varargs = true` declares the last parameter as `Vec<T>`, where `T` is the type of one variadic
argument. The macro hands `T`'s logical type to DuckDB's `duckdb_scalar_function_set_varargs` and
collects every column after the fixed ones into that `Vec<T>`:

```rust
#[duck_scalar_function(varargs = true)]
fn dfn_scalar_varargs_sum(values: Vec<i64>) -> i64 {
    values.iter().sum()
}

#[duck_scalar_function(varargs = true)]
fn dfn_scalar_varargs_join(sep: String, parts: Vec<Option<String>>) -> String {
    parts.into_iter().flatten().collect::<Vec<_>>().join(&sep)
}

#[duck_scalar_function(varargs = true)]
fn dfn_scalar_varargs_merge(lists: Vec<Vec<i64>>) -> Vec<i64> {
    lists.into_iter().flatten().collect()
}
```

```sql
SELECT dfn_scalar_varargs_sum(1, 2, 3);            -- 6
SELECT dfn_scalar_varargs_sum();                  -- 0  (zero variadic arguments)
SELECT dfn_scalar_varargs_join('-', 'a', 'b');    -- a-b
SELECT dfn_scalar_varargs_join('-', 'a', NULL);   -- NULL (the constant NULL is folded)
SELECT dfn_scalar_varargs_merge([1, 2], [3], []); -- [1, 2, 3]
SELECT typeof(dfn_scalar_varargs_merge([1]));     -- BIGINT[]
```

`T` may be any value type:

- `i64` → `BIGINT`;
- `Option<String>` → `VARCHAR`, with each variadic argument nullable (a `NULL` in a column arrives
  as `None`, while a constant `NULL` is still folded by DuckDB, exactly as for fixed arguments);
- `Vec<i64>` → `LIST(BIGINT)`, i.e. each variadic argument is a LIST itself — the same thing
  `varargs_logical(LogicalType::list(TypeId::BigInt))` does by hand in quack-rs.

A `NULL` in a non-nullable argument or element short-circuits the whole row to `NULL`, just like a
fixed non-`Option` argument. Zero variadic arguments are allowed. The switch requires duckfn's
`duckdb-1-5` feature (the DuckDB 1.5.0+ C API) and cannot be combined with `overloads_name`
(quack-rs' `ScalarOverloadBuilder` exposes no varargs switch); the macro rejects that combination
at compile time.

## Batch mode

Reading and writing a chunk are already batched; only your function is called per row. `batch = true`
hands the walking over to you: the function receives the whole batch of rows and returns the whole
batch of results. Use it when the per-row work can be merged — one HTTP request, one database round
trip — instead of one call per row.

The signature is not "a row of arguments" but "a batch of rows -> a batch of results". The row type
is your own `#[derive(DuckStruct)]` struct and *is* the argument type (no `DuckArgsImpl` struct is
generated), so the columns are exactly the fields of that struct:

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DfnBatchRow {
    pub id: i64,
    pub tag: String,
}

#[duck_scalar_function(batch = true)]
fn dfn_batch_join(rows: Vec<DfnBatchRow>) -> DuckOptionResult<Vec<String>> {
    let joined = rows.iter()
        .map(|row| format!("{}:{}", row.id, row.tag))
        .collect::<Vec<_>>()
        .join("|");
    Ok(Some(vec![joined; rows.len()]))
}
```

Two input shapes decide what happens to `NULL` rows:

| Parameter | `NULL` rows |
| --- | --- |
| `Vec<DfnBatchRow>` | Filtered out: the body only sees the non-NULL rows, and the results are spliced back at their original positions with `NULL`. |
| `Vec<Option<DfnBatchRow>>` | Delivered to the body as `None`; the function decides their results itself. |

```rust
#[duck_scalar_function(batch = true)]
fn dfn_batch_tag_len(rows: Vec<Option<DfnBatchRow>>) -> DuckOptionResult<Vec<Option<i64>>> {
    Ok(Some(rows.iter().map(|row| row.as_ref().map(|row| row.tag.len() as i64)).collect()))
}
```

Four return shapes mirror the per-row ones:

| Shape | Meaning |
| --- | --- |
| `Vec<T>` | One non-`NULL` value per row. |
| `Vec<Option<T>>` | One possibly-`NULL` value per row. |
| `DuckOptionResult<Vec<T>>` | `Ok(None)` makes the **whole batch** `NULL`; `Err(e)` fails the query. |
| `DuckOptionResult<Vec<Option<T>>>` | The nullable flavour of the previous shape. |

```sql
SELECT dfn_batch_join(id, tag) FROM (VALUES (1, 'a'), (NULL, 'b'), (2, 'c')) t(id, tag);
-- 1:a|2:c, NULL, 1:a|2:c   (the NULL row was filtered out, then filled back in)

SELECT dfn_batch_tag_len(id, tag) FROM (VALUES (1, 'aa'), (NULL, 'bbb'), (2, '')) t(id, tag);
-- 2, NULL, 0               (the NULL row arrived as None)
```

Notes:

- The batch is **one chunk**, not one table: DuckDB still calls the callback once per data chunk, so
  a large table is never materialised in memory and the vectorised read/write paths stay in place.
  `fn dfn_batch_batch_size(rows: Vec<DfnBatchRow>) -> Vec<i64>` returns `rows.len()` for every row,
  so `SELECT DISTINCT dfn_batch_batch_size(i, 'x') FROM range(5000)` shows the chunk sizes.
- The returned row count must equal the number of rows the function received (for `Vec<DfnBatchRow>`
  that is the number of **non-NULL** rows, since the NULL rows never reached the body). A mismatch
  fails the query with `batch function returned N rows, but received M input rows` instead of writing
  garbage.
- A per-row `None` (`Vec<Option<T>>`) only nulls that row, while `Ok(None)` from
  `DuckOptionResult<Vec<_>>` nulls the whole batch — do not confuse the two.
- `batch = true` cannot be combined with `varargs = true` (variadic arguments have no stable row
  structure); the macro rejects the combination at compile time. `overloads_name`,
  `auto_register`, `special_null_handling` and `volatile` work as usual.
- `NULL` handling is otherwise unchanged: a `NULL` in a non-nullable field still invalidates the
  whole row (that is exactly what the filtered shape reacts to), and an `Option` field still arrives
  as `None`.

## Overloads and function sets

Several signatures can share one SQL name. The simplest way is `overloads_name`, which merges every
signature carrying the same value into one function set — each overload keeps its own return type:

```rust
#[duck_scalar_function(overloads_name = "dfn_scalar_ovl_set")]
fn dfn_scalar_ovl_int(i: i32) -> String {
    format!("int:{i}")
}

#[duck_scalar_function(overloads_name = "dfn_scalar_ovl_set")]
fn dfn_scalar_ovl_varchar(s: String) -> i64 {
    s.len() as i64
}

#[duck_scalar_function(overloads_name = "dfn_scalar_ovl_set")]
fn dfn_scalar_ovl_int_int(a: i32, b: i32) -> i64 {
    i64::from(a) * i64::from(b)
}
```

```sql
SELECT dfn_scalar_ovl_set(1);         -- int:1
SELECT typeof(dfn_scalar_ovl_set(1)); -- VARCHAR
SELECT dfn_scalar_ovl_set('abcd');    -- 4
SELECT dfn_scalar_ovl_set(3, 4);      -- 12
```

The branch functions are only reachable through the function set; `dfn_scalar_ovl_int(1)` on its own
does not exist. To pick the overloads yourself — and to choose the registration name — set
`auto_register = false` and register a
[`ScalarFunctionSetBuilder`](./attributes.md#automatic-vs-manual-registration) instead.

## Named parameters

DuckDB accepts `name := value` syntax for scalar arguments, but duckfn registers scalar functions by
position, so the names are ignored and values bind in the order written:

```sql
SELECT dfn_scalar_reg_named_param(1, 2);          -- 12
SELECT dfn_scalar_reg_named_param(a := 1, b := 2); -- 12
SELECT dfn_scalar_reg_named_param(b := 2, a := 1); -- 21  (a gets 2, b gets 1)
SELECT dfn_scalar_reg_named_param(x := 1, y := 2); -- 12  (unknown names are not rejected)
```

`named_param_from` is a table-function-only key and is rejected by the other attribute macros;
scalar functions are always registered positionally, so the `name := value` names are ignored.

## Source and tests

- [`src/extension/functions/scalar_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/scalar_function.rs) — the example functions
- [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) — the expected results
- [`test/sql/functions/scalar_function_batch.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function_batch.test) — the batch-mode expected results
- [`duckfn/src/functions/scalar_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/scalar_function_adapter.rs) — the runtime side

## Next

- [Aggregate functions](./aggregate-functions.md)
- [Errors and panics](./errors-and-panics.md) — what happens when a body panics.
- [Attributes](./attributes.md) — the generated module and its builders.
