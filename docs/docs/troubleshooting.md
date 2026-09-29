---
title: Troubleshooting
sidebar_position: 10
description: Rust 1.86 pinning in the official CI's WebAssembly build, the upstream bug that corrupts all-NULL list literals, and why a `panic!` is unusable on WebAssembly.
---

# Troubleshooting

Three things that are largely none of duckfn's doing, but that you will run into. Each one says what
you see, why it happens, and what to do about it.

Problems with the *layout* — the crate roots, `error[E0583]`, and the IDE flagging a separate wasm
root — live in [Project structure](./getting-started/project-structure.md) instead,
because they are about how the project is put together rather than about something going wrong.

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

```sql {"type":"duckfn"}
SELECT dfn_echo_list_integer_n([NULL, NULL, NULL, NULL]);
-- [NULL, 0, 0, 0]                    expected [NULL, NULL, NULL, NULL]
```

```sql {"type":"duckfn"}
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

## A `panic!` inside a function is unusable on WebAssembly

**What you see.** On the native CLI (and in `just test`) a function that panics reports a readable
message. The *same* call in the browser (DuckDB-Wasm) fails with:

```
RangeError: Maximum call stack size exceeded
```

— with no trace of your panic message at all. (The extension survives: a later call on the same
connection still works; it is only that one call that comes back as a stack overflow.)

**Why.** duckfn and quack-rs *do* guard against this: every callback body runs inside
`std::panic::catch_unwind`, and the crate is built with `panic = "unwind"`. That is exactly why a
`panic!` reports a readable message on native — `catch_unwind` catches the unwind and the adapter turns
it into a DuckDB error. The problem is the target, not the code: `wasm32-unknown-emscripten` cannot
unwind across the wasm↔JS boundary (that needs `-C panic=unwinding` plus wasm exception handling /
emscripten `-fexceptions`, which duckdb-wasm's side-module toolchain does not enable). So on wasm the
`catch_unwind` landing pad never sees the panic — it escapes into the JS shim and exhausts the JavaScript
stack. The recoverable path is unaffected: returning `Err(duck_error("..."))` (a `DuckOptionResult`)
does not unwind at all, so it surfaces as a clean, readable `Invalid Input Error` on both native and wasm.

**What to do — never `panic!` to signal an error.** Return `Err(duck_error("..."))` /
`DuckOptionResult` for anything a caller can react to. Keep `panic!` only for "this is an internal bug
that must abort", and know that on WebAssembly it will look like a stack overflow to a reader, not a
message. Measured against a locally built `wasm_eh` extension on emscripten 3.1.71.

## See also

- [Project structure](./getting-started/project-structure.md) — the crate roots, `error[E0583]`, and the
  IDE flagging a separate wasm root.
- [FAQ](./faq.md) — the errors people hit while writing functions.
- [Build and release](./build-and-release.md) — what the pipeline does on a version tag.
- [Architecture](./internals/architecture.md) — how registration and dispatch actually work.
