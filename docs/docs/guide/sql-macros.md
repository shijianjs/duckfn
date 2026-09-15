---
title: SQL macros
sidebar_position: 6
description: Register scalar and table SQL macros from Rust, or from .sql files inlined at compile time.
---

# SQL macros

A SQL macro registers SQL rather than Rust code. There are two ways to describe one, and four ways
to hand it over, and `#[duck_sql_macro]` accepts all of them.

## Building a macro with `SqlMacro`

```rust
#[duck_sql_macro]
pub fn dfn_macro_clamp() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_clamp", &["x", "lo", "hi"], "greatest(lo, least(hi, x))")
}
```

```sql
SELECT dfn_macro_clamp(12, 0, 10);   -- 10
SELECT dfn_macro_clamp(-5, 0, 10);   -- 0
SELECT typeof(dfn_macro_clamp(5, 0, 10));  -- INTEGER
```

`SqlMacro` comes from `quack-rs`. `SqlMacro::scalar(name, params, body)` and
`SqlMacro::table(name, params, sql)` cover both kinds:

```rust
#[duck_sql_macro]
pub fn dfn_macro_gen() -> DuckResult<SqlMacro> {
    Ok(SqlMacro::table("dfn_macro_gen", &["n"], "SELECT * FROM range(n)"))
}
```

```sql
SELECT * FROM dfn_macro_gen(3);   -- 0, 1, 2
SELECT range FROM DFN_MACRO_GEN(2);  -- 0, 1   (macro names are case-insensitive)
```

A macro body is not limited to a scalar expression either — `dfn_macro_pair` returns a `STRUCT` and
`dfn_macro_mklist` a `LIST`:

```sql
SELECT CAST(dfn_macro_pair(5) AS VARCHAR);  -- {'a': 5, 'b': 10}
SELECT typeof(dfn_macro_pair(5));           -- STRUCT(a INTEGER, b INTEGER)
SELECT dfn_macro_pair(5).a;                 -- 5
SELECT CAST(dfn_macro_mklist(5) AS VARCHAR);-- [5, 10]
```

## Returning SQL as a string

Returning a string executes it verbatim, which is how you use `CREATE MACRO` directly and how you
register several macros at once:

```rust
#[duck_sql_macro]
pub fn dfn_macro_double() -> String {
    "CREATE OR REPLACE MACRO dfn_macro_double(x) AS (x * 2)".to_string()
}
```

```sql
SELECT dfn_macro_double(21);  -- 42
```

A single string may hold several statements separated by `;`, so one function can define a macro
that depends on another:

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

The four accepted return shapes are `SqlMacro`, `DuckResult<SqlMacro>`, a SQL string (`String` or
`&'static str`), and a `DuckResult` of a SQL string.

## Macros from `.sql` files

Longer SQL is easier to keep in a `.sql` file. A single file works with `include_str!`:

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

One file can also define scalar and table macros together, which cannot be expressed with a single
`SqlMacro`.

For several files at once, use `duck_sql_macro_files!`:

```rust
duck_sql_macro_files!(
    "sql/macro_files_a.sql",
    "sql/macro_files_b.sql",
    "sql/macro_files_c.sql"
);
```

```sql
SELECT dfn_macro_files_add(2, 3);   -- 5
SELECT dfn_macro_files_mul(4, 5);   -- 20
SELECT * FROM dfn_macro_files_gen(3);  -- 0, 1, 2
SELECT dfn_macro_files_negate(7);   -- -7
```

Paths are relative to the `.rs` file that invokes the macro and are inlined with `include_str!` at
compile time, so a wrong path is a compile error rather than a missing macro at runtime. Files are
executed in the order written.

## Errors

Macro names are looked up case-insensitively, and mistakes surface as ordinary DuckDB errors:

| Mistake | Error |
| --- | --- |
| Wrong number of arguments | `dfn_macro_add does not support the supplied arguments` |
| A table macro used as a scalar | `dfn_macro_gen is a table function but it was used as a scalar function` |
| A scalar macro used with `FROM` | `Table Function with name dfn_macro_add does not exist` |

## Next

- [Type mapping](./types.md)
- [Attributes](./attributes.md) — `duck_sql_macro_files!` and the other entry points.
