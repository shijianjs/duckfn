---
title: 类型转换
sidebar_position: 7
description: 覆盖 DuckDB 某一对源类型/目标类型的 CAST 行为，包括 TRY_CAST 与隐式转换代价。
---

# 类型转换

类型转换函数只接收一个参数 —— 源值 —— 返回类型就是目标类型。两者都从签名推断：

```rust
#[duck_cast_function]
fn dfn_cast_str_to_int(s: String) -> DuckOptionResult<i32> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        // 覆盖内置行为：内置的 CAST('' AS INTEGER) 会报错。
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| {
        duck_error(format!("dfn_cast_str_to_int: not an integer: {s:?}"))
    })
}
```

```sql
SELECT CAST('42' AS INTEGER);       -- 42
SELECT CAST(' 7 ' AS INTEGER);      -- 7（已 trim）
SELECT CAST('' AS INTEGER);         -- NULL（覆盖了内置报错行为）
SELECT TRY_CAST('abc' AS INTEGER);  -- NULL
SELECT CAST('abc' AS INTEGER);      -- 报错：dfn_cast_str_to_int: not an integer: "abc"
```

最后两行是同一个函数走了两条不同的转换路径：

| 路径 | 出错时 |
| --- | --- |
| `CAST(x AS T)` | 报告错误，整条查询失败。 |
| `TRY_CAST(x AS T)` | 该行变成 `NULL`，查询继续。 |

## NULL 的处理

沿用常规的参数规则：写成 `T` 时整行短路，写成 `Option<T>` 时函数体能拿到 `NULL`。

```rust
#[duck_cast_function]
fn dfn_cast_bigint_to_double(v: Option<i64>) -> Option<f64> {
    match v {
        Some(v) => Some(v as f64 / 2.0),
        None => Some(-1.0),   // NULL 进入了函数体
    }
}
```

```sql
SELECT CAST(3::BIGINT AS DOUBLE);     -- 1.5
SELECT CAST(NULL::BIGINT AS DOUBLE);  -- -1.0
```

## 类型

[类型映射](./types.md)里的任意类型都可以用在两侧，容器也一样：

```sql
SELECT CAST(CAST(['1', '2'] AS INTEGER[]) AS VARCHAR);  -- [1, 2]
SELECT TRY_CAST(['1', 'x'] AS INTEGER[]);               -- NULL
SELECT CAST(['1', 'x'] AS INTEGER[]);                   -- 报错：not an integer: "x"
```

为 DuckDB 本来就能处理的一对类型注册转换，会**替换**内置行为 —— 所以上面的 `CAST('' AS INTEGER)` 得到的是
`NULL` 而不是报错。

## 隐式转换代价

设置 `implicit_cost` 可以让 DuckDB 以指定代价把该转换用于隐式转换：

```rust
/// VARCHAR -> HUGEINT，可作为隐式转换使用，代价 100。
#[duck_cast_function(implicit_cost = 100)]
fn dfn_cast_str_to_hugeint(s: String) -> i128 { /* … */ }
```

```sql
SELECT CAST('41' AS HUGEINT);              -- 41
SELECT CAST('41' AS VARCHAR) + 1::HUGEINT; -- 42
```

## 源码与测试

- [`src/extension/functions/cast_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/cast_function.rs) —— 示例转换函数
- [`test/sql/functions/cast_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/cast_function.test) —— 期望结果
- [`src/functions/cast_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/cast_function_adapter.rs) —— 运行时侧

## 接下来

- [替换扫描](./replacement-scans.md)
- [类型映射](./types.md)
