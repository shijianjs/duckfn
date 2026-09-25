---
title: 替换扫描
sidebar_position: 8
description: 把 DuckDB 找不到的表名（通常是文件路径）解析到你自己的表函数。
---

# 替换扫描

替换扫描（replacement scan）有机会接管 DuckDB 找不到的任何表名 —— 通常是一个文件路径，比如
`data.points`。回调返回要转调的**表函数名**，duckfn 会把原始名称作为第一个 `VARCHAR` 参数传给它。

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

## 返回形态

| 形态 | 含义 |
| --- | --- |
| `Option<String>` | `Some(table_fn)` 接管，`None` 表示不处理。 |
| `Option<&'static str>` | 同理，名字是静态字符串。 |
| `DuckOptionResult<String>` | 多了报错的能力。 |
| `DuckOptionResult<&'static str>` | 多了报错能力，名字是静态字符串。 |

返回 `Err` 会让查询失败；回调里的 panic 同样会被捕获并转成查询错误。

:::caution[两条规则]

1. 回调会对**每一个**未解析的表名执行，所以不处理的输入必须返回 `Ok(None)`，不要接管不属于自己的名字。
2. 匹配逻辑由你自己写：示例是大小写敏感的，`'3.POINTS'` 会留给 DuckDB 并报
   `Table with name 3.POINTS does not exist`。非 UTF-8 的表名会原样放行。

如果返回的表函数不存在，查询会报 `Table Function with name dfn_scan_no_such_function does not exist`。
:::

## 源码与测试

- [`src/extension/functions/replacement_scan.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/replacement_scan.rs) —— 回调与它转调的表函数
- [`test/sql/functions/replacement_scan.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/replacement_scan.test) —— 期望结果
- [`src/functions/replacement_scan_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/replacement_scan_adapter.rs) —— 运行时侧

## 接下来

- [类型映射](./types.md)
- [表函数](./table-functions.md) —— 替换扫描转调的目标长什么样。
