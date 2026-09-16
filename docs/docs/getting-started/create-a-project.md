---
title: Create a project
sidebar_position: 1
description: Start from DuckDB's official Rust extension template, write with duckfn and quack-rs, and build with cargo-duckdb-ext-tools.
---

# Create a project

Three decisions come before writing any code: what to start from, what to write with, and how to
build.

## Start from the official template

[`duckdb/extension-template-rs`](https://github.com/duckdb/extension-template-rs) is DuckDB's own
Rust extension template, and it is the right base for a new extension:

- It ships a complete GitHub Actions pipeline that builds and tests the extension for every platform
  DuckDB supports and publishes the binaries on a version tag. There is nothing to write.
- It wires up `extension-ci-tools`, which provides `make configure` / `make debug` / `make release` /
  `make test` — the same flow DuckDB itself uses, and the one the sqllogictest runner needs.
- The repository you are reading is a template like that, with the functions replaced.

```shell
git clone --recurse-submodules https://github.com/duckdb/extension-template-rs my_ext
cd my_ext
```

`extension-ci-tools` is a git submodule, so clone with `--recurse-submodules`, or run
`git submodule update --init --recursive` before the first build.

Then rename what the template hard-codes: `EXTENSION_NAME` in the `Makefile`, the `[[example]]`
target if you also build for WebAssembly, and the name passed to `duckfn_entrypoint!`.

While you do it, keep the two crate roots mirrored: `src/lib.rs` and `src/wasm_lib.rs` have to declare
the same modules, and the official template's `mod lib;` re-export starts failing with
`error[E0583]` as soon as you nest modules — see
[Troubleshooting](../troubleshooting.md#nested-modules-fail-with-e0583).

## Write with duckfn and quack-rs

`duckfn` is the layer this repository provides. It sits on top of
[`quack-rs`](https://github.com/tomtom215/quack-rs) and turns an ordinary Rust function into an
extension function with a single attribute:

```rust
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    Ok(v.map(|x| x * 2))
}
```

Why not use the official `duckdb` crate directly? Because its extension API only covers two kinds of
functions — scalar functions behind the `vscalar` feature and table functions behind `vtab`, which is
the entire surface of this line:

```toml
duckdb = { version = "~1.10505.0", features = ["loadable-extension", "vscalar"] }
```

Aggregate functions, SQL macros, replacement scans, casts and nested types have no registration API
at all. That gap is what `duckfn` fills: one attribute system covering scalar functions, aggregate
functions, table functions, SQL macros, replacement scans and casts.

Reach for `quack-rs` directly when you need something the attributes do not expose — a hand-built
`LogicalType`, a vector-level operation, a corner of the C API. It is already in your dependency
list.

[Same functions, two ways](../examples/side-by-side.md) takes four functions from the official
template and the `quack-rs` example and writes each one both ways.

## Build with cargo-duckdb-ext-tools

For day-to-day work,
[`redraiment/cargo-duckdb-ext-tools`](https://github.com/redraiment/cargo-duckdb-ext-tools) packages
the extension straight from Cargo:

```shell
cargo install cargo-duckdb-ext-tools   # once
cargo duckdb-ext build                 # -> target/debug/my_ext.duckdb_extension
```

It is a global `cargo` subcommand, so it adds **no dependencies** to the project, and it does not
need the template's `make` flow to be working:

```shell
duckdb -unsigned -c "
LOAD './target/debug/my_ext.duckdb_extension';
SELECT double_it(21);
"
```

The repository wraps both flows in its `Justfile`; see [Quick start](./quick-start.md#3-build-it).

## When the official flow is still needed

Building with Cargo covers development, but two things expect `make`:

- The sqllogictest suite (`make test`), which is the standard way to test a DuckDB extension, and
  better suited to extension behaviour than a Rust test harness.
- CI, which runs the same set of makefiles.

The usual arrangement is therefore: get `make configure` working once, build with Cargo while
iterating, and run `make test` before pushing. On Windows that means running `make` from Git Bash
rather than PowerShell — see [Contributing](../contributing.md#windows).

## Next

- [Installation](./installation.md) — add duckfn to the crate.
- [Quick start](./quick-start.md) — write, build and load the extension.
- [Build and release](../build-and-release.md) — what the template's CI does on a version tag.
