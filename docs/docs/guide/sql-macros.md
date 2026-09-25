---
title: SQL macros
sidebar_position: 6
description: Register scalar and table SQL macros — preferably from .sql files that are inlined at compile time.
---

# SQL macros

A SQL macro registers SQL rather than Rust code. Four return shapes are accepted, but the easiest way
to keep macros maintainable is to write them in `.sql` files and register the files.

## Macros from `.sql` files

`duck_sql_macro_files!` takes any number of paths, inlines every file at compile time, and executes
them in order:

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

Why this is the convenient form:

- The macros are real SQL files, so editors highlight and check them, and one file may define any
  number of macros — scalar and table macros side by side, which a single `SqlMacro` cannot express.
- Paths are resolved relative to the `.rs` file that invokes the macro and inlined with
  `include_str!` at compile time, so a typo is a compile error rather than a macro that is missing at
  runtime.
- Files run in the order written, so a later file can build on a macro defined earlier.

For a single file, `include_str!` does the same job without the macro:

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

## Building a macro with `SqlMacro`

When the body is short and belongs next to the Rust code, build it directly. `SqlMacro` comes from
`quack-rs`:

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

`SqlMacro::scalar(name, params, body)` and `SqlMacro::table(name, params, sql)` cover both kinds:

```rust
#[duck_sql_macro]
pub fn dfn_macro_gen() -> DuckResult<SqlMacro> {
    Ok(SqlMacro::table("dfn_macro_gen", &["n"], "SELECT * FROM range(n)"))
}
```

```sql
SELECT * FROM dfn_macro_gen(3);      -- 0, 1, 2
SELECT range FROM DFN_MACRO_GEN(2);  -- 0, 1   (macro names are case-insensitive)
```

A body is not limited to a scalar expression — `dfn_macro_pair` returns a `STRUCT` and
`dfn_macro_mklist` a `LIST`:

```sql
SELECT CAST(dfn_macro_pair(5) AS VARCHAR);   -- {'a': 5, 'b': 10}
SELECT typeof(dfn_macro_pair(5));            -- STRUCT(a INTEGER, b INTEGER)
SELECT dfn_macro_pair(5).a;                  -- 5
SELECT CAST(dfn_macro_mklist(5) AS VARCHAR); -- [5, 10]
```

## Returning SQL as a string

Returning a string executes it verbatim, which is how you use `CREATE MACRO` directly:

```rust
#[duck_sql_macro]
pub fn dfn_macro_double() -> String {
    "CREATE OR REPLACE MACRO dfn_macro_double(x) AS (x * 2)".to_string()
}
```

```sql
SELECT dfn_macro_double(21);  -- 42
```

A single string may hold several statements separated by `;`, so one function can define a macro that
depends on another:

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

## Errors

Macro names are looked up case-insensitively, and mistakes surface as ordinary DuckDB errors:

| Mistake | Error |
| --- | --- |
| Wrong number of arguments | `dfn_macro_add does not support the supplied arguments` |
| A table macro used as a scalar | `dfn_macro_gen is a table function but it was used as a scalar function` |
| A scalar macro used with `FROM` | `Table Function with name dfn_macro_add does not exist` |

## Source and tests

- [`duckfn-quack/src/extension/functions/sql_macro.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/src/extension/functions/sql_macro.rs) — the macros built in Rust
- [`duckfn-quack/src/extension/functions/sql/`](https://github.com/shijianjs/duckfn/tree/main/duckfn-quack/src/extension/functions/sql) — the `.sql` files
- [`duckfn-quack/test/sql/functions/sql_macro.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/functions/sql_macro.test) — the expected results
- [`src/functions/sql_macro_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/sql_macro_adapter.rs) — the runtime side

## Next

- [Type mapping](./types.md)
- [Attributes](./attributes.md) — `duck_sql_macro_files!` and the other entry points.
