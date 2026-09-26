---
title: Copy functions
sidebar_position: 5
description: Provide custom file formats for COPY ... TO and COPY ... FROM, driven by runtime dynamic columns.
---

# Copy functions

A copy function gives `COPY` a file format of your own — in both directions:

```sql
COPY (SELECT * FROM orders) TO 'orders.tsv' (FORMAT dfn_copy_tsv);
COPY orders FROM 'orders.tsv' (FORMAT dfn_copy_tsv_from);
```

Both are built on **[runtime dynamic columns](./table-functions.md#dynamic-columns)**: the columns are
not fixed at compile time. `COPY ... TO` reconstructs the schema from the query's output columns during
bind and hands each chunk to your function as `DuckDynamicRow`s; `COPY ... FROM` reads the **target
table's** schema and takes dynamic rows from you batch by batch. `LIST` / `STRUCT` / `MAP` / `DECIMAL`
and `NULL` therefore work in both directions.

:::note[DuckDB 1.5.0+]
Copy functions come from DuckDB's C API as of 1.5.0, so they need `duckfn`'s `duckdb-1-5` feature:

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["duckdb-1-5"] }
```
:::

## `COPY ... TO`

The format name is the function name, and the function is called once per data chunk:

```rust
#[duck_copy_function]
fn dfn_copy_tsv(writer: &mut TsvWriter, rows: &[DuckDynamicRow]) -> DuckResult<()> {
    /* write `rows` to `writer` */
}
```

| Part | Meaning |
| --- | --- |
| `&mut MyWriter` | The format's writer state, implementing [`DuckCopyToWriter`](#duckcopytowriter). |
| `&[DuckDynamicRow]` | This chunk's rows: at most `2048`, one cell per schema column, `None` for SQL NULL. |
| `-> DuckResult<()>` | `Err` fails the whole `COPY`; panics become query errors too. |

The two parameters may be written in either order.

### `DuckCopyToWriter`

The writer state owns the lifecycle; the per-batch writing stays in the function:

```rust
pub trait DuckCopyToWriter: Sized + 'static {
    /// Called once, before the first chunk: the target path, the dynamic schema
    /// and the COPY options.
    fn open(path: &str, schema: &DuckResultSchema, options: &DuckCopyOptions) -> DuckResult<Self>;

    /// Called once, after the last chunk; flush and close here.
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}
```

`schema.columns()` gives `(name, DuckTypeDesc)` pairs in the query's column order — the description is
what tells a `LIST` from a `STRUCT`, and it is what a format should render against. `finish` defaults
to doing nothing; overriding it matters because a flush error must be reported (`Drop` cannot return
one, so a `BufWriter` that only flushes on drop would silently lose it).

### Options

The extra `COPY ... TO (...)` options arrive as a `STRUCT` and are exposed as `DuckCopyOptions`
(option name → dynamic value, lookups are case-insensitive):

```rust
fn open(path: &str, schema: &DuckResultSchema, options: &DuckCopyOptions) -> DuckResult<Self> {
    let header = options.get_bool("header").unwrap_or(false);
    /* ... */
}
```

```sql
COPY (SELECT 1 AS i) TO 'out.tsv' (FORMAT dfn_copy_tsv, HEADER true);
```

`get_bool` / `get_i64` / `get_str` / `get` cover the common cases; `entries()` returns everything.
Options whose type cannot be expressed as a `DuckTypeDesc` (an `ENUM`, say) are skipped rather than
failing the `COPY`.

### The four phases

DuckDB drives a `COPY TO` through four callbacks. `#[duck_copy_function]` generates all four; you only
write the row loop:

| Phase | Called | What duckfn does | What you write |
| --- | --- | --- | --- |
| bind | once | Turns the output columns into a `DuckResultSchema`, reads the options, keeps both as bind data. | — |
| global init | once | Calls `DuckCopyToWriter::open(path, schema, options)`. | `open` |
| sink | once per chunk | Reads the chunk into `Vec<DuckDynamicRow>` and calls the annotated function. | the function body |
| finalize | once | Calls `DuckCopyToWriter::finish()`. | `finish` |

Every phase runs inside `catch_unwind`, and both an `Err` and a panic are reported to DuckDB, so a
failure aborts the `COPY` instead of unwinding across the FFI boundary.

## `COPY ... FROM`

`COPY ... FROM` loads a file into an **existing** table, so the schema is not yours to decide: DuckDB
gives you the target table's columns and takes rows from you:

```rust
#[duck_copy_from_function]
fn dfn_copy_tsv_from(reader: &mut TsvReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> {
    /* return up to `limit` rows; an empty Vec ends the stream */
}
```

