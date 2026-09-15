---
title: Installation
sidebar_position: 2
description: Add duckfn to a crate, declare the cdylib crate type, and why a local DuckDB build is not needed.
---

# Installation

## Add the dependency

```toml
[dependencies]
duckfn = "0.0.2"

# duckfn is built on these two crates. Add them explicitly as soon as you name
# their types or builders yourself — SqlMacro, Connection, LogicalType, Value, …
quack-rs = "0.16.0"
# `loadable-extension` dispatches through DuckDB's API table instead of linking
# libduckdb, which is what keeps a local DuckDB build unnecessary. Only the
# headers are used.
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

`duckfn` re-exports every macro from [`duckfn-macro`](https://crates.io/crates/duckfn-macro), so
the snippet above is enough. Depend on `duckfn-macro = "0.0.2"` directly only if you want the
macros without the runtime.

## Cargo features

| Feature | Enables | Requires |
| --- | --- | --- |
| `duckdb-1-5` | Logical types added in DuckDB 1.5 — today `TIME_NS` (`DuckTimeNs`). | `libduckdb-sys` headers from DuckDB 1.5 or newer. |

```toml
duckfn = { version = "0.0.2", features = ["duckdb-1-5"] }
```

`loadable-extension` is a feature of `libduckdb-sys`, not of `duckfn`, and it has to be enabled by
you.

## The crate must build a `cdylib`

DuckDB loads an extension as a dynamic library:

```toml
[lib]
crate-type = ["cdylib"]
```

For a WebAssembly build the crate type changes instead to `staticlib`, because the final linking is
done by `emcc`. The repository's example extension does this with a separate
`[[example]]` target that carries `crate-type = ["staticlib"]`.

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
| Rust | 1.86 or newer (the crates use edition 2024) |
| DuckDB | tested against v1.5.5 |
| Python 3 *(optional)* | only for the `make configure` / `make test` flow, which sets up the sqllogictest runner |

## Where next

- [Quick start](./quick-start.md) — write, build and load an extension.
- [Attributes](../guide/attributes.md) — the full attribute and argument reference.
- [Architecture](../internals/architecture.md) — how the API table dispatch actually works.
