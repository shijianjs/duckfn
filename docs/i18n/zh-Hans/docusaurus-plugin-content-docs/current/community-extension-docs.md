---
title: 社区扩展文档页
sidebar_position: 10
description: 为什么 duckfn 扩展需要 function_descriptions.csv、四列各是什么含义、怎么生成与提交。
---

# 社区扩展文档页

每个社区扩展在
[duckdb.org/community_extensions](https://duckdb.org/community_extensions/list_of_extensions) 上都有一页
简介，它是从扩展二进制**生成**的：DuckDB 加载扩展，把 `duckdb_functions()` 在加载前后做一次差集，
再把多出来的东西渲染成页面。

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

文本本身有两点要知道：

- **换行在写出 CSV 时会被压成一个空格。** 生成的页面是一张 Markdown 表格，单元格里的换行会把表格行
  拆断；而且 DuckDB 的 `read_csv()` 本来就会把引号内字段里的裸换行读成 `\r\n`。逗号、双引号、非 ASCII
  都原样保留，只有换行会被压平。
- **大括号照写即可，转义是生成器的事。** `generate_md.sh` 会把这两列过一遍 `jekyll_format_function`
  宏，给每个 `{{` 和 `}}` 包上 `{% raw %}…{% endraw %}`，所以
  `example = "SELECT f('{{x}}')"` 这样写没问题。

这三个键不参与注册：它们只是被收集进一条 `inventory` 记录，导出 CSV 时才用上。

## 生成 CSV

```bash
cargo run --bin duckfn -- function_descriptions          # -> target/function_descriptions.csv
cargo run --bin duckfn -- function_descriptions --all    # -> target/function_descriptions_all.csv
```

习惯用 `just` 的话也可以：

```bash
just docs_csv
```

默认只给「写了 description / comment / example 之一」的函数出一行；加 `--all` 则把注册进去的
所有函数都写出来，没写文档的那几列留空，适合当「还差哪些没写」的清单看 —— 文件名会带上 `_all`
后缀，两个文件不会互相覆盖。输出路径固定是 `<项目根>/target/`，下游不用猜文件在哪。

整个导出是纯内存操作，只读编译期由宏记录下来的元数据：不加载扩展、不查 `duckdb_functions()`，
完全不牵扯 DuckDB。

### 在新项目里配好

两处，都可以从 [duckfn 仓库](https://github.com/shijianjs/duckfn) 抄：

1. `Cargo.toml` —— 打开 duckfn 的 `cli` feature：

   ```toml
   duckfn = { version = "x.y.z", features = ["duckdb-1-5", "cli"] }
   ```

2. `src/bin/duckfn.rs` —— CLI 入口：

   ```rust
   //! duckfn 命令行工具入口：cargo run --bin duckfn -- function_descriptions
   #[path = "../extension/mod.rs"]
   mod extension;

   fn main() -> std::process::ExitCode {
       duckfn::cli::run(env!("CARGO_MANIFEST_DIR"))
   }
   ```

   这里的 `#[path]` 是刻意为之：`#[duck_*]` 的元数据靠 `inventory` 的静态构造器收集，只有真正被
   链接进最终二进制的目标文件才会生效。只是依赖库的话，链接器可能把这些模块整块丢掉，导出的 CSV
   会是空的，而且不会报错。

## 提交

这份文件是从 **community-extensions 仓**读的，不是你自己的仓：提 PR 加上
`extensions/<扩展名>/description.yml` 时，把
`extensions/<扩展名>/docs/function_descriptions.csv` 一起放进去。在自己仓里留一份副本便于重新
生成 —— 新增函数后重跑一次命令即可。
