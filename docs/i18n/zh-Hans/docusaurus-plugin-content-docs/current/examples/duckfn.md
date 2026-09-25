---
title: 示例扩展
sidebar_position: 1
description: duckfn —— 随 duckfn 一起发布的示例扩展，用可运行的 SQL 覆盖 duckfn 的每一项功能。
---

# 示例扩展

示例扩展是 `duckfn` 包自己的一部分：模块树在 `src/extension/`，`src/wasm_lib.rs` 与
`src/bin/duckfn.rs` 分别是它的 WebAssembly 与命令行入口。它不单独发布，存在的意义是把每一项功能都跑
一遍；`test/sql/` 下的 sqllogictest 用例则是各函数行为的权威依据。由于它**随已发布的 `duckfn` 包一起
分发**，不用克隆仓库就能读到它的源码与测试。

## 构建与加载

```bash
make configure   # 只做一次：准备测试运行器所需的 Python venv
make debug       # -> build/debug/extension/duckfn/duckfn.duckdb_extension
```

示例只在打开 `quack` feature 时才参与编译，所以所有构建它的命令都会带上 `--features quack`：
`make debug` 靠根 `Makefile` 带、`just build` 在 Justfile 里带（顺带把结果打包成可加载的
`duckfn.duckdb_extension`）。裸跑 `cargo build` 只会得到一个没有入口符号的 cdylib，DuckDB 在
`LOAD` 时会拒绝它。

```bash
duckdb -unsigned -c "
LOAD './build/debug/extension/duckfn/duckfn.duckdb_extension';
SELECT rusty_echo('Jane');
"
```

开发时用 `just sql "<SQL>"` 可以一步完成重新构建并执行语句，`just test` 跑完整的 sqllogictest 套件。

## 源码组织

| 路径 | 内容 |
| --- | --- |
| `src/extension/mod.rs` | 模块树 —— 入口不在这个文件里。 |
| `src/extension/entry.rs` | `duckfn_entrypoint!("duckfn")`；单独一个文件，CLI 才能只编模块树、不重复定义入口符号。 |
| `src/extension/demo/` | 每个功能域一个文件，另有若干手写 FFI 版本用于对照。 |
| `src/extension/functions/` | 每种注册方式一个文件：标量、聚合、表函数、类型转换、替换扫描、SQL 宏。 |
| `src/extension/functions/sql/` | 通过 `include_str!` 与 `duck_sql_macro_files!` 注册的 `.sql` 文件。 |
| `src/extension/types/` | 每种受支持类型的 echo 函数，标量与表函数两种形式。 |
| `test/sql/` | 41 个 sqllogictest 文件，与源码目录一一对应。 |

`src/extension/demo/rewrite_official_template_demo.rs` 是最小的可用起点 —— 它是把 DuckDB 官方的模板示例
用 duckfn 重写了一遍：

```rust
#[duck_scalar_function]
fn rusty_echo(s: String) -> String {
    format!("🐤 {s} 🦀 {s}")
}

#[derive(Clone, Debug, DuckStruct)]
pub struct RustyQuackResult {
    column0: String,
}

#[duck_table_function]
fn rusty_quack(name: String) -> impl Iterator<Item = RustyQuackResult> {
    vec![RustyQuackResult {
        column0: format!("Rusty Quack {} 🐥", name),
    }]
    .into_iter()
}
```

```sql
SELECT rusty_echo('Hello');        -- 🐤 Hello 🦀 Hello
SELECT * FROM rusty_quack('Sam');  -- Rusty Quack Sam 🐥
```

## 标量、列表、映射与结构体

demo 模块覆盖了函数可以取的形态，包括嵌套的输入与输出：

```sql
SELECT double_it5(21);                               -- 42
SELECT first_word_tuple('hello world');              -- hello
SELECT sum_list_w([1, 2, 3, 4]);                      -- 10
SELECT sum_list_nest([[1, 2], [3, null, 4], null]);   -- 10
SELECT struct_scalar_w({hello_count: 15});            -- 25
SELECT struct_nest_scalar_w({structf: {hello_count: 15}, list: [1, null, 2]});  -- 18
SELECT input_map_demo(MAP {'key1': [10], 'key2': [20, 5], 'key3': null});       -- 35
SELECT CAST(input_array_demo(a) AS VARCHAR) FROM (VALUES (ARRAY[1, 2]), (ARRAY[4, null])) t(a);
-- [1, 2]
-- [4, NULL]
```

