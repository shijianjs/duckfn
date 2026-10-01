---
title: DuckDB version compatibility
sidebar_position: 8
description: Which DuckDB the headers come from, what the metadata's version field means under each ABI type, and how to keep one binary loadable across several DuckDB releases.
---

# DuckDB version compatibility

A duckfn extension never links DuckDB: it calls through the C API's function table, and DuckDB itself
decides at `LOAD` time whether the file is acceptable. Three different numbers take part in that
decision, and only one of them is the "DuckDB version" people usually mean. Which side you take on
each of them is what decides whether one build serves a range of DuckDB releases or exactly one.

## The three numbers

| Number | Where it is set | What it means |
| --- | --- | --- |
| The **release the headers come from** | `libduckdb-sys` in `Cargo.toml` (resolved in `Cargo.lock`) | Which `duckdb_extension.h` the build compiles against — the API surface your code may call. |
| The **value written into the metadata** | `TARGET_DUCKDB_VERSION` in the `Makefile`, handed to `append_extension_metadata.py` as `-dv` | Read as a **C API version** under `abi_type = C_STRUCT`, and as a **release** under `C_STRUCT_UNSTABLE`. |
| The **engine that loads the file** | the user's DuckDB | Its own C API level (or release) is what your declaration is checked against. |

The workflow's `duckdb_version` — and the `DUCKDB_VERSION` it also exports — is a fourth, unrelated
knob: it picks which DuckDB source the distribution pipeline checks out, which engine the
sqllogictest run uses, and what the artifacts are named. It never reaches your extension.

## `C_STRUCT` vs `C_STRUCT_UNSTABLE`

`USE_UNSTABLE_C_API` in the `Makefile` decides which of the two ABI types the metadata claims, and
*that* is what makes the `-dv` value mean two different things.

**`USE_UNSTABLE_C_API=0` → `abi_type = C_STRUCT`.** The declaration is a **floor**, and the loader
compares C API versions:

```text
The file was built for DuckDB C API version '<declared>', but we can only load extensions built for
DuckDB C API '<engine>' and lower.
```

An engine whose C API is at or above your floor loads the file; its exact release does not matter.
That is the portability you want, and it costs nothing as long as your code stays inside the stable
region of the C API.

**`USE_UNSTABLE_C_API=1` → `abi_type = C_STRUCT_UNSTABLE`.** Now the declaration is a **release** and
the engine demands a literal match. The unstable part of the extension API is a C struct addressed by
slot order, so a build compiled against a different set of slots cannot run: one file, one engine.

`append_extension_metadata.py`'s own help text spells the split out: *"The DuckDB version to encode,
depending on the ABI type this encodes the duckdb version or the C API version."*

## Where the values come from

The two settings live in different files. The pairing below is the **stable** one; duckfn's own
example extension sits on the other side (`USE_UNSTABLE_C_API=1`, `TARGET_DUCKDB_VERSION=v1.5.6`)
because its `quack` example really does use the unstable region.

```toml
# Cargo.toml — the headers
libduckdb-sys = { version = ">=1.10500, <2", features = ["loadable-extension"] }
```

```make
# Makefile — the declaration
USE_UNSTABLE_C_API=0
TARGET_DUCKDB_VERSION=v1.2.0
```

`libduckdb-sys` encodes a DuckDB release as `1.<major*10000 + minor*100 + patch>.0`, so `1.10506.0`
is DuckDB 1.5.6 and the `>=1.10500` above means "DuckDB 1.5 headers or newer".

Nothing else rewrites `TARGET_DUCKDB_VERSION`: ci-tools' `set_duckdb_version` is a no-op for C API
extensions, and the community registry's `duckdb_version` only chooses which DuckDB source to check
out, how the artifacts are named and which version directory they are deployed into. The line is
yours to maintain — see [Build and release](./development/build-and-release.md).

## Choosing for your own extension

One question decides it: **does your code touch the unstable region?**

- It holds copy functions, the host file system (`duckfn::duck_vfs`), the scalar `bind` / `init`
  slots, `varargs`, and the logical types DuckDB gained in 1.5 (today `TIME_NS`). duckfn groups all
  of it behind the `duckdb-1-5` feature; `owned-connection` (the host VFS) implies it.
