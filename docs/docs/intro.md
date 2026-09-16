---
title: Introduction
sidebar_position: 1
slug: /intro
description: duckfn turns plain Rust functions into DuckDB scalar, aggregate and table functions, SQL macros, casts and replacement scans — without C/C++ glue code or a local DuckDB build.
---

# Introduction

[![GitHub](https://img.shields.io/badge/GitHub-shijianjs%2Fduckfn-181717?logo=github&logoColor=white)](https://github.com/shijianjs/duckfn)
[![Docs](https://img.shields.io/badge/docs-shijianjs.github.io%2Fduckfn-2e8555?logo=readthedocs&logoColor=white)](https://shijianjs.github.io/duckfn/)

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
