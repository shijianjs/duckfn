---
title: Create a project
sidebar_position: 1
description: Start from the duckfn extension template or DuckDB's official Rust extension template, write with duckfn and quack-rs, and build with cargo-duckdb-ext-tools.
---

# Create a project

Three decisions come before writing any code: what to start from, what to write with, and how to
build.

## Start from a template

Both templates below are the same skeleton — DuckDB's CI, `extension-ci-tools`, a `cdylib` crate
root, a sqllogictest directory. They differ in how much is already wired up for duckfn.

### The duckfn template

[`shijianjs/duckfn-extension-template`](https://github.com/shijianjs/duckfn-extension-template) is
this project's own template: the official skeleton with the duckfn-specific work already done.

```shell
git clone https://github.com/shijianjs/duckfn-extension-template my_ext
cd my_ext
rm -rf .git && git init    # optional: drop the template's history and start your own
just rename my_ext
```

`just rename` (that is, `scripts/rename.sh`) is the reason the template exists. The extension name
has to agree in several places at once, and missing one is not a compile error — it is an extension
that will not load, because DuckDB derives the entry-point symbol from the file name, so a mismatch
fails at `LOAD` with nothing useful to go on. The script rewrites all of them in one pass:

- `[package] name` and the `[[example]] name` in `Cargo.toml`;
- `EXTENSION_NAME` in the `Makefile`;
- the name passed to `duckfn_entrypoint!(…)`, which becomes the entry-point symbol
  `my_ext_init_c_api`;
- `extension_name` in the `Justfile` and in the CI workflow;
- the path examples in the README and the documentation site, plus the `Cargo.lock` entry.

It ends by printing what is still left for a human — replacing the two sample functions being the
main item. Everything else is there already: `duckfn` in the dependency list with
`loadable-extension` enabled, `src/lib.rs` and `src/wasm_lib.rs` declaring the same set of modules, a
scalar and an aggregate sample function with SQLLogicTest files for both, the `Justfile`, a
two-language Docusaurus site under `docs/` (deletable — nothing else depends on it), the release
scripts, and the `community-extension/` staging files. Its `README.md` and `DEVELOPMENT.md` describe
the loop, and its `AGENTS.md` is already the one from
[Brief your coding agent](#brief-your-coding-agent).

### The official template

[`duckdb/extension-template-rs`](https://github.com/duckdb/extension-template-rs) is DuckDB's own
Rust extension template, and the base both templates derive from. Start here if you would rather do
the wiring yourself — but keep it in mind either way, because it is where the makefiles and the CI
ultimately come from:

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

Then rename what the template hard-codes — `EXTENSION_NAME` in the `Makefile`, the `[[example]]`
target if you also build for WebAssembly, and the name passed to `duckfn_entrypoint!` — which is the
same list `just rename` automates, plus the `Justfile` and CI entries it would not know about yet.

Whichever template you start from, leave the crate roots alone: `src/lib.rs`, `src/wasm_lib.rs` and
`src/bin/duckfn.rs` all point at the same `src/extension/` module, and that is the one rule the
layout follows — see [Project structure](./project-structure.md) for why, and what `error[E0583]`
means when it is broken.

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

The [duckfn template](#the-duckfn-template) wraps both flows in a
[`Justfile`](https://github.com/shijianjs/duckfn-extension-template/blob/main/Justfile): `just build`,
`just sql "SELECT …"`, `just repl`, `just test`, `just ci-release`, plus the `docs_*` and release
recipes. `just rename` has already set `extension_name` to whatever `duckfn_entrypoint!` declares.

## When the official flow is still needed

Building with Cargo covers development, but two things expect `make`:

- The sqllogictest suite (`make test`), which is the standard way to test a DuckDB extension, and
  better suited to extension behaviour than a Rust test harness.
- CI, which runs the same set of makefiles.

The usual arrangement is therefore: get `make configure` working once, build with Cargo while
iterating, and run `make test` before pushing. On Windows that means running `make` from Git Bash
rather than PowerShell — see [Contributing](../contributing.md#windows).

## Brief your coding agent

If an AI agent writes the extension, give it a project-level `AGENTS.md`. The
[duckfn template](#the-duckfn-template) ships one: fill in its two placeholders — what the extension
does and where the duckfn clone lives — and that is the whole setup. Starting from anywhere else,
copy that file:
[`AGENTS.md`](https://github.com/shijianjs/duckfn-extension-template/blob/main/AGENTS.md).

It exists because of what a dependency does and does not carry. `cargo` unpacks `duckfn` and
`duckfn-macro` into the local registry, so the runtime and the macro implementations — the ground
truth for which attribute accepts which arguments and which return shapes it allows — are on disk
and readable. The published `duckfn` package is more than that: it also carries the documentation
sources under `docs/docs/**` with their Simplified Chinese translations *and* the example extension —
`src/extension/**` together with its sqllogictest suite under `test/sql/` — so the guide, the example
page and a complete worked extension can all be read straight out of the unpacked crate. See
[Build and release](../build-and-release.md) for the exact file list.

What the crate does not carry is the repository around them: the issue history, the CI workflows, the
documentation site's own tooling, and the `AGENTS.md` file itself. That is why the `AGENTS.md` points
the agent at a local clone of this repository and tells it to stop and ask rather than invent an
attribute it cannot find — the clone's path is one of the two placeholders to fill in.

Nothing that can be read out of the code is written down in it — extension name, crate name, duckfn
version — so it cannot go stale.

## Next

- [Installation](./installation.md) — add duckfn to the crate.
- [Quick start](./quick-start.md) — write, build and load the extension.
- [Build and release](../build-and-release.md) — what the template's CI does on a version tag.
