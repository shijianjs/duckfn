---
title: 标量函数
sidebar_position: 2
description: duckfn 标量函数的返回形态、NULL 处理、参数个数、重载与命名参数。
---

# 标量函数

```rust
#[duck_scalar_function]
fn dfn_scalar_ret_plain(i: i32) -> i32 {
    i * 2
}
```

参数成为 DuckDB 的参数，返回类型成为 DuckDB 的返回类型，除此之外不需要任何额外代码 —— 函数会自动注册。

## 返回形态

| 形态 | 含义 |
| --- | --- |
| `T` | 永远返回一个值。 |
| `Option<T>` | `None` 变成 SQL 的 `NULL`。 |
| `DuckOptionResult<T>` | `Ok(None)` 变成 `NULL`，`Err(e)` 让整条查询失败。 |

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
SELECT dfn_scalar_ret_checked(0);             -- 报错：dfn_scalar_ret_checked: division by zero
```

第四种形态 `Result<T, ExtensionError>` **不被接受** —— 请用 `DuckOptionResult<T>` 并返回 `Ok(Some(value))`。

## 参数个数

任意个数都可以，包括零个：

```rust
#[duck_scalar_function]
fn dfn_scalar_arity_zero() -> i32 {
    42
}
```

```sql
SELECT dfn_scalar_arity_zero();            -- 42
SELECT dfn_scalar_arity_zero(1);           -- 报错：No function matches
```

参数按声明顺序与 SQL 参数位置一一对应。

## NULL 的处理

`NULL` 能否进入函数体，取决于参数的写法：

| 参数类型 | 列里的 `NULL` | 常量 `NULL` |
| --- | --- | --- |
| `T` | 整行短路为 `NULL`，函数体**不会执行**。 | `NULL` |
| `Option<T>` | 以 `None` 传入函数体。 | DuckDB 会折叠成 `NULL`、函数体不执行；写 `special_null_handling = true` 时则以 `None` 传入。 |

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
SELECT dfn_scalar_null_arg_plain(NULL, 2);    -- NULL，函数体未执行
SELECT dfn_scalar_null_arg_plain(1, NULL);    -- NULL，函数体未执行
SELECT dfn_scalar_null_arg_option(NULL);      -- -1，函数体以 None 被调用
```

### `special_null_handling`

DuckDB 会在 bind 阶段求值常量表达式：字面量 `NULL`，以及任何折叠成 `NULL` 的表达式（例如 `NULL::INTEGER + 0`），
都会在回调执行前被折叠成 `NULL`。写 `special_null_handling = true` 就是告诉 DuckDB 不要折叠，函数体仍能拿到 `None`：

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
SELECT dfn_scalar_null_handling_default(NULL::INTEGER);     -- NULL（被折叠）
SELECT dfn_scalar_null_handling_special(NULL::INTEGER);     -- -1（未折叠）
SELECT dfn_scalar_null_handling_special(NULL::INTEGER + 0); -- -1
```

这是该开关唯一能观察到的差异。对一列取值来说两种设置行为一致；而入参写成非 `Option` 类型时它完全不起作用 ——
读取层仍会先把该行短路成 `NULL`，函数体根本执行不到。

### `volatile`

volatile 函数即使被相同参数调用，也会对每一行重新求值。否则 DuckDB 可能把常量参数的调用折叠成只执行一次，
对 `random()` 这类函数来说就是错的。`volatile = true` 让注册时调用 `duckdb_scalar_function_set_volatile`：

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
SELECT dfn_scalar_volatile_random(1) FROM range(3); -- 每一行都重新求值
SELECT dfn_scalar_volatile_special(NULL::INTEGER);  -- -1（常量 NULL 未被折叠）
```

该开关需要 duckfn 的 `duckdb-1-5` feature（DuckDB 1.5.0+ 的 C API），未开启时会被忽略。它只适用于独立注册的
标量函数 —— quack-rs 的 `ScalarOverloadBuilder` 没有暴露 volatile 开关，因此 `volatile = true` 与
`overloads_name` 同时出现会在编译期直接报错。上面的函数是确定性的，取值不随开关变化，变的是 DuckDB 调用它的次数。

## 重载与函数集

多个签名可以共用一个 SQL 名字。最简单的方式是 `overloads_name`：取值相同的签名会被合并成一个函数集，
每个重载各自保留自己的返回类型：

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

这些分支函数只能通过函数集访问：单独调用 `dfn_scalar_ovl_int(1)` 并不存在。如果你想自己挑选重载、自己决定注册名，
就把 `auto_register = false` 与
[`ScalarFunctionSetBuilder`](./attributes.md#自动注册与手动注册) 组合起来。

## 命名参数

DuckDB 支持对标量参数使用 `名字 := 值` 的写法，但 duckfn 注册标量函数时用的是位置参数列表，因此这些名字会被忽略，
值按书写顺序绑定：

```sql
SELECT dfn_scalar_reg_named_param(1, 2);           -- 12
SELECT dfn_scalar_reg_named_param(a := 1, b := 2); -- 12
SELECT dfn_scalar_reg_named_param(b := 2, a := 1); -- 21（a 拿到 2，b 拿到 1）
SELECT dfn_scalar_reg_named_param(x := 1, y := 2); -- 12（不存在的名字也不会报错）
```

`named_param_from` 只对表函数有意义，对标量函数没有可观察效果。

## 源码与测试

- [`src/extension/functions/scalar_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/scalar_function.rs) —— 示例函数
- [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) —— 期望结果
- [`duckfn/src/functions/scalar_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/scalar_function_adapter.rs) —— 运行时侧

## 接下来

- [聚合函数](./aggregate-functions.md)
- [错误与 panic](./errors-and-panics.md) —— 函数体 panic 时会发生什么。
- [属性参考](./attributes.md) —— 生成的模块与其中的 builder。
