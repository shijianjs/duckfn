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
- Function documentation: `description` / `comment` / `example` on any `#[duck_*]` attribute, exported to the `function_descriptions.csv` that DuckDB's community-extension pages read (`cargo run --bin duckfn -- function_descriptions`)
- Works with DuckDB's official multi-platform extension CI

> Status: early / experimental. APIs may change before `1.0`.

## Repository layout

This repository is a Cargo workspace:

| Path | Description | Published to crates.io |
| --- | --- | --- |
| [`duckfn/`](duckfn/) | Runtime framework: traits, type adapters, function registration. | Yes |
| [`duckfn-macro/`](duckfn-macro/) | Procedural macros: `#[duck_scalar_function]`, `#[derive(DuckStruct)]`, `#[derive(DuckEnum)]`, ... | Yes |
| `/` (`rusty_quack`) | Example extension built with `duckfn`. Kept here to reuse DuckDB's official multi-platform CI. | No, example only |

## Installation

```toml
[dependencies]
duckfn = "0.0.10"

# duckfn is built on these two crates; add them explicitly when you use their
# types or builders directly (the example extension below does).
quack-rs = "0.16.0"
# `loadable-extension` dispatches through DuckDB's API table instead of linking
# libduckdb — that is what keeps a local DuckDB build unnecessary. (headers only)
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

The macros are re-exported by `duckfn`; add
[`duckfn-macro`](https://crates.io/crates/duckfn-macro) directly only if you want them without the
runtime.

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

// Generate the DuckDB extension entry point (extension name must be lowercase + underscores).
duckfn_entrypoint!("my_ext");
```

`#[duck_scalar_function]` generates the DuckDB wrapper, the logical types and — by default —
registration through `inventory`. `duckfn_entrypoint!` emits the `*_init_c_api` symbol DuckDB looks
for when loading the extension.

Everything in that snippet is safe Rust: you never write an `unsafe fn`, dereference a raw pointer,
or name a DuckDB C type.

## Documentation

The full documentation — installation, every attribute, type mapping, the error model and a
runnable example extension — lives at **<https://shijianjs.github.io/duckfn/>**:

| Page | Contents |
| --- | --- |
| [Introduction](https://shijianjs.github.io/duckfn/docs/intro) | What duckfn is, and how the crates fit together. |
| [Create a project](https://shijianjs.github.io/duckfn/docs/getting-started/create-a-project) | Start from DuckDB's official Rust extension template. |
| [Project structure](https://shijianjs.github.io/duckfn/docs/getting-started/project-structure) | The crate roots, `error[E0583]`, and the command-line tool's own root. |
| [Installation](https://shijianjs.github.io/duckfn/docs/getting-started/installation) | Dependencies, MSRV, and why no DuckDB build is needed. |
| [Quick start](https://shijianjs.github.io/duckfn/docs/getting-started/quick-start) | Write, build and load your first extension. |
| [Guide](https://shijianjs.github.io/duckfn/docs/guide/attributes) | Attributes, scalar/aggregate/table/copy functions, casts, replacement scans, SQL macros. |
| [Type mapping](https://shijianjs.github.io/duckfn/docs/guide/types) | DuckDB ↔ Rust types, nullability rules and known gaps. |
| [Errors and panics](https://shijianjs.github.io/duckfn/docs/guide/errors-and-panics) | `duck_error`, `DuckOptionResult`, and panic handling. |
| [Example extension](https://shijianjs.github.io/duckfn/docs/examples/rusty-quack) | `rusty_quack`, with runnable SQL for every feature. |
| [Community extension docs](https://shijianjs.github.io/duckfn/docs/community-extension-docs) | Function descriptions, and the CSV DuckDB's community-extension pages read. |
| [Build and release](https://shijianjs.github.io/duckfn/docs/build-and-release) · [Contributing](https://shijianjs.github.io/duckfn/docs/contributing) · [FAQ](https://shijianjs.github.io/duckfn/docs/faq) | Local builds, CI, and troubleshooting. |

中文文档：<https://shijianjs.github.io/duckfn/zh-Hans/>

API reference: <https://docs.rs/duckfn> · Library guide: [`duckfn/README.md`](duckfn/README.md)

## License

Licensed under the [MIT License](LICENSE).