- Everything else is in the stable region: scalars, aggregates, table functions, casts, replacement
  scans, SQL macros, named STRUCT / ENUM types, and the chrono / uuid / rust_decimal bridges.

| Your code | `USE_UNSTABLE_C_API` | `TARGET_DUCKDB_VERSION` | What you get |
| --- | --- | --- | --- |
| Stable region only | `0` | the C API version you need — today `v1.2.0` | one binary for every engine whose C API is at least that |
| Anything from the unstable region | `1` | the exact release, e.g. `v1.5.6` | one binary, on that engine only |

Three consequences worth knowing before you pick:

- **Do not turn `duckdb-1-5` on while claiming `C_STRUCT`.** The loader only checks what you declared,
  so the file would be accepted by engines whose unstable layout differs — and the mismatch would show
  up as undefined behaviour instead of a load error. Pick one side and match the feature to it.
- **With the unstable API off, the ABI guard never fires.** quack-rs decides that from the same switch
  (`abi::uses_unstable_api()` is `cfg!(feature = "duckdb-1-5")`) and returns `StableOnly` before it
  asks anything, so the `QUACK_RS_TARGET_DUCKDB_VERSION` export the unstable path needs is redundant
  there. Leaving it out while the unstable API *is* on is not — add it back when you flip the flag.
- **The two numbers move together when you flip the flag.** Going back to `USE_UNSTABLE_C_API=1` means
  `TARGET_DUCKDB_VERSION` becomes a release number (and has to equal the engine), not a C API floor.

### What a stable build actually covers

`v1.2.0` is the C API level DuckDB 1.3.2 through 1.5.5 all sit at, so one stable build declaring it is
accepted across the whole range. Measured with a single artifact, each engine loading it and running a
real call:

| Declared C API floor | Engines that took the same binary |
| --- | --- |
| `v1.2.0` | 1.3.2, 1.4.0, 1.4.5, 1.5.0, 1.5.5, 1.5.6 |

Raise the floor only when you genuinely need newer headers. And raise it by the *C API version*, not
by a release number: writing `v1.5.6` there leaves exactly that engine willing to take the file.

## Checking a build yourself

`make debug` prints the metadata it just wrote, which is the quickest way to see what you actually
declared rather than what you meant to:

```text
FIELD2 = windows_amd64
FIELD3 = v1.2.0          # the declared version
FIELD5 = C_STRUCT        # the ABI type
```

Then take that one artifact and load it into several engines — a locally built extension always needs
`-unsigned`:

```shell
duckdb -unsigned -c "LOAD '<path>/my_extension.duckdb_extension'; SELECT my_greet('world');"
```

Two traps come up here:

- **A pinned test runner is not the engine under test.** The `configure/venv` directory is a one-time
  stamp, so `make` never refreshes the Python `duckdb` inside it; a stale runner refuses a freshly
  built extension. Upgrade it in place after moving a pin.
- **The header pin and the engine are separate.** Updating `libduckdb-sys` changes what you may call,
  not what will load you — and vice versa.

## WebAssembly

The same rules apply, with one extra caveat: the engine is fixed inside the DuckDB-Wasm bundle, so
you cannot pick it at `LOAD` time. An unstable build therefore has to be served by the exact dev build
carrying the matching release, while a stable build only needs an engine at or above its floor. How
this site pins that build is in [Preloaded extensions](./docs-kit/preloaded-extensions.md).

## Release assets vs the community registry

`LOAD '<url of a release asset>'` fetches exactly the file you name, so "one binary across releases"
is entirely your declaration's doing. The community registry works differently: it hands out the
build for the DuckDB version the *client* reports, from a directory tree the registry fills in itself
— see [Community extension docs](./community-extension-docs.md). That makes the registry route
version-directed rather than version-tolerant, whatever you declared.

## See also

- [Installation](./getting-started/installation.md) — what each duckfn feature requires of DuckDB.
- [File system](./guide/file-system.md) and [Copy functions](./guide/copy-functions.md) — the two
  capabilities that live in the unstable region.
- [Build and release](./development/build-and-release.md) — the build paths, the release flow and the
  way the CI knobs relate to these ones.
- [Known issues](./known-issues.md) — upstream bugs that are not about versions.