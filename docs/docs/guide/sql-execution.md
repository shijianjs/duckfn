---
title: Running SQL from an extension
sidebar_position: 13
description: Run SQL from a callback through duckfn's owned connection and handle the result as a dynamic table — with parameter binding and identifier quoting for runtime-only names.
---

# Running SQL from an extension

DuckDB's C API hands out a **client context** in some callbacks only — and a client context carries
**no connection**: there is no `duckdb_client_context_get_connection`. A row handler, a table
function's `scan`, an aggregate's `finalize` — none of them can reach a connection, which is why a
callback that needs to run SQL has no direct route to it.

Registration time is the only window onto one, and the connection DuckDB passes to your entry point
is **borrowed**: it is disconnected as soon as registration returns. `duckfn` therefore opens **its
own long-lived connection** during registration — the same one [File system access](./file-system.md)
describes — and `duckfn::sql` is that connection's SQL side: run statements, run queries, and get the
rows back as runtime **dynamic values**.

::::note[Stable C API — no `duckdb-1-5` needed]

`duckfn::sql` rests on the connection captured at registration time, but every call it makes lives in
the **stable** region of the C API — `duckdb_connect`, prepare / bind / fetch-chunk, the logical
types. So it does not need `duckdb-1-5`, and the one feature is enough:

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["sql"] }
```

That matters for the artifact, not just for the build: keep `USE_UNSTABLE_C_API=0` with a `v1.2.0`
floor and one binary serves DuckDB 1.3 through 2.0 (see
[Version numbers and the ABI](../duckdb-versions/c-api-and-abi.md)). An extension that also wants the
host file system needs `owned-connection` instead, which brings `sql` along and pulls in
[`duckfn::duck_vfs`](./file-system.md) — that half *is* in the unstable region and has to line up with
the engine's minor release.

::::

Either way the feature has side effects worth reading before turning it on — see
[Cost and limits](#cost-and-limits).

## Running statements

`duckfn::sql::execute` runs one or more `;`-separated statements and returns the number of rows
changed — 0 for `CREATE`, `SET` and friends:

```rust
use duckfn::sql;

let created = sql::execute("CREATE OR REPLACE TABLE runs(id BIGINT, note VARCHAR)")?;  // 0
let inserted = sql::execute("INSERT INTO runs VALUES (1, 'a'), (2, 'b')")?;             // 2
let deleted = sql::execute("DELETE FROM runs WHERE id > 1")?;                           // 1
```

Every statement runs, in order. The count is that of **the statement whose result is returned** —
the first one that produces rows, or else the last — so `"SELECT 1; INSERT …"` reports the `INSERT`,
and a string of only `SELECT`s reports the first one's. A failing statement fails the call, but the
statements before it have already run and committed unless you opened an explicit transaction.

## Running queries

`duckfn::sql::query` runs a query and returns a `DuckQueryResult` — a schema plus the rows, read off
the result itself:

```rust
use duckfn::{DuckDynamicValue, sql};

let result = sql::query("SELECT 42::BIGINT AS answer, [1, 2, 3] AS tags")?;
assert_eq!(result.len(), 1);
assert_eq!(
    result.cell(0, "answer")?,
    Some(&DuckDynamicValue::BigInt(42)),
);
```

The **column names and types come from SQL**, not from your Rust code: they are read from the result
(`duckdb_column_name` / `duckdb_column_logical_type`) and turned into `DuckTypeDesc`, the same runtime
description the [dynamic-column table functions](./table-functions.md) use. A table function can
therefore run a caller-supplied query during `bind` and hand the columns straight back:

```rust
use duckfn::{DuckDynamicTable, DuckResult, duck_table_function, sql};

#[duck_table_function(dynamic_columns = true)]
fn my_query(statement: String) -> DuckResult<DuckDynamicTable> {
    Ok(sql::query(&statement)?.into_table())
}
```

The shapes that survive the round trip are the ones `DuckTypeDesc` can describe: scalars, `DECIMAL`,
`LIST`, `STRUCT` and `MAP`, nested to any depth. `ENUM`, `ARRAY`, `UNION` and `BIT` have no reliable
reconstruction, so a column of one of those fails the query with a message naming the column rather
than coming back half-described.

## Handling the result

`DuckQueryResult` is materialised: `query` reads every chunk into `DuckDynamicRow`s as it goes,
because a `duckdb_result` handle cannot leave the owned connection. That makes the whole result a
plain value you can index:

| Call | What it does |
| --- | --- |
| `schema()` / `column_count()` / `column_names()` | The output columns, in order |
| `column_index(name)` | The index of a column by name (case-sensitive) |
| `len()` / `is_empty()` / `rows()` / `row(i)` | The rows |
| `cell(row, name)` | One cell by **column name**; `Ok(None)` means SQL NULL, an unknown column is an `Err` |
| `single_row()` | The only row — an error when the count is not 1 |
| `single_value()` | The only value (one row, one column); `Ok(None)` means SQL NULL |
| `single_value_text()` | The same value rendered as text |
| `into_table()` | A `DuckDynamicTable`, ready for a table function's `bind` |
| `into_parts()` / `into_rows()` | The schema and rows on their own |

A scalar function has to return a fixed Rust type, so `single_value_text()` is how a `SELECT` result
usually gets back to SQL:

```rust
#[duck_scalar_function(volatile = true)]
fn dfn_sql_scalar(statement: String) -> DuckOptionResult<String> {
    sql::query(&statement)?.single_value_text()
}
```

:::note[It renders the physical value]

`single_value_text()` uses `DuckDynamicValue::to_text`, so `DECIMAL` comes out as its **unscaled
integer** (`1.5::DECIMAL(4,1)` → `"15"`) and `DATE` / `TIMESTAMP*` / `UUID` / `INTERVAL` as their
physical integer too — it is a rendering, not a round-trip encoding. Use `single_value()` and match
on the `DuckDynamicValue` when you need the value itself.

:::

## Building SQL with runtime-only content

Table names, paths and user input only exist at run time, and there are exactly two ways in:

**Bind values** with `query_with` / `execute_with`. The value goes to a `?` placeholder, so the SQL
text never contains it:

```rust
use duckfn::{DuckDynamicValue, sql};

