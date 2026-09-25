---
title: SQL 宏
sidebar_position: 6
description: 注册标量宏与表宏 —— 推荐把 SQL 写进 .sql 文件，编译期自动内联。
---

# SQL 宏

SQL 宏注册的是 SQL 而不是 Rust 代码。可接受的返回形态有四种，但要把宏维护得省心，最省事的做法是把 SQL
写进 `.sql` 文件，然后注册这些文件。

## 从 `.sql` 文件注册宏

`duck_sql_macro_files!` 接受任意多个路径，编译期把每个文件内联，并按顺序执行：

```rust
duck_sql_macro_files!(
    "sql/macro_files_a.sql",
    "sql/macro_files_b.sql",
    "sql/macro_files_c.sql"
);
```

```sql title="sql/macro_files_a.sql"
CREATE OR REPLACE MACRO dfn_macro_files_add(a, b) AS (a + b);

CREATE OR REPLACE MACRO dfn_macro_files_mul(a, b) AS (a * b);
```

```sql title="sql/macro_files_b.sql"
CREATE OR REPLACE MACRO dfn_macro_files_gen(n) AS TABLE SELECT * FROM range(n);
```

```sql title="sql/macro_files_c.sql"
CREATE OR REPLACE MACRO dfn_macro_files_negate(x) AS (-x);
```

```sql
SELECT dfn_macro_files_add(2, 3);      -- 5
SELECT dfn_macro_files_mul(4, 5);      -- 20
SELECT * FROM dfn_macro_files_gen(3);  -- 0, 1, 2
SELECT dfn_macro_files_negate(7);      -- -7
```

之所以说这是最便捷的方式：

- 宏就是真正的 SQL 文件，编辑器能高亮和检查；一个文件里可以定义任意多个宏 —— 标量宏与表宏并存，
  这是单个 `SqlMacro` 表达不了的。
- 路径相对调用宏的 `.rs` 文件解析，编译期由 `include_str!` 内联，因此路径写错是编译错误，
  而不是运行时少一个宏。
- 文件按书写顺序执行，后面的文件可以依赖前面定义的宏。

只有一个文件时，`include_str!` 能达到同样效果，且不需要宏：

```rust
#[duck_sql_macro]
pub fn dfn_macro_inc_script() -> &'static str {
    include_str!("sql/macro_inc.sql")
}
```

```sql title="sql/macro_inc.sql"
CREATE OR REPLACE MACRO dfn_macro_inc_add(a, b) AS (a + b);

CREATE OR REPLACE MACRO dfn_macro_inc_triple(x) AS (x * 3);

CREATE OR REPLACE MACRO dfn_macro_inc_gen(n) AS TABLE SELECT * FROM range(n);
```

```sql
SELECT dfn_macro_inc_add(2, 3);      -- 5
SELECT dfn_macro_inc_triple(4);      -- 12
SELECT * FROM dfn_macro_inc_gen(3);  -- 0, 1, 2
```

## 用 `SqlMacro` 构造宏

宏体很短、并且希望它和 Rust 代码放在一起时，可以直接构造。`SqlMacro` 来自 `quack-rs`：

```rust
#[duck_sql_macro]
pub fn dfn_macro_clamp() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_clamp", &["x", "lo", "hi"], "greatest(lo, least(hi, x))")
}
```

```sql
SELECT dfn_macro_clamp(12, 0, 10);         -- 10
SELECT dfn_macro_clamp(-5, 0, 10);         -- 0
SELECT typeof(dfn_macro_clamp(5, 0, 10));  -- INTEGER
```

`SqlMacro::scalar(name, params, body)` 与 `SqlMacro::table(name, params, sql)` 分别覆盖两类宏：

```rust
#[duck_sql_macro]
pub fn dfn_macro_gen() -> DuckResult<SqlMacro> {
    Ok(SqlMacro::table("dfn_macro_gen", &["n"], "SELECT * FROM range(n)"))
}
```

```sql
SELECT * FROM dfn_macro_gen(3);      -- 0, 1, 2
SELECT range FROM DFN_MACRO_GEN(2);  -- 0, 1（宏名大小写不敏感）
```

宏体也不限于标量表达式 —— `dfn_macro_pair` 返回 `STRUCT`，`dfn_macro_mklist` 返回 `LIST`：

```sql
SELECT CAST(dfn_macro_pair(5) AS VARCHAR);   -- {'a': 5, 'b': 10}
SELECT typeof(dfn_macro_pair(5));            -- STRUCT(a INTEGER, b INTEGER)
SELECT dfn_macro_pair(5).a;                  -- 5
SELECT CAST(dfn_macro_mklist(5) AS VARCHAR); -- [5, 10]
```

## 直接返回 SQL 字符串

返回字符串会原样执行，这也是直接使用 `CREATE MACRO` 的方式：

```rust
#[duck_sql_macro]
pub fn dfn_macro_double() -> String {
    "CREATE OR REPLACE MACRO dfn_macro_double(x) AS (x * 2)".to_string()
}
```

```sql
SELECT dfn_macro_double(21);  -- 42
```

一个字符串里可以放用 `;` 分隔的多条语句，因此一个函数就能定义有依赖关系的宏：

```rust
#[duck_sql_macro]
pub fn dfn_macro_quad() -> DuckResult<String> {
    Ok("
        CREATE OR REPLACE MACRO dfn_macro_double(x) AS (x * 2);
        CREATE OR REPLACE MACRO dfn_macro_quad(x) AS (dfn_macro_double(dfn_macro_double(x)));
    ".to_string())
}
```

```sql
SELECT dfn_macro_quad(3);  -- 12
```

可接受的四种返回形态是：`SqlMacro`、`DuckResult<SqlMacro>`、SQL 字符串（`String` 或 `&'static str`），
以及 SQL 字符串的 `DuckResult`。

## 错误

宏名按大小写不敏感查找，出错时都是普通的 DuckDB 错误：

| 错误用法 | 报错 |
| --- | --- |
| 参数个数不对 | `dfn_macro_add does not support the supplied arguments` |
| 把表宏当标量函数用 | `dfn_macro_gen is a table function but it was used as a scalar function` |
| 把标量宏用在 `FROM` 后 | `Table Function with name dfn_macro_add does not exist` |

## 源码与测试

- [`src/extension/functions/sql_macro.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/sql_macro.rs) —— 用 Rust 构造的宏
- [`src/extension/functions/sql/`](https://github.com/shijianjs/duckfn/tree/main/src/extension/functions/sql) —— 那批 `.sql` 文件
- [`test/sql/functions/sql_macro.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/sql_macro.test) —— 期望结果
- [`src/functions/sql_macro_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/sql_macro_adapter.rs) —— 运行时侧

## 接下来

- [类型映射](./types.md)
- [属性参考](./attributes.md) —— `duck_sql_macro_files!` 与其它入口。
