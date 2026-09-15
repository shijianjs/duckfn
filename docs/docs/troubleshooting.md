---
title: Troubleshooting
sidebar_position: 9
description: The IDE flagging the WebAssembly entry file, E0583 in a nested module tree, the Rust 1.86 pin in the official CI, and an upstream bug that corrupts all-NULL list literals.
---

# Troubleshooting

Four things that are none of duckfn's doing, but that you will run into. Each one says what you see,
why it happens, and what to do about it.

## IDE errors in the WebAssembly entry file

**What you see.** RustRover or rust-analyzer marks up `src/wasm_lib.rs` with errors that `make debug`,
`just build` or `cargo duckdb-ext build` never reproduce.

**Why.** `Cargo.toml` registers that file as an example:

```toml
[[example]]
# crate-type can't be (at the moment) be overriden for specific targets
path = "src/wasm_lib.rs"
crate-type = ["staticlib"]
```

`crate-type` cannot be chosen per target, and the two targets need different ones — `cdylib` when
compiling natively, `staticlib` for WebAssembly — so the template carries a second crate root that is
only built when cross-compiling:

```shell
just build_wasm     # cargo build --release --target wasm32-unknown-emscripten --example rusty_quack
```

An IDE checks *every* target by default (`cargo check --all-targets`), which compiles that example for
your host platform as well — a configuration it was never written for.

**Fix.** Gate the file on the target architecture. On any other target it compiles to nothing and the
errors disappear:

```rust
#![cfg(target_arch = "wasm32")]
#![allow(special_module_name)]

mod extension;
```

The second attribute silences the lint about a crate root that is not named `lib.rs`.

## Nested modules fail with E0583

**What you see.** The build breaks as soon as a module gains a submodule — for example when a second
level appears under `src/`:

```
error[E0583]: file not found for module `demo`
 --> src\lib.rs:3:1

error[E0583]: file not found for module `types`
 --> src\lib.rs:4:1
```

**Why.** The official template makes the WebAssembly root re-export the native one:

```rust
// src/wasm_lib.rs, official template
mod lib;
```

`mod lib;` resolves to `src/lib.rs`, and from that point on `lib.rs` is a **file** module: its children
are looked up *beside* it, under `src/lib/`. So a `mod demo;` written inside `src/lib.rs` is searched
for at `src/lib/demo.rs` instead of `src/extension/demo.rs`, and rustc reports E0583 on the `mod` line.
A flat `lib.rs` hides the problem; nesting exposes it.

**Fix: mirror the two roots.** Both crate roots declare the same path, and every module lives under
`src/extension/`:

```
src/
├─ lib.rs              mod extension;                 native, crate-type = ["cdylib"]
├─ wasm_lib.rs         mod extension;                 wasm example, crate-type = ["staticlib"]
└─ extension/
   ├─ mod.rs           mod demo; mod functions; mod types;
   │                   duckfn_entrypoint!("rusty_quack");
   ├─ demo/
   ├─ functions/
   └─ types/
```

`mod extension;` resolves to `src/extension/mod.rs` — a *directory* module — so both roots see the same
tree, and `mod demo;` inside `src/extension/mod.rs` is looked up at `src/extension/demo.rs` or
`src/extension/demo/mod.rs`. Nesting works at any depth, because nothing resolves relative to a file
module any more.

**Rule.** Keep the two roots identical, never re-export one from the other, put `duckfn_entrypoint!` in
`src/extension/mod.rs`, and hang everything else off it. That is what this repository does, and it is
why `src/lib.rs` and `src/wasm_lib.rs` are three lines each.

## WASM builds on the official CI are pinned to Rust 1.86

**What you see.** On a version tag the distribution pipeline builds every native platform and the
WebAssembly job fails — sometimes before your own crate is compiled at all:

```
error[E0658]: `let` expressions in this position are unstable
  --> ar_archive_writer-0.5.0/src/archive_writer.rs:591:20
```

**Why.** The reusable workflow this repository calls
(`duckdb/extension-ci-tools/.github/workflows/_extension_distribution.yml@v1.5-variegata`, wired up in
`.github/workflows/MainDistributionPipeline.yml`) hard-codes the WebAssembly toolchain:

```yaml
- name: Setup Rust for cross compilation
  uses: dtolnay/rust-toolchain@1.86.0
  with:
    targets: wasm32-unknown-emscripten
```

Only the wasm job is pinned, and it is pinned inside the reusable workflow, so an extension repository
cannot raise it without forking. Anything in the dependency graph that needs a language feature newer
than 1.86 — let-chains in `ar_archive_writer 0.5.x`, in the reported case — fails there while
`linux_amd64`, `osx_*` and `windows_*` all pass.

**Status.** Upstream: [`duckdb/extension-ci-tools#385`](https://github.com/duckdb/extension-ci-tools/issues/385).
Still open as of 2026-09-15, and the proposed fixes — raise the pin, or expose the toolchain version as
a workflow input — are not merged. Until then you either keep the dependency graph buildable by 1.86,
or skip the WebAssembly targets with `exclude_archs` (`wasm_eh` and the other wasm variants) when you
do not ship them.

## All-NULL list literals arrive corrupted

**What you see.**

```sql
SELECT dfn_echo_list_integer_n([NULL, NULL, NULL, NULL]);
-- [NULL, 0, 0, 0]                    expected [NULL, NULL, NULL, NULL]

SELECT dfn_echo_map_varchar_integer_n(map(['a', 'b'], [NULL, NULL]));
-- {a=NULL, b=0}                     expected {a=NULL, b=NULL}
```

Those zeros are uninitialised memory: a different process can print different values.

**Why.** This is an upstream DuckDB bug, not a conversion mistake in duckfn —
[`duckdb/duckdb#25616`](https://github.com/duckdb/duckdb/issues/25616). The list's child vector stays
a *constant* NULL vector: a logical length of four backed by a single physical element. DuckDB's own
code flattens such vectors (`UnifiedVectorFormat`, `Vector::Flatten()`), but the C API exposes only
`duckdb_vector_get_data()` and `duckdb_vector_get_validity()`, with no vector representation and no
logical-index accessor — so an extension that indexes the child vector by logical index reads past the
buffer after the first element.

**When it triggers.** Only for literals in which *every* element is an untyped `NULL`:

| Expression | Result |
| --- | --- |
| `[NULL, NULL, NULL, NULL]` | corrupted |
| `[NULL, NULL, NULL, NULL]::INTEGER[]` | corrupted — typing the whole list does not help |
| `[NULL, NULL::INTEGER, NULL, NULL]` | correct |
| `[NULL::INTEGER, NULL::INTEGER, NULL::INTEGER, NULL::INTEGER]` | correct |
| `[1, 1, 1, 1]` | correct |
| a list produced by a table or by an expression | correct |

**Workaround.** Type at least one *element* — `[NULL, NULL::INTEGER, NULL, NULL]` — rather than the
list, or hand the function a value that comes from a query instead of a literal. Reported against
DuckDB v1.5.4 and v1.5.5, still open upstream.

## See also

- [FAQ](./faq.md) — the errors people hit while writing functions.
- [Build and release](./build-and-release.md) — what the pipeline does on a version tag.
- [Architecture](./internals/architecture.md) — how registration and dispatch actually work.
