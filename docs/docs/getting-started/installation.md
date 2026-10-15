---
title: Installation
sidebar_position: 3
description: Add duckfn to a crate, declare the cdylib crate type, and why a local DuckDB build is not needed.
---

# Installation

## Add the dependency

```toml
[dependencies]
duckfn = "{{DUCKFN_VERSION}}"

# duckfn is built on these two crates. Add them explicitly as soon as you name
# their types or builders yourself — SqlMacro, Connection, LogicalType, Value, …
quack-rs = "0.18.0"
# `loadable-extension` dispatches through DuckDB's API table instead of linking
# libduckdb, which is what keeps a local DuckDB build unnecessary. Only the
# headers are used.
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

`duckfn` re-exports every macro from [`duckfn-macro`](https://crates.io/crates/duckfn-macro), so
the snippet above is enough. Depend on `duckfn-macro = "{{DUCKFN_VERSION}}"` directly only if you want the
macros without the runtime.

## Cargo features

| Feature | Enables | Requires |
| --- | --- | --- |
| `duckdb-1-5` | Logical types added in DuckDB 1.5 — today `TIME_NS` (`DuckTimeNs`) — plus the copy-function types behind `COPY ... TO`. | `libduckdb-sys` headers from DuckDB 1.5 or newer. |
| `sql` | Running SQL from a callback (`duckfn::sql`): one long-lived connection captured at registration, then `execute` / `query` — with parameter binding — and the result read back as dynamic columns. Uses **only the stable C API**, so it does not need `duckdb-1-5`: `USE_UNSTABLE_C_API=0` with a `v1.2.0` floor gives one binary for DuckDB 1.3 through 2.0. Has side effects: the connection lives as long as the process, take-ups serialize, and it deadlocks a runtime that executes DuckDB synchronously on one thread (DuckDB-Wasm's Node *blocking* bindings) — see [Running SQL](../guide/sql-execution.md#cost-and-limits). | Nothing beyond the crate's own `libduckdb-sys`: every call stays in the stable C API region, which is what keeps the artifact loadable across releases. |
| `duck_vfs` | Host file-system access (`duckfn::duck_vfs`): on top of that registration-time connection, DuckDB's VFS — so callbacks without a client context, aggregates above all, can read and write files, `s3://` and `http(s)://` included. Implies `owned-connection` and `duckdb-1-5`. Has the same side effects as `sql` — see [File system access](../guide/file-system.md#what-it-costs). | Otherwise the same headers as `duckdb-1-5`. |
| `owned-connection` | The base the two callback-side features share: one long-lived connection captured at registration. It exposes **no API of its own** — `sql` and `duck_vfs` are its two public faces and both depend on it, while it depends on neither. Name it directly only when you want to be explicit about that base. | Nothing. |
| `chrono` | Conversions between the time wrapper types and [`chrono`](https://crates.io/crates/chrono) (`DuckDate::to_naive_date` and friends), so the epoch arithmetic lives in duckfn. | An optional `chrono` dependency, pulled in by this feature. |
| `uuid` | Conversions between `DuckUuid` and [`uuid`](https://crates.io/crates/uuid) (`to_uuid` / `from_uuid`). | An optional `uuid` dependency. |
| `rust_decimal` | Conversions between `DuckDecimal<W, S>` and [`rust_decimal`](https://crates.io/crates/rust_decimal); out-of-range and digit-losing values are errors. | An optional `rust_decimal` dependency. |
| `quack` | The marker feature of *this repository's* example extension and CLI: it decides whether those two targets are compiled at all, never what they can do — that comes from naming capabilities. A dependent never needs it. | The example's capability list, which the build passes alongside it. |

Every feature is named after the module it exposes, and there is **no aggregate switch**: a
dependency names exactly what it uses. That is deliberate — an "everything on" feature says nothing
about what it costs (the two connection-carrying ones have side effects; `duckdb-1-5` pins the build
to an engine release) and hides which capability a build actually ended up with.

The three interop features are independent — enable only the crates you actually use:

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["sql", "chrono", "uuid", "rust_decimal"] }
```

`cli` only adds `clap` and `csv` for the CLI binary, so it belongs in the crate that builds
`src/bin/duckfn.rs` — not in a dependency tree you want to keep lean.

`loadable-extension` is a feature of `libduckdb-sys`, not of `duckfn`, and it has to be enabled by
you.

## The crate must build a `cdylib`

DuckDB loads an extension as a dynamic library:

```toml
[lib]
crate-type = ["cdylib"]
```

For a WebAssembly build the crate type changes instead to `staticlib`, because the final linking is
done by `emcc`. Leave the library alone and give the wasm target a crate root of its own — a
`[[example]]` carrying `crate-type = ["staticlib"]`, as described in
[Project structure](./project-structure.md).

## Why no local DuckDB build

Normally a DuckDB extension is compiled against, and linked to, DuckDB itself. `duckfn` avoids that:

1. `libduckdb-sys` with `loadable-extension` compiles against DuckDB **headers only** — no library
   is linked.
2. DuckDB API calls go through a function pointer table that the host DuckDB fills in when it loads
   the extension and calls the entry point.

The consequence is a fast, dependency-free build (and cross-compilation that works), with one
trade-off: the extension must match the DuckDB version it is loaded into. The extension is also
marked as using the *unstable* C API, which is why loading it requires `-unsigned`.

## Requirements

| | |
| --- | --- |
| Rust | 1.88 or newer (the crates use edition 2024; the floor comes from `syn 3` in `duckfn-macro`, which needs rustc 1.88) |
| DuckDB | tested against v1.5.6 |
| Python 3 *(optional)* | only for the `make configure` / `make test` flow, which sets up the sqllogictest runner |

## Where next

- [Quick start](./quick-start.md) — write, build and load an extension.
- [Attributes](../guide/attributes.md) — the full attribute and argument reference.
- [Architecture](../development/architecture.md) — how the API table dispatch actually works.
