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

### 可变参数

`varargs = true` 把函数签名的最后一个参数声明成 `Vec<T>`，其中 `T` 是「单个可变参数」的类型。
宏把 `T` 的逻辑类型交给 DuckDB 的 `duckdb_scalar_function_set_varargs`，并把固定参数之后的每一列
读成该类型、收进这个 `Vec<T>`：

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
SELECT dfn_scalar_varargs_sum();                  -- 0（零个可变参数）
SELECT dfn_scalar_varargs_join('-', 'a', 'b');    -- a-b
SELECT dfn_scalar_varargs_join('-', 'a', NULL);   -- NULL（常量 NULL 被折叠）
SELECT dfn_scalar_varargs_merge([1, 2], [3], []); -- [1, 2, 3]
SELECT typeof(dfn_scalar_varargs_merge([1]));     -- BIGINT[]
```

`T` 可以是任意值类型：

- `i64` → `BIGINT`；
- `Option<String>` → `VARCHAR`，每个可变参数都可空（列里的 `NULL` 以 `None` 进入函数体，而常量
  `NULL` 仍会被 DuckDB 折叠 —— 与固定参数的行为一致）；
- `Vec<i64>` → `LIST(BIGINT)`，即每个可变参数本身就是一个 LIST，等价于在 quack-rs 里手写
  `varargs_logical(LogicalType::list(TypeId::BigInt))`。

任一非可空参数或元素为 `NULL` 时整行短路成 `NULL`，与固定的非 `Option` 参数一致；零个可变参数也是
合法的。该开关需要 duckfn 的 `duckdb-1-5` feature（DuckDB 1.5.0+ 的 C API），且不能与
`overloads_name` 同用（quack-rs 的 `ScalarOverloadBuilder` 没有暴露 varargs 开关），宏会在编译期
拒绝这种组合。

## 批量模式

读值和写值本来就都是按批的，逐行的只有用户函数。`batch = true` 把「遍历整批」这一步也交给你：
函数一次收到整批行、一次还回整批结果。适用场景是「行级逻辑，但可以合并成一次请求」—— 一次 HTTP
调用、一次数据库往返，而不是每行各来一次。

这时签名不再是「一行参数」，而是「整批行 -> 整批结果」。行类型就是你自己用 `#[derive(DuckStruct)]`
定义的结构体，它直接充当参数类型（宏不再生成 `DuckArgsImpl`），字段即列：

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

入参有两种形态，决定空行怎么处理：

| 参数 | 空行 |
| --- | --- |
| `Vec<DfnBatchRow>` | 先被剔掉：函数体只看到非空行，结果再按原位回填 `NULL`。 |
| `Vec<Option<DfnBatchRow>>` | 以 `None` 交给函数体，由函数自己决定这些行的结果。 |

```rust
#[duck_scalar_function(batch = true)]
fn dfn_batch_tag_len(rows: Vec<Option<DfnBatchRow>>) -> DuckOptionResult<Vec<Option<i64>>> {
    Ok(Some(rows.iter().map(|row| row.as_ref().map(|row| row.tag.len() as i64)).collect()))
}
```

返回形态有四种，与逐行版本一一对应：

| 形态 | 含义 |
| --- | --- |
| `Vec<T>` | 每行一个非 `NULL` 的值。 |
| `Vec<Option<T>>` | 每行一个可空值。 |
| `DuckOptionResult<Vec<T>>` | `Ok(None)` 让**整批**变成 `NULL`，`Err(e)` 让整条查询失败。 |
| `DuckOptionResult<Vec<Option<T>>>` | 上一种的可空版本。 |

```sql
SELECT dfn_batch_join(id, tag) FROM (VALUES (1, 'a'), (NULL, 'b'), (2, 'c')) t(id, tag);
-- 1:a|2:c, NULL, 1:a|2:c   （空行被剔掉，再按原位回填）

SELECT dfn_batch_tag_len(id, tag) FROM (VALUES (1, 'aa'), (NULL, 'bbb'), (2, '')) t(id, tag);
-- 2, NULL, 0               （空行以 None 到达函数体）
```

几点说明：

- 一批就是一个 chunk，不是一张表：DuckDB 仍然按数据块调用回调，整表不会被拉进内存，读写两侧的
  向量化路径原样保留。`fn dfn_batch_batch_size(rows: Vec<DfnBatchRow>) -> Vec<i64>` 让每行返回
  `rows.len()`，于是 `SELECT DISTINCT dfn_batch_batch_size(i, 'x') FROM range(5000)` 能看出块大小。
- 返回的行数必须与函数收到的行数一致（`Vec<DfnBatchRow>` 形态下是**非空行**的行数，因为空行根本没
  进函数体）。不一致会让整条查询失败并报出 `batch function returned N rows, but received M input rows`，
  而不是写出错位的数据。
- 逐行的 `None`（`Vec<Option<T>>`）只让那一行变成 `NULL`，而 `DuckOptionResult<Vec<_>>` 的
  `Ok(None)` 会让整批变成 `NULL` —— 两者不要混淆。
- `batch = true` 不能与 `varargs = true` 同用（可变参数没有稳定的行结构），宏会在编译期拒绝；
  `overloads_name`、`auto_register`、`special_null_handling`、`volatile` 都照常生效。
- 其余 NULL 语义完全不变：非可空字段为 `NULL` 仍然让整个参数值作废（过滤形态正是据此剔除空行），
  可空字段仍然以 `None` 进入函数体。

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

`named_param_from` 是表函数专用的键，其它属性宏会直接拒绝它；标量函数始终按位置注册，因此 `名字 := 值` 里的名字会被忽略。

## 源码与测试

- [`src/extension/functions/scalar_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/scalar_function.rs) —— 示例函数
- [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) —— 期望结果
- [`test/sql/functions/scalar_function_batch.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function_batch.test) —— 批量模式的期望结果
- [`duckfn/src/functions/scalar_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/scalar_function_adapter.rs) —— 运行时侧

## 接下来

- [聚合函数](./aggregate-functions.md)
- [错误与 panic](./errors-and-panics.md) —— 函数体 panic 时会发生什么。
- [属性参考](./attributes.md) —— 生成的模块与其中的 builder。