| Part | Meaning |
| --- | --- |
| `&mut MyReader` | The reader state, implementing [`DuckCopyFromReader`](#duckcopyfromreader). |
| `limit: usize` | The batch size — one DuckDB vector's row count. Taking fewer rows is fine. |
| `-> DuckResult<Vec<DuckDynamicRow>>` | An empty `Vec` means end of stream. |

### `DuckCopyFromReader`

Two phases again, and the arguments carry both the path and the options:

```rust
pub trait DuckCopyFromReader: Sized + Send + 'static {
    /// Field 0 must be the file path (the one positional parameter); the remaining
    /// fields are the named COPY options this reader supports.
    type Args: DuckBindArgs;

    fn open(args: Self::Args, schema: &DuckResultSchema) -> DuckResult<Self>;

    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}
```

`Args` is an ordinary `#[derive(DuckStruct)]` struct, so `COPY ... FROM 'f' (FORMAT fmt, SKIP_ROWS 1)`
reaches `open` as a field:

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "skip_rows")]
pub struct MyFromArgs {
    pub path: String,
    pub skip_rows: Option<i64>,
}
```

Three rules come from DuckDB:

- **exactly one positional parameter** (the file path; `duckfn` checks this and reports the count);
- the reader **must not declare result columns** — the schema comes from the target table, and
  `open` receives it;
- every option you accept must be **declared** through `Args`; an undeclared one is rejected by the
  binder *before* bind runs, and that message names the **table function** — which is why the reader
  table function carries the format name.

### The three callbacks

| Phase | Called | What duckfn does | What you write |
| --- | --- | --- | --- |
| bind | once | Parses `Args`, reads the target table's schema, calls `Reader::open`, keeps the state. | `open` |
| scan | until EOF | Calls the annotated function and writes the returned rows into the output chunk. | the function body |
| finalize | — | A table function has no finalize callback: `Reader::finish` runs when the query ends and the state is dropped. | `finish` |

Because there is no finalize callback, an error from `finish` **cannot be reported** — it becomes one
`-- [duckfn]` warning line. Anything that must abort the load belongs in `open` or in the batch
function.

Row validation is free: if a row's width or a cell's type disagrees with the target table, the
per-column check inside `DuckDynamicRow::write_batch` fails the `COPY` with a readable error instead of
writing a misaligned row.

## Encoding is yours

`DuckDynamicValue` is a value, not a text format. `to_text` renders it for **display** (strings
verbatim, a nested `NULL` printed as `NULL`), which is ambiguous as soon as a string happens to be
`NULL` — so a format should render recursively with its own escaping rules. The TSV example does
exactly that, and the escaping side is worth copying:

| Concern | What the example does |
| --- | --- |
| Row / column framing | one record per line, tab-separated, NULL as `\N` |
| Strings inside containers | quoted (`['NULL', NULL]` cannot be confused with `['NULL', 'NULL']`) |
| A string containing a separator | escaped (`\t` → `\t`), so it cannot break the framing |
| `BLOB` | `\xHH` per byte |

## Generated items and registration

Like the other function attributes, the macros keep the function and add a module named after it:

| Item | Purpose |
| --- | --- |
| `CopyFunctionImpl` / `CopyFromFunctionImpl` | The adapter implementation. |
| `copy_function_builder()` | The configured `CopyFunctionBuilder` (returns a `DuckResult`). |
| `copy_function_register(connection)` / `copy_from_register(connection)` | Registers the format on a connection. |

With the default `auto_register = true` the format is registered when the extension loads, so the SQL
above works out of the box. With `auto_register = false` only the items are generated:

```rust
#[duck_copy_from_function(auto_register = false)]
fn dfn_copy_manual(reader: &mut MyReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> {
    /* … */
}

#[duck_custom_register]
fn dfn_copy_manual_register(c: &Connection) -> DuckResult<()> {
    dfn_copy_manual::copy_from_register(c)
}
```

## Limitations

- **`COPY ... FROM` scans serially.** The reader state is `Send` but not `Sync`, so the adapter pins
  the scan to one thread with `set_max_threads(1)`; there is no parallel loading.
- **`ENUM` / `ARRAY` / `UNION` / `BIT` columns are rejected** in both directions, with an error,
  because their parameters are not expressible as a `DuckTypeDesc` (see
  [type mapping](./types.md)).
- **`COPY ... TO` column names are synthesised** (`column_0`, `column_1`, …): DuckDB exposes the
  output columns' types but not their names. A format that needs real names should carry them through
  its own option instead of guessing.
- **`MAP` values may not be `NULL`** — neither in a dynamic value nor in the target table.
- Registering a format with an existing name is a registration error, so do not collide with `csv`,
  `parquet`, `json`, …

## Source and tests

- [`test/extension/functions/copy_function.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/copy_function.rs) — the `dfn_copy_tsv` writer
- [`test/extension/functions/copy_from_function.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/copy_from_function.rs) — the `dfn_copy_tsv_from` reader
- [`test/extension/functions/tsv_format.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/tsv_format.rs) — the shared cell codec (escaping and parsing)
- [`test/sql/functions/copy_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/copy_function.test) and [`copy_from_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/copy_from_function.test) — the expected results
- [`src/functions/copy_to_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/copy_to_adapter.rs) / [`copy_from_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/copy_from_adapter.rs) — the runtime side

## Next

- [Table functions](./table-functions.md) — the dynamic columns these formats move around
- [Type mapping](./types.md)
- [Attributes](./attributes.md)
