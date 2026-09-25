---
title: 错误与 panic
sidebar_position: 10
description: 如何报告查询错误、如何返回 NULL，以及 Rust panic 如何变成 DuckDB 错误。
---

# 错误与 panic

## 错误

`DuckOptionResult<T>` 就是 `Result<Option<T>, ExtensionError>`，它让一个函数同时具备三种结果：

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

| 返回值 | SQL 侧表现 |
| --- | --- |
| `Ok(Some(v))` | `v` |
| `Ok(None)` | `NULL` |
| `Err(e)` | 查询失败，错误信息为 `e` 的内容。 |

```sql
SELECT dfn_scalar_ret_checked(4);    -- 25
SELECT dfn_scalar_ret_checked(-1);   -- NULL
SELECT dfn_scalar_ret_checked(0);    -- 报错：dfn_scalar_ret_checked: division by zero
```

`duck_error("…")` 用于构造错误。`ExtensionError` 即 `quack_rs::error::ExtensionError`，因此 `?` 也能直接
传播 `quack-rs` API 的错误；需要把变量放进消息里时用 `format!`。

示例扩展里通行的两条约定，值得照做：

- 消息以函数名开头，出错时一眼能看出源头：`dfn_cast_str_to_int: not an integer: "abc"`、
  `dfn_table_checked: n must be >= 0`。
- `Ok(None)` 留给「这一行没有值」，真正的失败用 `Err`。

需要注意，`Result<T, ExtensionError>` **不是**宏接受的返回形态 —— 请用 `DuckOptionResult<T>`，
把成功的值包进 `Some`。

## panic

duckfn 函数里的 panic 不会跨 FFI 边界展开，而是被捕获并作为查询错误报告：

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
SELECT dfn_scalar_ret_panic(13);   -- 报错：unlucky input: 13
```

各类注册方式都一样：

| 类型 | 捕获 panic 的位置 | 示例 |
| --- | --- | --- |
| 标量函数 | 函数体 | `SELECT dfn_scalar_ret_panic(13);` |
| 聚合函数 | 行处理函数 | `SELECT dfn_agg_panic(x) FROM (VALUES (13)) t(x);` |
| 类型转换 | 函数体 | `SELECT CAST('NaN'::DOUBLE AS BIGINT);` → `dfn_cast_double_to_bigint: not a finite number: NaN` |
| 表函数 | 迭代器以及 bind 阶段 | `SELECT * FROM dfn_table_full(5);` → `dfn_table_full: bad row 2` |
| 替换扫描 | 回调 | `SELECT * FROM 'boom.panic';` → `dfn_scan_points: panic while handling boom.panic` |

:::caution[panic 是安全网，不是控制流]
被捕获只能保证进程不崩，它仍然会终止整条查询，而且丢失了你本可以自行选择的错误类型。
请优先使用 `Ok(None)` 与 `Err(duck_error(…))`。
:::

## 各类失败的暴露位置

**表函数**：返回形态决定失败在什么时候被报告：

| 失败类型 | 形态 | 结果 |
| --- | --- | --- |
| 参数不合法 | `DuckResult<impl Iterator<Item = Row>>` | 在任何行产出之前查询就失败。 |
| 某行是 `NULL` | `DuckFullIteratorResult` 里的 `Ok(None)` | 该行所有列都是 `NULL`。 |
| 某行无法产出 | `DuckFullIteratorResult` 里的 `Err(…)` | 查询失败。 |

**类型转换**：同一个 `Err` 会随进入方式不同而不同：

| SQL | 结果 |
| --- | --- |
| `CAST('abc' AS INTEGER)` | 查询失败。 |
| `TRY_CAST('abc' AS INTEGER)` | 该行为 `NULL`，查询继续。 |

**聚合函数**：行处理函数返回的 `Err` 会让查询失败；这里没有逐行的 `NULL` 通道，因为一行只是更新状态。

## 源码与测试

- [`duckfn-quack/test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/functions/scalar_function.test) —— `duck_error`、`Ok(None)` 与 panic 用例
- [`duckfn-quack/test/sql/functions/aggregate_function.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/functions/aggregate_function.test) —— 聚合函数的同类用例
- [`duckfn-quack/test/sql/functions/cast_function.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/functions/cast_function.test) —— `CAST` 与 `TRY_CAST` 的差别
- [`src/functions/table_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/table_function_adapter.rs) —— bind 与 scan 里捕获 panic 的位置

## 接下来

- [属性参考](./attributes.md) —— 各宏接受的返回形态。
- [示例扩展](../examples/duckfn-quack.md) —— 这些错误路径的可运行 SQL。