let result = sql::query_with(
    "SELECT ?::BIGINT * 2",
    &[DuckDynamicValue::BigInt(21)],
)?;
assert_eq!(result.single_value()?, Some(DuckDynamicValue::BigInt(42)));
```

Every scalar `DuckDynamicValue` is bindable (the value is handed to DuckDB as a `duckdb_value`, so
`UUID`, `TIMESTAMP_S|MS|NS` and `TIME WITH TIME ZONE` work too, and `TIME` / `TIMESTAMP*` / `DECIMAL`
are range-checked before the call). `LIST` / `STRUCT` / `MAP` are **not** bindable: their nested NULLs
have no `duckdb_value` form. Build those inline in SQL (`list_value`, `struct_pack`, `map`) or bind
their scalar parts.

**Quote identifiers** with `quote_identifier`. An identifier cannot be a `?` placeholder — it is part
of the statement's structure — so a runtime table name has to be spliced, and this is the escaping
boundary:

```rust
let statement = format!("SELECT count(*) FROM {}", sql::quote_identifier(table_name));
let count = sql::query(&statement)?.single_value()?;
```

`quote_identifier` doubles inner `"` and wraps both ends, so `dfn_t` becomes `"dfn_t"` and a name like
`a"b` cannot break out of the quoting. `quote_literal` does the same for single-quoted string
literals — use it only where a placeholder cannot go (`SET` values, `INSTALL '...'`), and prefer
binding everywhere else.

## A worked example

The repository's example extension has both halves wired to SQL
(`test/extension/functions/sql_exec.rs`):

```sql
SELECT dfn_sql_exec('CREATE OR REPLACE TABLE dfn_sql_demo(i INTEGER, s VARCHAR)');
SELECT dfn_sql_exec('INSERT INTO dfn_sql_demo VALUES (1, ''a''), (2, ''b''), (3, ''c'')');  -- 3

SELECT * FROM dfn_sql_query('SELECT 42::BIGINT AS answer, [1, 2] AS tags');
SELECT dfn_sql_scalar('SELECT 6 * 7');                                   -- 42
SELECT dfn_sql_scalar_arg('SELECT ?::BIGINT * 2', 21);                   -- 42
SELECT dfn_sql_table_count('dfn_sql_demo');                              -- 3
SELECT dfn_sql_has_table('dfn_sql_demo');                                -- true
```

| Function | Shows |
| --- | --- |
| `dfn_sql_exec` | `execute` — run a statement, report rows changed |
| `dfn_sql_scalar` | `query` + `single_value_text` — one value back as text |
| `dfn_sql_scalar_arg` | `query_with` — a bound parameter (`?`), never spliced text |
| `dfn_sql_table_count` | `quote_identifier` — a runtime table name spliced safely |
| `dfn_sql_has_table` | A bound string value against `information_schema` |
| `dfn_sql_query` | `query` + `into_table` — the columns come from SQL itself |

`test/sql/functions/sql_exec.test` covers all of them, including the chunk boundary (a
`range(5000)` result read through the dynamic path), NULL cells, the empty-result case and the error
paths.

## Cost and limits

- **Statements run on a second connection.** That is the point — it is the only connection a callback
  can reach — and it means an outer query keeps its own snapshot while your statement runs. It also
  means connection-level `SET`s are not shared: a `SET` you run through `duckfn::sql` applies to the
  owned connection, not to the caller's.
- **The whole result is materialised.** `query` reads every chunk into memory before returning. For a
  result too large to hold, hand-write an adapter and keep your own connection instead.
- **It shares one lock with `duckfn::duck_vfs` when both are on.** A call holds that connection from
  start to finish, so a long query blocks file I/O, and **nesting a take-up on one thread deadlocks** —
  with `owned-connection` on, never call `duckfn::sql` from inside a `with_file_system` closure, or the
  other way round. Concurrent calls from other threads simply queue.
- **One process-level connection, first instance wins.** With several database instances in one
  process, the one that finished registering first is the connection used.
- **Not usable where DuckDB runs synchronously on a single thread.** Opening the connection during
  registration re-enters the engine on the current thread, so a runtime with no other thread
  deadlocks and the extension `LOAD` hangs. The one known case is DuckDB-Wasm's Node **blocking**
  bindings; the browser, the native CLI and Node's worker mode are fine.

## Errors

`duckfn::sql` prefixes the underlying error with the step that produced it, so assertions and logs can
key on it:

| Prefix | Comes from |
| --- | --- |
| `duckfn::sql::execute: …` | `execute` / `execute_with` |
| `duckfn::sql::query: …` | `query` / `query_with`, and the chunk read behind them |
| `duckfn::sql: parameter N: …` | Binding parameter `N` (1-based) |
| `duckfn::sql: the statement has N parameter(s) but M value(s) were given` | A parameter-count mismatch |
| `duckfn::sql: column I (\`name\`) cannot be turned into a dynamic column: …` | A column type the dynamic path cannot express |

A scalar function returning `Err` fails the whole query, so a broken inner query is reported rather
than silently skipped.
