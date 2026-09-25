<div align="center">
  <img src="https://raw.githubusercontent.com/shijianjs/duckfn/main/docs/static/img/duckfn-logo.svg" alt="duckfn logo" width="180" />
</div>

# duckfn

[English](README.md) | [简体中文](README.zh-CN.md)

[![GitHub](https://img.shields.io/badge/GitHub-181717?logo=github&logoColor=white)](https://github.com/shijianjs/duckfn)
[![Docs](https://img.shields.io/badge/docs-duckfn-14459b?logo=docusaurus&logoColor=white)](https://shijianjs.github.io/duckfn/)
[![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
[![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)

**Write DuckDB extensions in plain Rust.**

`duckfn` is a Rust framework for building [DuckDB](https://duckdb.org) extensions on top of
DuckDB's C Extension API. A single attribute turns an ordinary Rust function into a DuckDB
**scalar**, **aggregate**, **table** or **copy function**, a SQL macro, a replacement scan, a type
cast, or a nested type — no C/C++ glue code, and no local DuckDB build required.

- Repository: <https://github.com/shijianjs/duckfn>
- Crates: [`duckfn`](https://crates.io/crates/duckfn) · [`duckfn-macro`](https://crates.io/crates/duckfn-macro)
- Built on: [`quack-rs`](https://crates.io/crates/quack-rs) · [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys)
- No `unsafe` to write: no `unsafe fn`, no raw pointers in your function bodies
- No DuckDB build required, no C/C++ code
- Attribute-driven registration through `inventory`
- Panic-safe: Rust panics become DuckDB errors instead of unwinding across the FFI boundary
- Host file system access: read and write through DuckDB's virtual file system (`s3://`, `http(s)://` with `httpfs`, in-memory) from any callback — aggregate functions included — plus one-line helpers such as `duckfn::duck_vfs::read_string` / `write_string` / `append_string` (`duckdb-1-5` feature)
- Interop with other crates: the time wrapper types (`DuckDate`, `DuckTimestamp` / `_S` / `_Ms` / `_Ns`, `DuckTimestampTz`, `DuckTime`) convert to and from [`chrono`](https://crates.io/crates/chrono), `DuckUuid` ↔ [`uuid`](https://crates.io/crates/uuid), and `DuckDecimal<W, S>` ↔ [`rust_decimal`](https://crates.io/crates/rust_decimal) (`chrono` / `uuid` / `rust_decimal` features) — out-of-range values, DuckDB's `infinity` and digit-losing conversions come back as errors, never as a panic or a silent truncation
- Function documentation: `description` / `comment` / `example` on any `#[duck_*]` attribute, exported to the `function_descriptions.csv` that DuckDB's community-extension pages read (`cli` feature)
- Works with DuckDB's official multi-platform extension CI

> Status: early / experimental. APIs may change before `1.0`.

## Repository layout

The repository is a Cargo workspace whose root is also the `duckfn` crate root:

| Path | Description | Published to crates.io |
| --- | --- | --- |
| `/` (`duckfn`) | Runtime framework: traits, type adapters, function registration. Also the workspace root. | Yes |
| `src/extension/`, `test/sql/` | The example extension (`duckfn`): runnable SQL for every feature, its three entry points (`src/lib.rs`, `src/wasm_lib.rs`, `src/bin/duckfn.rs`) and its sqllogictest suite. Part of this package, compiled only with the `quack` feature. | Yes — sources only, never compiled for a dependent |
| [`duckfn-macro/`](duckfn-macro/) | Procedural macros: `#[duck_scalar_function]`, `#[derive(DuckStruct)]`, `#[derive(DuckEnum)]`, ... | Yes |
| [`docs/`](docs/) | Docusaurus documentation site: `docs/docs/**` (English) and `docs/i18n/zh-Hans/**` (Simplified Chinese). | No, docs site — but its sources ship inside the `duckfn` package |

The published `duckfn` package carries the runtime, its tests, this README, the license, the whole
documentation source and the example extension with its sqllogictest suite, so the guide, the example
page and a complete worked extension are readable without cloning the repository
(`cargo package --list` in the repository root prints the exact file list). The example is compiled
only when the `quack` feature is on — off by default — so a dependency on `duckfn` compiles none of
it. Keeping the example inside this package is also what lets cargo ship it at all: cargo never
packages a subdirectory that contains its own `Cargo.toml`.

## Installation

```toml
[dependencies]
duckfn = "0.0.10"

# duckfn itself is built on these two crates; add them explicitly when you use
# their types or builders directly.
quack-rs = "0.16.0"
# `loadable-extension` dispatches through DuckDB's API table instead of linking
# libduckdb, which is what keeps a local DuckDB build unnecessary.
# (headers only — no linked library)
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

If you prefer the macros without the runtime, depend on
[`duckfn-macro`](https://crates.io/crates/duckfn-macro) directly; otherwise the macros are
re-exported by `duckfn` and no extra dependency is needed.

`duckfn` has seven optional features. `cli` adds the command-line tool that exports a
`function_descriptions.csv` for DuckDB's community-extension pages
(`cargo run --features quack --bin duckfn-cli -- function_descriptions` in this repository); only an
extension project's `src/bin/duckfn.rs` needs it. `duckdb-1-5` enables what DuckDB's 1.5 C API
added: the logical types from DuckDB 1.5 (currently `TIME_NS`), copy functions, and host
file-system access. The other three are interop and independent of each other: `chrono` converts
the time wrapper types to and from [`chrono`](https://crates.io/crates/chrono), `uuid` converts
`DuckUuid` to and from [`uuid`](https://crates.io/crates/uuid), and `rust_decimal` converts
`DuckDecimal<W, S>` to and from [`rust_decimal`](https://crates.io/crates/rust_decimal) — so the
epoch / 128-bit / scaled-integer arithmetic lives in duckfn rather than in every extension:

```toml
duckfn = { version = "0.0.10", features = ["duckdb-1-5", "chrono", "uuid", "rust_decimal"] }
```

`all` is the aggregate switch: it turns all five of the real features on at once. duckfn normally
sits at the end of the dependency tree, so `features = ["all"]` is the convenient spelling; pick the
individual features above when you want a leaner tree.

`quack` is the one feature that is not meant for dependents: it compiles this package's own example
extension (`src/extension/`). It depends on `all`, never the other way round, so asking for `all` does
not drag the example and its test functions into your build.

## Quick start

```rust
use duckfn::{duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult};

/// ```sql
/// SELECT double_it(21);    -- 42
/// SELECT double_it(NULL);  -- NULL
/// SELECT double_it(13);    -- error: unlucky input
/// ```
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

// Generate the extension entry point (name must be lowercase + underscores).
duckfn_entrypoint!("my_ext");
```

The wrapper, the logical types and the registration are all generated, so the snippet above is
entirely safe Rust. The only place `unsafe` shows up is manual registration:
`#[duck_custom_register]` calls quack-rs' `unsafe fn register_scalar` / `register_aggregate` /
`register_table`.

Calling the generated module of a `#[duck_scalar_function]` exposes builders such as
`scalar_function_builder()` and `scalar_overload_builder()`, so you can register overloads or
function sets yourself.

## Documentation

The full guide — every attribute and its arguments, the type mapping, the error model, and a
runnable example extension — lives at **<https://shijianjs.github.io/duckfn/>**:

| Page | Contents |
| --- | --- |
| [Introduction](https://shijianjs.github.io/duckfn/docs/intro) | What duckfn is, and how the crates fit together. |
| [Create a project](https://shijianjs.github.io/duckfn/docs/getting-started/create-a-project) | Start from the [duckfn extension template](https://github.com/shijianjs/duckfn-extension-template) or DuckDB's official Rust one. |
| [Project structure](https://shijianjs.github.io/duckfn/docs/getting-started/project-structure) | The crate roots, `error[E0583]`, and the command-line tool's own root. |
| [Installation](https://shijianjs.github.io/duckfn/docs/getting-started/installation) | Dependencies, MSRV, and why no DuckDB build is needed. |
| [Quick start](https://shijianjs.github.io/duckfn/docs/getting-started/quick-start) | Write, build and load your first extension. |
| [Attributes](https://shijianjs.github.io/duckfn/docs/guide/attributes) | All attributes, shared arguments, and manual registration. |
| [Scalar functions](https://shijianjs.github.io/duckfn/docs/guide/scalar-functions) | Return shapes, `NULL` handling, batch mode, overloads. |
| [Aggregate functions](https://shijianjs.github.io/duckfn/docs/guide/aggregate-functions) | Row handlers, state types, parallel aggregation. |
| [Table functions](https://shijianjs.github.io/duckfn/docs/guide/table-functions) | Row structs, named parameters, streaming. |
| [Copy functions](https://shijianjs.github.io/duckfn/docs/guide/copy-functions) | Custom file formats for `COPY ... TO` / `COPY ... FROM`, built on runtime dynamic columns. |
| [Type casts](https://shijianjs.github.io/duckfn/docs/guide/casts) | Overriding `CAST` for one source/target pair. |
| [Replacement scans](https://shijianjs.github.io/duckfn/docs/guide/replacement-scans) | Making `SELECT * FROM 'data.points'` work. |
| [SQL macros](https://shijianjs.github.io/duckfn/docs/guide/sql-macros) | Macros from Rust or from `.sql` files. |
| [Type mapping](https://shijianjs.github.io/duckfn/docs/guide/types) | DuckDB ↔ Rust types, nullability and known gaps. |
| [Community extension docs](https://shijianjs.github.io/duckfn/docs/community-extension-docs) | The `description` / `comment` / `example` attributes, and the CSV DuckDB's community-extension pages read. |
| [Errors and panics](https://shijianjs.github.io/duckfn/docs/guide/errors-and-panics) · [Architecture](https://shijianjs.github.io/duckfn/docs/internals/architecture) | Error handling, expansion, registration and adapters. |
| [Example extension](https://shijianjs.github.io/duckfn/docs/examples/duckfn) | `duckfn`, the example shipped with this package, with runnable SQL for every feature. |
| [Build and release](https://shijianjs.github.io/duckfn/docs/build-and-release) · [Contributing](https://shijianjs.github.io/duckfn/docs/contributing) · [FAQ](https://shijianjs.github.io/duckfn/docs/faq) | Local builds, CI, and troubleshooting. |

中文文档：<https://shijianjs.github.io/duckfn/zh-Hans/>

Rust API reference: <https://docs.rs/duckfn>

## License

Licensed under the [MIT License](LICENSE).
