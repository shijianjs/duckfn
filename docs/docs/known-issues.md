---
title: Known issues
sidebar_position: 10
description: Upstream DuckDB bugs that surface in extension builds — today, all-NULL list literals that read back as garbage through the C API.
---

# Known issues

Upstream bugs and platform quirks that bite extension builds, each with a workaround. They are largely
not duckfn's doing. The two **version-related** issues — the old Rust 1.86 pin on the official CI's wasm
build, and why a `panic!` behaves differently on WebAssembly depending on the Rust version — now live
under [DuckDB version compatibility](./duckdb-versions/index.md): see
[The wasm build toolchain](./duckdb-versions/wasm-toolchain.md) and
[Rust unwinding on WebAssembly](./duckdb-versions/rust-wasm-unwinding.md). For usage questions — why a
function does not show up, or why `-unsigned` is needed — see [FAQ](./faq.md).

Problems with the *layout* — the crate roots, `error[E0583]`, and the IDE flagging a separate wasm
root — live in [Project structure](./getting-started/project-structure.md) instead, because they are
about how the project is put together rather than about something going wrong.

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

## See also

- [The wasm build toolchain](./duckdb-versions/wasm-toolchain.md) and
  [Rust unwinding on WebAssembly](./duckdb-versions/rust-wasm-unwinding.md) — the version issues, now
  under DuckDB version compatibility.
- [FAQ](./faq.md) — the errors people hit while writing functions.
- [Build and release](./development/build-and-release.md) — what the pipeline does on a version tag.
- [Architecture](./development/architecture.md) — how registration and dispatch actually work.
