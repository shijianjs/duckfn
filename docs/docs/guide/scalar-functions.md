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

`named_param_from` only affects table functions; on a scalar function it has no observable effect.

## Source and tests

- [`src/extension/functions/scalar_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/scalar_function.rs) — the example functions
- [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) — the expected results
- [`duckfn/src/functions/scalar_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/scalar_function_adapter.rs) — the runtime side

## Next

- [Aggregate functions](./aggregate-functions.md)
- [Errors and panics](./errors-and-panics.md) — what happens when a body panics.
- [Attributes](./attributes.md) — the generated module and its builders.
