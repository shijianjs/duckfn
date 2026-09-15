---
title: 类型转换与 replacement scan
sidebar_position: 5
description: 覆盖 DuckDB 的 CAST 行为，以及让 SELECT * FROM 'data.points' 解析到你自己的表函数。
---

# 类型转换与 replacement scan

## 类型转换

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

`NULL` 的处理遵循常规的参数规则：写成 `Option<T>` 时函数体能拿到 `NULL`：

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

支持的返回形态与标量函数相同（`T`、`Option<T>`、`DuckOptionResult<T>`）；两侧都可以用复杂类型，
例如 `Vec<Option<String>>` 转 `Vec<Option<i32>>`：

```sql
SELECT CAST(CAST(['1', '2'] AS INTEGER[]) AS VARCHAR);  -- [1, 2]
SELECT TRY_CAST(['1', 'x'] AS INTEGER[]);               -- NULL
SELECT CAST(['1', 'x'] AS INTEGER[]);                   -- 报错：not an integer: "x"
```

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

## Replacement scan

DuckDB 找不到某张表时会把表名交给 replacement scan —— 通常是一个文件路径。回调返回要转调的**表函数名**，
duckfn 会把原始名称作为第一个 `VARCHAR` 参数传给它。

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

转调目标就是一个普通的表函数：

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
SELECT * FROM dfn_scan_read_points('2.points');  -- 同一个表函数，直接调用
SELECT * FROM 'nope.txt';                        -- 报错：Table with name nope.txt does not exist
```

### 返回形态

| 形态 | 含义 |
| --- | --- |
| `Option<String>` | `Some(table_fn)` 接管，`None` 表示不处理。 |
| `Option<&'static str>` | 同理，名字是静态字符串。 |
| `DuckOptionResult<String>` | 多了报错的能力。 |
| `DuckOptionResult<&'static str>` | 多了报错能力，名字是静态字符串。 |

返回 `Err` 会让查询失败；回调里的 panic 同样会被捕获并转成查询错误。

:::caution 两条规则
1. 回调会对**每一个**未解析的表名执行，所以不处理的输入必须返回 `Ok(None)`，不要接管不属于自己的名字。
2. 匹配逻辑由你自己写：示例是大小写敏感的，`'3.POINTS'` 会留给 DuckDB 并报
   `Table with name 3.POINTS does not exist`。非 UTF-8 的表名会原样放行。

如果返回的表函数不存在，查询会报 `Table Function with name dfn_scan_no_such_function does not exist`。
:::

## 接下来

- [SQL 宏](./sql-macros.md)
- [表函数](./table-functions.md) —— scan 转调的目标长什么样。
