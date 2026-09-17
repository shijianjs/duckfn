---
title: Copy functions
sidebar_position: 5
description: Provide a custom file format for COPY ... TO, with the four-phase lifecycle (bind, global init, sink, finalize) hidden behind one function.
---

# Copy functions

A copy function gives `COPY ... TO` a file format of your own:

```sql
COPY (SELECT * FROM orders) TO 'orders.tsv' (FORMAT dfn_copy_tsv);
COPY orders TO 'orders.tsv' (FORMAT dfn_copy_tsv);
```

The format name is the function name, and the function is called once per data chunk:

```rust
#[duck_copy_function]
fn dfn_copy_tsv(writer: &mut TsvWriter, chunk: &DataChunk) -> DuckResult<()> {
    /* write the rows of `chunk` to `writer` */
}
```

:::note[DuckDB 1.5.0+]
`COPY` functions come from DuckDB's C API as of 1.5.0, so they need `duckfn`'s
`duckdb-1-5` feature:

```toml
duckfn = { version = "0.0.3", features = ["duckdb-1-5"] }
```
:::

## The signature

The signature is fixed — one writer and one chunk, in either order:

| Part | Meaning |
| --- | --- |
| `&mut MyWriter` | The format's writer state. Its type implements [`DuckCopyWriter`](#duckcopywriter). |
| `&DataChunk` | The chunk to write: at most `2048` rows, all the query's columns. |
| `-> DuckResult<()>` | `Err` fails the whole `COPY`; panics are turned into query errors too. |

`chunk.size()` is the number of rows in this chunk (the last one may be smaller), and
`chunk.column_count()` matches the columns `DuckCopyWriter::open` was told about.

## `DuckCopyWriter`

The writer state is a plain struct implementing two methods:

```rust
pub trait DuckCopyWriter: Sized + 'static {
    /// Called once, before the first chunk: `path` is the COPY target,
    /// `columns` the logical types of the query's output columns.
    fn open(path: &str, columns: &[LogicalType]) -> DuckResult<Self>;

    /// Called once, after the last chunk; flush and close here.
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}
```

`finish` defaults to doing nothing. Overriding it matters because a flush error must be reported:
a `Drop` impl cannot return an error, so a `BufWriter` that only flushes on drop silently loses it.

```rust
use duckfn::{DuckCopyWriter, DuckResult, LogicalType, TypeId, duck_error};
use std::fs::File;
use std::io::{BufWriter, Write};

pub struct TsvWriter {
    file: BufWriter<File>,
    /// The output columns' types, recorded at bind time and used to read each cell.
    column_types: Vec<TypeId>,
}

impl DuckCopyWriter for TsvWriter {
    fn open(path: &str, columns: &[LogicalType]) -> DuckResult<Self> {
        let file = File::create(path)
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot create {path}: {e}")))?;
        let column_types = columns
            .iter()
            .map(|column| unsafe { column.get_type_id() })
            .collect();
        Ok(Self { file: BufWriter::new(file), column_types })
    }

    fn finish(&mut self) -> DuckResult<()> {
        self.file
            .flush()
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot flush: {e}")))
    }
}
```

## The four phases

DuckDB drives a `COPY` through four callbacks. `#[duck_copy_function]` generates all four; you only
write the sink:

| Phase | Called | What duckfn does | What you write |
| --- | --- | --- | --- |
| bind | once | Reads the output columns' `LogicalType`s from the bind info and keeps them as bind data. | — |
| global init | once | Calls `DuckCopyWriter::open(path, columns)`. | `open` |
| sink | once per chunk | Calls the annotated function. | the function body |
| finalize | once | Calls `DuckCopyWriter::finish()`. | `finish` |

Every phase runs inside `catch_unwind`, and both an `Err` and a panic are reported to DuckDB, so a
failure aborts the `COPY` instead of unwinding across the FFI boundary.

## Reading the chunk

A copy function receives data, so it has to read values itself. `DataChunk::reader(col)` gives a
`VectorReader` for one column, `is_valid(row)` says whether the cell is `NULL`, and the typed
`read_i64` / `read_str` / … methods read it. `TypeId` (from `LogicalType::get_type_id`) selects the
right one:

```rust
fn format_cell(reader: &VectorReader, row: usize, type_id: TypeId) -> DuckResult<Option<String>> {
    if !unsafe { reader.is_valid(row) } {
        return Ok(None); // SQL NULL
    }
    let text = match type_id {
        TypeId::BigInt => unsafe { reader.read_i64(row) }.to_string(),
        TypeId::Double => unsafe { reader.read_f64(row) }.to_string(),
        TypeId::Varchar => escape(unsafe { reader.read_str(row) }),
        other => {
            return Err(duck_error(format!(
                "dfn_copy_tsv: unsupported column type: {}",
                other.sql_name()
            )));
        }
    };
    Ok(Some(text))
}
```

Returning `Err` for types you do not handle is the honest choice: a half-written file that cannot be
read back is worse than a failed `COPY`.

:::note[Column names are not available]
The bind info exposes the output columns' *types* (`column_count` / `column_type`) but not their
names, so a format cannot write a header from it. Take the names from the query instead — or write a
headerless format.
:::

## Generated items and registration

Like the other function attributes, `#[duck_copy_function]` keeps the function and adds a module
named after it:

| Item | Purpose |
| --- | --- |
| `CopyFunctionImpl` | The adapter implementation. |
| `copy_function_builder()` | The configured `CopyFunctionBuilder` (returns a `DuckResult`). |
| `copy_function_register(connection)` | Registers the format on a connection. |

With the default `auto_register = true` the format is registered when the extension loads, so the
SQL above works out of the box. With `auto_register = false` only the items are generated:

```rust
#[duck_copy_function(auto_register = false)]
fn dfn_copy_manual(writer: &mut TsvWriter, chunk: &DataChunk) -> DuckResult<()> {
    /* … */
}

#[duck_custom_register]
fn dfn_copy_manual_register(c: &Connection) -> DuckResult<()> {
    dfn_copy_manual::copy_function_register(c)
}
```

## Limitations

- Only **`COPY ... TO`** is implemented. `COPY ... FROM` needs DuckDB's `copy_from` table-function
  hook, which `quack-rs` 0.16 does not expose yet; reading a custom format today means a normal
  [table function](./table-functions.md) plus a [replacement scan](./replacement-scans.md)
  (`SELECT * FROM 'orders.tsv'`).
- The format has **no bind-time options** of its own. `COPY ... (FORMAT dfn_copy_tsv, HEADER true)`
  is rejected by the binder before your code runs, because the option is not declared. Format-specific
  settings must travel in the query or the path.
- Column *names* are unavailable (see above).
- Registering a format with an existing name is a registration error, so do not collide with
  `csv`, `parquet`, `json`, …

## Source and tests

- [`src/extension/functions/copy_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/copy_function.rs) — the `dfn_copy_tsv` example format
- [`test/sql/functions/copy_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/copy_function.test) — the expected results (round-tripped through `read_csv`)
- [`duckfn/src/functions/copy_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/copy_function_adapter.rs) — the runtime side

## Next

- [Table functions](./table-functions.md)
- [Replacement scans](./replacement-scans.md)
- [Attributes](./attributes.md)
