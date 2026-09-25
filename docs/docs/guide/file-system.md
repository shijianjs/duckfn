---
title: File system access
sidebar_position: 12
description: Read and write files through DuckDB's virtual file system — including s3:// and http(s):// — from aggregate functions and any other callback.
---

# File system access

DuckDB's C API hands out a **client context** in some callbacks only: scalar functions get one in
their `bind` / `init` callbacks, table functions have
`duckdb_table_function_get_client_context`, and each of the four `COPY TO` callbacks has its own
entry point. **Aggregate functions have none** — no bind callback, and no
`duckdb_aggregate_function_get_client_context`. A row handler or a `finalize` cannot reach a
`ClientContext`, and without one there is no `FileSystem`.

Registration time is the only window, and the connection DuckDB passes to your entry point is
**borrowed**: it is disconnected as soon as registration returns. Keeping that handle — or a
`ClientContext` / `FileSystem` derived from it — would leave you with a dangling reference: both are
reference-semantic in the C API, and `duckdb_destroy_client_context` / `duckdb_destroy_file_system`
only `delete` the wrapper object.

`duckfn` therefore opens **its own long-lived connection** during registration and hides the rest
behind three functions, so any callback can reach DuckDB's file system — `s3://` and `http(s)://`
(through `httpfs`), in-memory file systems and local disk all go the same way, instead of degrading
to `std::fs`, which only ever sees local disk.

:::note[DuckDB 1.5.0+]

File-system access comes from DuckDB's C API as of 1.5.0, so it needs `duckfn`'s `duckdb-1-5`
feature:

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["duckdb-1-5"] }
```
:::

## What duckfn sets up

At extension load, `duckfn::register_all_duckfn` (the function your `duckfn_entrypoint!` points at)
captures the database handle it is given and opens one `OwnedConnection` from it, storing the result
in a process-level static. That connection holds a `shared_ptr` to the database instance, so it
outlives extension loading, and statics are never dropped — which is what keeps the handles derived
from it valid.

Every take-up then builds a fresh `ClientContext` → `FileSystem` pair from that connection and
releases both when the guard goes away. Nothing escapes, and nothing has to be registered per
function: a macro-written aggregate and a hand-written adapter use exactly the same call.

Capture is best-effort: if opening the connection fails, registration still succeeds and later
take-ups report the recorded reason.

## Reading a file

```rust
use std::ffi::CString;

use duckfn::duck_vfs::{ErrorData, FileOpenOptions};
use duckfn::{DuckAggregateState, DuckResult, duck_aggregate_function, duck_error};

#[derive(Default, Debug, Clone)]
struct FileSizeState {
    total: i64,
}

impl DuckAggregateState for FileSizeState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
    }

    fn simple_result(&self) -> Self::Output {
        self.total
    }
}

#[duck_aggregate_function]
fn dfn_agg_file_size(path: String, state: &mut FileSizeState) -> DuckResult<()> {
    let path = CString::new(path)
        .map_err(|_| duck_error("dfn_agg_file_size: path contains a NUL byte"))?;
    state.total += duckfn::duck_vfs::with_file_system(|fs| {
        let handle = fs
            .open(&path, &FileOpenOptions::read_only())
            .map_err(file_error)?;
        let size = handle.size().map_err(file_error)?;
        i64::try_from(size).map_err(|_| duck_error("file is larger than BIGINT"))
    })?;
    Ok(())
}

