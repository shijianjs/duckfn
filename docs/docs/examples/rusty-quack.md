---
title: The example extension
sidebar_position: 1
description: rusty_quack, the repository's example extension, with runnable SQL for every feature duckfn supports.
---

# The example extension

The repository root is a complete DuckDB extension called `rusty_quack`. It is not published — it
exists to exercise every feature, and its sqllogictest suite under `test/sql/` is the reference for
what each function returns.

## Build and load

```bash
make configure   # once: prepares the Python venv used by the test runner
make debug       # -> build/debug/extension/rusty_quack/rusty_quack.duckdb_extension
```

```bash
duckdb -unsigned -c "
LOAD './build/debug/extension/rusty_quack/rusty_quack.duckdb_extension';
SELECT rusty_echo('Jane');
"
```

During development `just duckdb_ext "<SQL>"` rebuilds and runs a statement in one step, and
`just test` runs the whole sqllogictest suite.

## How the source is organised

| Path | Contents |
| --- | --- |
| `src/extension/mod.rs` | `duckfn_entrypoint!("rusty_quack")` and the module tree. |
| `src/extension/demo/` | One file per feature area, including several hand-written FFI variants kept for comparison. |
| `src/extension/functions/` | One file per registration kind: scalar, aggregate, table, cast, replacement scan, SQL macro. |
| `src/extension/functions/sql/` | `.sql` files registered through `include_str!` and `duck_sql_macro_files!`. |
| `src/extension/types/` | Echo functions for every supported type, in scalar and table form. |
| `test/sql/` | 25 sqllogictest files mirroring the source layout. |

`src/extension/demo/rewrite_official_template_demo.rs` is the smallest useful starting point — it is
the DuckDB template's demo rewritten with duckfn:

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
SELECT rusty_echo('Hello');     -- 🐤 Hello 🦀 Hello
SELECT * FROM rusty_quack('Sam');  -- Rusty Quack Sam 🐥
```

## Scalars, lists, maps and structs

The demo module covers the shapes a function can take, including nested input and output:

```sql
SELECT double_it5(21);                            -- 42
SELECT first_word_tuple('hello world');           -- hello
SELECT sum_list_w([1, 2, 3, 4]);                  -- 10
SELECT sum_list_nest([[1, 2], [3, null, 4], null]);  -- 10
SELECT struct_scalar_w({hello_count: 15});        -- 25
SELECT struct_nest_scalar_w({structf: {hello_count: 15}, list: [1, null, 2]});  -- 18
SELECT input_map_demo(MAP {'key1': [10], 'key2': [20, 5], 'key3': null});       -- 35
SELECT CAST(input_array_demo(a) AS VARCHAR) FROM (VALUES (ARRAY[1, 2]), (ARRAY[4, null])) t(a);
-- [1, 2]
-- [4, NULL]
```

Returning structured data works the same way: `make_list_scalar_w(range)` returns a `LIST`,
`nest_list_scalar_w(range)` a nested `LIST`, and `struct_nest_output_scalar_w(range::int)` a nested
`STRUCT`.

## Errors and NULL in practice

`error_scalar_demo` takes one input and answers with a value, a `NULL`, or an error:

```sql
SELECT error_scalar_demo(3);   -- 6
SELECT error_scalar_demo(10);  -- error: input is 10
SELECT error_scalar_demo(20);  -- error: panic: input is 20
SELECT error_scalar_demo(30);  -- error: explicit panic
```

## Aggregates

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

`word_count_w` is registered by hand through `#[duck_custom_register]` and a
`WordCountStateWrapper`, and `agg_list_w` collects rows into a `LIST` while failing the query when it
sees the value `12`.

## Table functions and named parameters

```sql
SELECT * FROM count_down_m_simple(start=12);   -- 11, 10, 9, … 0

SELECT * FROM bind_map_demo(MAP {'key1': [10], 'key2': [20, 5], 'key3': null});
-- 10
-- 25
--  0
```

## Casts and replacement scans

```sql
SELECT CAST('42' AS INTEGER);        -- 42
SELECT TRY_CAST('abc' AS INTEGER);   -- NULL
SELECT CAST('abc' AS INTEGER);       -- error: not an integer: "abc"

SELECT * FROM '3.points';            -- x 0 y 0 / 1 1 / 2 4
SELECT * FROM 'hi.echo';             -- hi.echo  7
```

## SQL macros

```sql
SELECT clamp(range, 4, 7) FROM range(9);
-- 4, 4, 4, 4, 4, 5, 6, 7, 7

SELECT add_two_v1(1), add_two_v2(2), add_two_v3(3), add_two_v4(4);
-- 3, 4, 5, 6
```

The four `add_two_*` names are four different ways of returning the same macro: `SqlMacro`,
`DuckResult<SqlMacro>`, `DuckResult<String>` and `String`. The `dfn_macro_inc_*` family comes from
`include_str!`, and `dfn_macro_files_*` from `duck_sql_macro_files!`.

## Type echoes

Every supported type has an identity function in both scalar and table form:

```sql
SELECT dfn_echo_integer(42);          -- 42
SELECT dfn_echo_date(DATE '2024-01-02');  -- 2024-01-02
SELECT CAST(dfn_echo_list_integer_n([1, NULL, 3]) AS VARCHAR);  -- [1, NULL, 3]
SELECT CAST(v AS VARCHAR) FROM dfn_table_echo_bool(true, count => 3);
-- true
-- NULL
-- true
```

The naming is regular: `dfn_echo_<type>` for scalars, `dfn_table_echo_<type>` for table functions,
where `<type>` is `integer`, `list_integer`, `map_varchar_integer`, `array_bigint`,
`struct_with_list`, and so on.

## The test suite

`test/sql/` mirrors the source layout and is the most precise description of behaviour available:

```
test/sql/
├── demo/        scalar_function_demo, aggregate_function_demo, table_function_demo,
│                macro_demo, rusty_echo, rusty_quack, sql_lang_demo
├── functions/   scalar_function, aggregate_function, table_function, cast_function,
│                replacement_scan, sql_macro
└── types/       <type>_scalar_echo and <type>_table_echo for every supported type
```

```bash
make configure debug test   # or: just test
```

## Next

- [Same functions, two ways](./side-by-side.md) — `rusty_echo`, `rusty_quack`, `word_count` and `first_word` next to their raw `duckdb` / `quack-rs` implementations.
- [Quick start](../getting-started/quick-start.md) — the same ideas on a minimal crate.
- [Build and release](../build-and-release.md) — how this extension is packaged and published.
