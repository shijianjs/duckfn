---
title: 社区扩展文档页
sidebar_position: 10
description: 为什么 duckfn 扩展需要 function_descriptions.csv、四列各是什么含义、怎么生成与提交。
---

# 社区扩展文档页

每个社区扩展在 <https://duckdb.org/community_extensions/extensions/…> 上都有一页简介，它是从
扩展二进制**生成**的：DuckDB 加载扩展，把 `duckdb_functions()` 在加载前后做一次差集，再把多出来
的东西渲染成页面。

问题就在最后一步 —— 页面上显示的是 catalog 里查到什么就显示什么，而 **DuckDB 的 C 扩展 API
没有设置函数 description 与 example 的接口**。它只提供
`duckdb_scalar_function_set_name`、`_set_return_type`、`_set_varargs`、`_set_volatile`、
`_set_special_handling`、`_set_extra_info`、`_set_bind`、`_set_function` 这些，**没有**
`duckdb_scalar_function_set_description`，也没有 `_add_example`。所以 duckfn 扩展没法把这类文本
带进 catalog，不加处理的话，页面上的「Added Functions」就是一串光秃秃的函数名。

出路是一份 CSV：`duckdb/community-extensions` 的 `scripts/generate_md.sh` 会去找
`extensions/<扩展名>/docs/function_descriptions.csv`，存在就用
`function_name == other.function` 做 LEFT JOIN，覆盖 `functions` 与 `functions_overloads` 两表的
`description` / `comment` / `examples` 列。匹配不上的行直接被忽略，所以这份文件纯粹是一张
「覆盖表」。

## 把文本写在属性上

duckfn 让你把这段文本写在它所描述的函数旁边：

```rust
use duckfn::duck_scalar_function;

/// 把 INTEGER 翻倍。
#[duck_scalar_function(
    description = "Doubles an INTEGER",
    comment = "NULL in, NULL out",
    examples = ["SELECT double_it(21)", "SELECT double_it(x) FROM t"]
)]
pub fn double_it(v: Option<i64>) -> Option<i64> {
    v.map(|x| x * 2)
}
```

三个键，都是可选的：

| 键 | CSV 列 | 说明 |
| --- | --- | --- |
| `description` | `description` | 一句话说明，页面上主要显示的就是它。 |
| `comment` | `comment` | 补充说明，与描述一起展示。 |
| `example` / `examples` | `example` | 单条示例（`example = "SELECT ..."`）或多条（`examples = ["...", "..."]`）。两者互斥，同时写是编译错误。 |

所有「会注册出函数」的属性都认这三个键：`#[duck_scalar_function]`、`#[duck_aggregate_function]`、
`#[duck_table_function]`、`#[duck_cast_function]`、`#[duck_copy_function]`、
`#[duck_copy_from_function]`、`#[duck_sql_macro]`。`#[duck_replacement_scan]` 与
`#[duck_custom_register]` 不支持 —— catalog 里没有以 Rust 函数名命名的对应条目。

这三个键不参与注册：它们只是被收集进一条 `inventory` 记录，导出 CSV 时才用上。

## 生成 CSV

```bash
just docs_csv                      # -> docs/function_descriptions.csv
just docs_csv_check                # 已提交的 CSV 过期时非零退出
```

`just docs_csv` 会先构建扩展，再带上 `DUCKFN_DUMP_FUNCTION_DESCRIPTIONS=<路径>` 用 duckdb 加载它，
duckfn 在注册的同时把文件写出来。`function` 列取自注册前后 `duckdb_functions()` 的真实差集，
因此它一定与社区扩展生成器做 JOIN 时用的名字一致 —— 包括那些由 `#[duck_custom_register]` 或
`duck_sql_macro_files!` 注册出来的函数（它们自己并不需要写描述）。没写描述的函数也会出现在
文件里，只是文本为空，这样骨架是完整的，还剩哪些没写一目了然。

不用 `just` 的话就是一条命令：

```bash
DUCKFN_DUMP_FUNCTION_DESCRIPTIONS=docs/function_descriptions.csv \
  duckdb -unsigned -c "LOAD './target/debug/my_ext.duckdb_extension';"
```

## 提交

这份文件是从 **community-extensions 仓**读的，不是你自己的仓：提 PR 加上
`extensions/<扩展名>/description.yml` 时，把
`extensions/<扩展名>/docs/function_descriptions.csv` 一起放进去。在自己仓里留一份副本便于重新
生成 —— `just docs_csv_check` 会在「新增了函数但没写描述」时提醒你。
