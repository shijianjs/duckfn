---
title: DuckDB version compatibility
sidebar_position: 1
description: How duckfn extensions relate to DuckDB versions — the C API / ABI compatibility rules, the WebAssembly build toolchain, and Rust unwinding on wasm.
---

# DuckDB version compatibility

An extension is never linked against DuckDB — it talks through the C API's function table, and DuckDB
decides at `LOAD` time whether to accept the file. "Which version does this work with" therefore splits
into a few distinct questions, each with its own answer. This section covers all of them.

## What is in this section

- **[Version numbers and the ABI](./c-api-and-abi.md)** — the three numbers behind the load decision
  (the header release, the metadata's `TARGET_DUCKDB_VERSION`, the loading engine), what the metadata
  value means under `C_STRUCT` vs `C_STRUCT_UNSTABLE`, the complete stable and unstable configuration
  sets, how far one stable build reaches (across releases *and* onto 2.0), and how to check a build.
  This is the DuckDB-version side of the story.
- **[The wasm build toolchain](./wasm-toolchain.md)** — building the `wasm_*` artifacts is a separate,
  involved version problem: the Rust the CI installs, the emsdk/binaryen it pins, and the link flags,
  across three repositories, with the exact compile / link / load failure modes and a measured
  compatibility window.
- **[Rust unwinding on WebAssembly](./rust-wasm-unwinding.md)** — why a `panic!` / `catch_unwind`
  behaves differently on wasm depending on the `rustc` version.

Start with [Version numbers and the ABI](./c-api-and-abi.md) for the everyday "which DuckDB will load
my extension" question; reach for the other two when a `wasm_*` build or a wasm `panic!` misbehaves.