返回结构化数据同理：`make_list_scalar_w(range)` 返回 `LIST`，`nest_list_scalar_w(range)` 返回嵌套 `LIST`，
`struct_nest_output_scalar_w(range::int)` 返回嵌套 `STRUCT`。

## 错误与 NULL 的实战

`error_scalar_demo` 对同一个入参可以返回值、`NULL` 或错误：

```sql
SELECT error_scalar_demo(3);   -- 6
SELECT error_scalar_demo(10);  -- 报错：input is 10
SELECT error_scalar_demo(20);  -- 报错：panic: input is 20
SELECT error_scalar_demo(30);  -- 报错：explicit panic
```

## 聚合函数

```sql
SELECT word_count_m(sentence)
FROM (VALUES ('hello world'), ('  padded  '), (''), (NULL)) t(sentence);
-- 3

SELECT range % 3 AS g, agg_list_w(range)
FROM range(9)
GROUP BY g;
-- 0  [0, 3, 6]
-- 1  [1, 4, 7]
-- 2  [2, 5, 8]
```

`word_count_w` 通过 `#[duck_custom_register]` 与 `WordCountStateWrapper` 手动注册；`agg_list_w` 把行收集成
`LIST`，遇到值 `12` 时让查询失败。

## 表函数与命名参数

```sql
SELECT * FROM count_down_m_simple(start=12);   -- 11, 10, 9, … 0

SELECT * FROM bind_map_demo(MAP {'key1': [10], 'key2': [20, 5], 'key3': null});
-- 10
-- 25
--  0
```

## 类型转换与替换扫描

```sql
SELECT CAST('42' AS INTEGER);        -- 42
SELECT TRY_CAST('abc' AS INTEGER);   -- NULL
SELECT CAST('abc' AS INTEGER);       -- 报错：not an integer: "abc"

SELECT * FROM '3.points';            -- x 0 y 0 / 1 1 / 2 4
SELECT * FROM 'hi.echo';             -- hi.echo  7
```

## SQL 宏

```sql
SELECT clamp(range, 4, 7) FROM range(9);
-- 4, 4, 4, 4, 4, 5, 6, 7, 7

SELECT add_two_v1(1), add_two_v2(2), add_two_v3(3), add_two_v4(4);
-- 3, 4, 5, 6
```

四个 `add_two_*` 是同一个宏的四种返回形态：`SqlMacro`、`DuckResult<SqlMacro>`、`DuckResult<String>` 与 `String`。
`dfn_macro_inc_*` 系列来自 `include_str!`，`dfn_macro_files_*` 来自 `duck_sql_macro_files!`。

## 类型 echo

每种受支持类型都有标量与表函数两种形式的恒等函数：

```sql
SELECT dfn_echo_integer(42);             -- 42
SELECT dfn_echo_date(DATE '2024-01-02'); -- 2024-01-02
SELECT CAST(dfn_echo_list_integer_n([1, NULL, 3]) AS VARCHAR);  -- [1, NULL, 3]
SELECT CAST(v AS VARCHAR) FROM dfn_table_echo_bool(true, count => 3);
-- true
-- NULL
-- true
```

命名规律固定：标量是 `dfn_echo_<type>`，表函数是 `dfn_table_echo_<type>`，其中 `<type>` 形如 `integer`、
`list_integer`、`map_varchar_integer`、`array_bigint`、`struct_with_list` 等。

## 测试套件

`test/sql/` 与源码目录一一对应，是关于行为最精确的描述：

```
test/sql/
├── demo/        与 demo 目录同名的一系列用例，外加 rusty_echo、rusty_quack、sql_lang_demo
├── functions/   scalar_function、aggregate_function、table_function、cast_function、
│                replacement_scan、sql_macro
└── types/       每种受支持类型的 <type>_scalar_echo 与 <type>_table_echo
```

```bash
make configure debug test   # 或者：just test
```

## 接下来

- [同样功能的两种写法](./side-by-side.md) —— `rusty_echo`、`rusty_quack`、`word_count`、`first_word` 与它们用原始 `duckdb` / `quack-rs` 写法的对照。
- [快速开始](../getting-started/quick-start.md) —— 在一个最小 crate 上重复这些思路。
- [构建与发布](../build-and-release.md) —— 这个扩展是如何打包与发布的。