fn file_error(error: ErrorData) -> quack_rs::error::ExtensionError {
    duck_error(error.message().unwrap_or_else(|| "file system error".to_string()))
}
```

```sql
SELECT dfn_agg_file_size(path) FROM (VALUES ('a.csv'), ('b.csv')) t(path);
```

Three entry points are available:

| Entry point | Use it when |
| --- | --- |
| `duckfn::duck_vfs::with_file_system(\|fs\| …)` | You read inside one closure and nothing escapes — the least error-prone form |
| `duckfn::duck_vfs::file_system()` | You want to hold the guard; it implements `Deref<Target = FileSystem>`, so `open()` is available directly |
| `duckfn::duck_vfs::client_context()` | You need connection-level configuration or the connection ID (note that `catalog` needs an active transaction and returns `None` on an idle connection) |

`FileSystem::open` takes a `&CStr`, and the `FileHandle` it returns implements `read` / `read_exact` /
`read_to_end` / `write` / `write_all` / `seek` / `tell` / `size` / `sync` / `close`, closing itself on
drop. `duckfn::duck_vfs` re-exports `FileSystem`, `FileHandle`, `FileOpenOptions`, `FileFlag`,
`ClientContext` and `ErrorData`, so you do not have to depend on `quack-rs` directly.

## Convenience helpers (`duckfn::duck_vfs`)

Everything above is the raw form: you pick the open flags, hold the handle and push the bytes. Day to
day that is more ceremony than most callers want, so `duckfn::duck_vfs` offers Hutool-`FileUtil`-style
one-liners:

| Call | What it does |
| --- | --- |
| `duck_vfs::read(path)` | Whole file as `Vec<u8>` |
| `duck_vfs::read_string(path)` | Whole file as UTF-8 (invalid bytes are an error) |
| `duck_vfs::read_string_lossy(path)` | Same, with invalid bytes replaced by `U+FFFD` |
| `duck_vfs::read_lines(path)` | UTF-8 lines (`\n` split, trailing `\r` stripped, no empty last line) |
| `duck_vfs::write(path, bytes)` / `duck_vfs::write_string(path, text)` | Replace the file with exactly these bytes |
| `duck_vfs::write_with(path, bytes, mode)` / `duck_vfs::write_string_with(path, text, mode)` | Same, with an explicit `WriteMode` |
| `duck_vfs::append(path, bytes)` / `duck_vfs::append_string(path, text)` | Append, creating the file when missing |
| `duck_vfs::size(path)` / `duck_vfs::exists(path)` | Byte count / existence |

`WriteMode` is the interesting part:

| Mode | Semantics |
| --- | --- |
| `Replace` (default) | The file ends up holding **exactly** what you wrote — even when the old file was longer. This is where the C API's missing truncate is hidden: a longer file is zeroed with a zero-row `COPY ... TO` first, then the contents are written. |
| `FailIfExists` | Error when the file already exists, leaving it untouched. Existence is decided by an explicit check, with `EXCLUSIVE_CREATE` added on top as a guard against concurrent creators — DuckDB only turns that flag into a real `O_EXCL` on POSIX local file systems, and its Windows branch ignores it (it even reports "file not found" for a missing file), so the flag alone would not do. |
| `Append` | Append to the end, creating the file when missing. |

```rust
use duckfn::duck_vfs::{self, WriteMode};

duck_vfs::write_string("report.html", render())?;                        // replace
duck_vfs::append_string("report.log", "one more line\n")?;               // append
duck_vfs::write_string_with("once.txt", "x", WriteMode::FailIfExists)?;  // error if it exists

let text = duck_vfs::read_string("report.html")?;
let lines = duck_vfs::read_lines("report.log")?;
let bytes = duck_vfs::size("report.html")?;
```

Everything under this layer — the shared connection, the C-string conversion, the zeroing `COPY` —
is an implementation detail, and the behaviour is what callers should rely on: if DuckDB ever grows
truncate in the C API, only `duckfn::duck_vfs` changes.

There is no `delete`: the C API has neither remove nor move, and DuckDB ships no `remove_file`
function — overwrite with empty contents to clear a file. Each call takes the shared connection for
itself, so do not call `duck_vfs::*` from inside a `with_file_system` closure (that deadlocks), and
concurrent calls serialize against each other.

## Concurrency and cost

The guard holds the mutex on the owned connection:

- **Do not nest it on one thread** — `with_file_system` inside a closure that already holds
  `file_system()` deadlocks. Read several files inside a single `with_file_system` instead.
- **Take-ups serialize.** With parallel aggregation every worker goes through that one connection, so
  take it where it is cheap: once per group in `finalize`, or on the group's first row, and cache the
  result in your own state. Doing it in a per-row handler is fine for a quick `open` + `size`, but
  a long S3 read belongs in `finalize`.
- The file system itself is DuckDB's instance-level VFS and is safe to read from many threads; only
  the take-up is serialized. A `FileHandle` is **not** shareable — keep it on the thread that opened
  it and let it drop when you are done.

## Errors

`FileSystem::open`, `FileHandle` operations and `FileSystem::error_data` all return quack-rs'
`ErrorData`. Convert it into a query error with `duck_error(error.message()…)`; an aggregate's row
handler returning `Err` fails the query (the adapter reports it through
`AggregateFunctionInfo::set_error`) rather than being skipped silently.

## Limitations

- **One process-level entry, first instance wins.** With several database instances in one process,
  the VFS of the instance that finished registering first is the one used. For per-instance
  isolation, hand-write an adapter and keep your own connection in the function's extra data
  (`duckfn::DuckExtraInfo` + `quack_rs::query::OwnedConnection`); the framework does not need to be
  involved.
- **The owned connection's `FileOpener` is not the query connection's.** Instance-level
  configuration and secrets apply, but connection-level `SET`s are not guaranteed to be equivalent.
- **DuckDB 1.5.0+ and the `duckdb-1-5` feature**, as noted above; without them these functions do not
  exist.

The example extension uses this from an aggregate (`dfn_agg_file_size` in
`src/extension/functions/file_system.rs`), and `test/sql/functions/file_system.test` checks the
results against DuckDB's own `read_blob`. The `duckfn::duck_vfs` helpers — round-trips, overwriting a
longer file, append, fail-if-exists, invalid UTF-8, line splitting — are covered by
`test/sql/functions/file_util.test`.
