---
title: Known issues
sidebar_position: 10
description: The (resolved) Rust 1.86 pin in the official CI's WebAssembly build, the upstream bug that corrupts all-NULL list literals, and how a `panic!` on WebAssembly depends on the Rust version.
---

# Known issues

Upstream bugs and platform quirks that bite extension builds, each with a workaround. They are
largely not duckfn's doing, and the first one is already fixed upstream and kept for context. For
usage questions — why a function does not show up, why `-unsigned` is needed — see
[FAQ](./faq.md).

Problems with the *layout* — the crate roots, `error[E0583]`, and the IDE flagging a separate wasm
root — live in [Project structure](./getting-started/project-structure.md) instead,
because they are about how the project is put together rather than about something going wrong.

## WASM builds on the official CI are no longer pinned to Rust 1.86 (resolved)

**Was.** The reusable distribution workflow hard-coded the WebAssembly job to Rust 1.86
(`dtolnay/rust-toolchain@1.86.0`) while every native job used stable, and the pin lived *inside* the
workflow, so an extension repository could not raise it without forking. Anything in the dependency
graph needing a language feature newer than 1.86 — the `let`-chains in `ar_archive_writer 0.5.x` were
the reported case — failed the `wasm_*` job alone while `linux_amd64`, `osx_*` and `windows_*` passed.

**Now.** Upstream fixed it: the wasm job's Rust toolchain was raised to **1.97.1**
([`duckdb/extension-ci-tools#394`](https://github.com/duckdb/extension-ci-tools/pull/394), closing
[`#385`](https://github.com/duckdb/extension-ci-tools/issues/385)). You no longer need to keep your
dependency graph buildable by 1.86, and you do not need to `exclude_archs` the wasm variants to dodge it.

**What is still worth checking.** The bump lives in an `extension-ci-tools` revision at/after #394, so
the ref your repository calls has to be new enough: bump `ci_tools_version` in
`.github/workflows/MainDistributionPipeline.yml` (and the vendored `extension-ci-tools` submodule)
accordingly. A ref that predates the fix keeps the old behaviour — for instance the `@v1.5-variegata`
this repository currently pins still carries `dtolnay/rust-toolchain@1.86.0` for the wasm job in the
vendored workflow, until that ref is moved forward. A repository-side way to bypass that without waiting
upstream: drop a root `rust-toolchain.toml` — it outranks the rustup *default* dtolnay sets, and travels
with the registry's `override_ref` too. See the wasm toolchain section of
[version compatibility](./duckdb-versions.md).

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

## A `panic!` inside a function on WebAssembly (depends on the Rust version)

**What you see — it is toolchain-dependent.** On the native CLI (and in `just test`) a panicking
function reports a readable message. The *same* call in the browser (DuckDB-Wasm) behaved differently
by stable `rustc`. Measured with two probe functions — one that just `panic!`s, one that wraps a
`panic!` in its own `catch_unwind` — on the pinned host `duckdb-wasm 1.33.1-dev65.0` / emsdk 3.1.71,
changing only the toolchain:

| stable rustc | a bare `panic!` | `catch_unwind` |
| --- | --- | --- |
| **1.89** | `RangeError: Maximum call stack size exceeded` (aborts) | **no** — nothing to unwind |
| **1.97.1** | a readable `Invalid Input Error: <message>` | **yes** — caught |

**Why.** DuckDB-Wasm's `eh` bundle already turns on WebAssembly-level exception handling (the C++ layer
is fine); the missing piece was *Rust's own* unwinding. rustup's precompiled `std` for
`wasm32-unknown-emscripten` used to ship `panic = "abort"` with no `libpanic_unwind`, so duckfn and
quack-rs's `catch_unwind` had no unwinder to run — the `panic!` aborted, and the wasm runtime surfaced
that as the stack-overflow `RangeError`. A **recent stable** now builds that target's `std` **with**
unwinding, so the guard works in the browser too. `emcc -fwasm-exceptions` is not the lever (it cannot
revive a panic already compiled to `abort`), and hand-rebuilding `std` with
`-Zbuild-std=std,panic_unwind` on nightly is no longer needed on current stable.

The recoverable path is still the right one, and toolchain-independent: return
`Err(duck_error("..."))` / `DuckOptionResult` for anything a caller can react to — it never unwinds and
reads as a clean `Invalid Input Error` on both native and wasm. Keep `panic!` for genuine "must abort"
internal bugs. Measured on a locally built `wasm_eh` extension (emsdk 3.1.71) by swapping only `rustc`.

## See also

- [Project structure](./getting-started/project-structure.md) — the crate roots, `error[E0583]`, and the
  IDE flagging a separate wasm root.
- [FAQ](./faq.md) — the errors people hit while writing functions.
- [Build and release](./development/build-and-release.md) — what the pipeline does on a version tag.
- [Architecture](./development/architecture.md) — how registration and dispatch actually work.
