---
title: Known issues
sidebar_position: 10
description: The (resolved) Rust 1.86 pin in the official CI's WebAssembly build, the upstream bug that corrupts all-NULL list literals, and why a `panic!` is unusable on WebAssembly.
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
vendored workflow, until that ref is moved forward.

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

**Why — it is Rust's build, not Emscripten's.** Two independent layers have to line up, and duckfn
only controls one of them:

- **DuckDB-Wasm's `eh` bundle** does enable WebAssembly-level exception handling, so a DuckDB/C++
  throw is catchable. That layer is fine.
- **The Rust side** needs *Rust* unwinding, and that is not built for this target. rustup ships a
  precompiled `std` for `wasm32-unknown-emscripten` compiled with `panic = "abort"` — it carries no
  `libpanic_unwind`. So even though duckfn and quack-rs wrap every callback in
  `std::panic::catch_unwind` and build with `panic = "unwind"`, there is no unwinder to run: the
  `panic!` aborts, and the wasm runtime surfaces that as `RangeError: Maximum call stack size
  exceeded` instead of your message. On native, real unwinding exists — that is precisely why
  `catch_unwind` there turns the panic into a readable DuckDB error.

Making `catch_unwind` genuinely work on wasm means rebuilding `std` with unwinding, which today
requires nightly: `RUSTFLAGS="-Cpanic=unwind" cargo +nightly build -Zbuild-std=std,panic_unwind …`
(`-Zbuild-std` is still nightly-only). The official `wasm_eh` CI job does **not** do this — it builds
the staticlib with stable `cargo build --target wasm32-unknown-emscripten` and only runs `emcc` at the
end, and `emcc -fwasm-exceptions` cannot revive a `panic!` that Rust already compiled to `abort`. That
is the whole reason the guard is real on native and a no-op on wasm.

The recoverable path is unaffected: returning `Err(duck_error("..."))` (a `DuckOptionResult`) does not
unwind at all, so it is a clean, readable `Invalid Input Error` on both native and wasm.

**What to do — never `panic!` to signal an error.** Return `Err(duck_error("..."))` /
`DuckOptionResult` for anything a caller can react to. Keep `panic!` only for "this is an internal bug
that must abort", and know that on WebAssembly it will look like a stack overflow to a reader, not a
message. Measured against a locally built `wasm_eh` extension on emscripten 3.1.71.

## See also

- [Project structure](./getting-started/project-structure.md) — the crate roots, `error[E0583]`, and the
  IDE flagging a separate wasm root.
- [FAQ](./faq.md) — the errors people hit while writing functions.
- [Build and release](./development/build-and-release.md) — what the pipeline does on a version tag.
- [Architecture](./development/architecture.md) — how registration and dispatch actually work.
