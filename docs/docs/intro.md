---
title: Introduction
sidebar_position: 1
slug: /intro
description: duckfn turns plain Rust functions into DuckDB scalar, aggregate and table functions, SQL macros, casts and replacement scans — without C/C++ glue code or a local DuckDB build.
---

# Introduction

[![GitHub](https://img.shields.io/badge/GitHub-181717?logo=github&logoColor=white)](https://github.com/shijianjs/duckfn)
[![Docs](https://img.shields.io/badge/docs-duckfn-2e8555?logo=docusaurus&logoColor=white)](https://shijianjs.github.io/duckfn/)
[![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
[![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)


**Write DuckDB extensions in plain Rust.**

`duckfn` is a framework for building [DuckDB](https://duckdb.org) extensions on top of DuckDB's
C Extension API. A single attribute turns an ordinary Rust function into a DuckDB **scalar**,
**aggregate** or **table function**, a SQL macro, a replacement scan, or a type cast.

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

// Generates the symbol DuckDB looks for when loading the extension.
duckfn_entrypoint!("my_ext");
```

The attribute generates the FFI wrapper, the column readers and writers, and the registration code,
so everything above is safe Rust.

## Why duckfn

| | |
| --- | --- |
| **No C/C++ glue code** | DuckDB C types never appear in your code. You write `Option<i64>`, `Vec<String>` and `#[derive(DuckStruct)]` structs. |
| **No local DuckDB build** | The extension is compiled against headers only and dispatches through DuckDB's API table at load time. |
| **Safe by default** | No `unsafe fn` and no raw pointers in your function bodies. The only `unsafe` left is in explicit manual registration. |
| **Attribute-driven registration** | Annotating a function is enough; registration items are collected with `inventory` and applied when DuckDB loads the extension. |
| **Panic-safe** | A Rust panic inside a function body is caught and reported as a DuckDB error instead of unwinding across the FFI boundary. |
| **Nested types included** | `Vec<T>`, `IndexMap<K, V>`, fixed-size arrays, `STRUCT`s and any nesting of them map to DuckDB's LIST, MAP, ARRAY and STRUCT. |
| **Fits DuckDB's CI** | The repository reuses DuckDB's official multi-platform extension pipeline, so a version tag produces binaries for every supported platform. |

## How the pieces fit together

| Crate | Role |
| --- | --- |
| [`duckfn`](https://crates.io/crates/duckfn) | Runtime framework: traits, type adapters, value types, registration. Re-exports every macro. |
| [`duckfn-macro`](https://crates.io/crates/duckfn-macro) | The procedural macros behind `#[duck_scalar_function]`, `#[derive(DuckStruct)]`, … |
| [`quack-rs`](https://crates.io/crates/quack-rs) | The DuckDB C API bindings `duckfn` builds on; its builders and value types are part of the public surface. |
| [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys) | DuckDB headers; with the `loadable-extension` feature nothing is linked at build time. |

## Status

> Early / experimental — APIs may change before `1.0`.

## Where to go next

- [Create a project](./getting-started/create-a-project.md) — start from DuckDB's official Rust extension template.
- [Installation](./getting-started/installation.md) — dependencies, MSRV, and why no DuckDB build is needed.
- [Quick start](./getting-started/quick-start.md) — build and load your first extension.
- [Attributes](./guide/attributes.md) — the full attribute and argument reference.
- [The example extension](./examples/rusty-quack.md) — a working extension covering every feature.
