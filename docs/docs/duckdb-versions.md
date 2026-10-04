---
title: DuckDB version compatibility
sidebar_position: 8
description: Which DuckDB the headers come from, what the metadata's version field means under each ABI type, how to keep one binary loadable across several DuckDB releases, and why building the wasm artifacts is a separate version coupling between Rust, emsdk/binaryen and the ci-tools pin.
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

Two files carry the settings — the headers come from `Cargo.toml`, the declaration from the
`Makefile`:

```toml
# Cargo.toml — the headers
libduckdb-sys = { version = ">=1.10500, <2", features = ["loadable-extension"] }
```

`libduckdb-sys` encodes a DuckDB release as `1.<major*10000 + minor*100 + patch>.0`, so `1.10506.0`
is DuckDB 1.5.6 and the `>=1.10500` above means "DuckDB 1.5 headers or newer".

Nothing else rewrites the `Makefile` line: ci-tools' `set_duckdb_version` is a no-op for C API
extensions, and the community registry's `duckdb_version` only chooses which DuckDB source to check
out, how the artifacts are named and which version directory they are deployed into. It is yours to
maintain — see [Build and release](./development/build-and-release.md). The next section gives both
complete sets.

## Choosing for your own extension

One question decides it: **does your code touch the unstable region?**

- It holds copy functions, the host file system (`duckfn::duck_vfs`), the scalar `bind` / `init`
  slots, `varargs`, and the logical types DuckDB gained in 1.5 (today `TIME_NS`). duckfn groups all
  of it behind the `duckdb-1-5` feature; `owned-connection` (the host VFS) implies it.
- Everything else is in the stable region: scalars, aggregates, table functions, casts, replacement
  scans, SQL macros, named STRUCT / ENUM types, and the chrono / uuid / rust_decimal bridges.

### Stable region — the complete set

```make
# Makefile
EXTENSION_NAME=my_extension
USE_UNSTABLE_C_API=0
TARGET_DUCKDB_VERSION=v1.2.0
```

```toml
# Cargo.toml — the ABI point is that `duckdb-1-5` stays off (`cli` is only for the
# `function_descriptions` bin and has nothing to do with the ABI).
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["cli"] }
```

```yaml
# .github/workflows/MainDistributionPipeline.yml
      duckdb_version: v1.5.6     # any engine at or above the floor; the newest is the safe pick
```

- `TARGET_DUCKDB_VERSION` is a **C API floor**, not a release — that is what the mode buys.
  `v1.2.0` is where DuckDB 1.3.2 through 1.5.5 all sit, so one artifact covers them (measured, below).
- **`export QUACK_RS_TARGET_DUCKDB_VERSION` is deliberately absent here, and adding it would not
  help.** quack-rs reads that variable only in the unstable path: `abi::check()` returns `StableOnly`
  before it ever calls `built_against_version()`, whenever `uses_unstable_api()` — i.e.
  `cfg!(feature = "duckdb-1-5")` — is false.
- `DUCKDB_TEST_VERSION` is absent for the same reason: the test runner may be any newer engine,
  because every engine at or above the floor accepts the artifact.

### Unstable region — the complete set

```make
# Makefile
EXTENSION_NAME=my_extension
USE_UNSTABLE_C_API=1
TARGET_DUCKDB_VERSION=v1.5.6
export QUACK_RS_TARGET_DUCKDB_VERSION=$(TARGET_DUCKDB_VERSION)
DUCKDB_TEST_VERSION := $(patsubst v%,%,$(TARGET_DUCKDB_VERSION))
```

```toml
# Cargo.toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["duckdb-1-5"] }   # + "owned-connection" for duckfn::duck_vfs
```

```yaml
# .github/workflows/MainDistributionPipeline.yml
      duckdb_version: v1.5.6     # must equal TARGET_DUCKDB_VERSION; it is the engine `make test` loads into
```

- **The `export` keyword is part of the line, not decoration.** quack-rs' build script reads
  `QUACK_RS_TARGET_DUCKDB_VERSION` from the *environment*, and `make` does not put a variable there
  unless it is exported. Drop `export` and cargo sees nothing — no error, just an empty value, and
  the layout check then falls back to quack-rs' own table, which is exactly what rejects the engine
  when it is a release quack-rs has not catalogued yet.
- `TARGET_DUCKDB_VERSION` is a **release** in this mode and the engine has to match it literally —
  which is why the test-runner pin (`DUCKDB_TEST_VERSION`) and the CI pin move together with it.
- Going back to the stable side means dropping all three version-flavoured lines at once: the
  `export`, the `DUCKDB_TEST_VERSION` derivation, and the CI pin's "must equal" requirement.

The one trap that survives in either mode:

- **Do not turn `duckdb-1-5` on while claiming `C_STRUCT`.** The loader only checks what you declared,
  so the file would be accepted by engines whose unstable layout differs — and the mismatch would show
  up as undefined behaviour instead of a load error. Pick one side and match the feature to it.

### What a stable build actually covers

`v1.2.0` is the C API level DuckDB 1.3.2 through 1.5.5 all sit at, so one stable build declaring it is
accepted across the whole range. Measured with a single artifact, each engine loading it and running a
real call:

| Declared C API floor | Engines that took the same binary |
| --- | --- |
| `v1.2.0` | 1.3.2, 1.4.0, 1.4.5, 1.5.0, 1.5.5, 1.5.6, 2.0.0 (pre-release) |

DuckDB 2.0 is close (see its release calendar), and a stable build declaring `v1.2.0` already loads and
runs there — verified by loading such an extension into a 2.0 pre-release and calling its functions (an
aggregate and a scalar both returned the expected values). The floor is what buys that portability: do
not read "runs on 2.0" as "was compiled against 2.0".

One 2.0 naming trap: DuckDB ships a **second, v2 C API** (`duckdb_v2.h`) — a genuinely different,
still-rapid surface that the experimental `duckdb-neo` wrapper targets. It does **not** replace the v1 API
for extensions: a loadable extension builds on the v1 C API (`libduckdb-sys`'s default `capi-v1` feature,
required by `loadable-extension`), and that v1 surface is exactly what 2.0 still loads. So "C API v2 is
different" is about the *new header*, not about v1 extensions losing compatibility. Mind the version
encoding too: DuckDB 2.0.0 maps to a `libduckdb-sys` crate version `1.20000.x` (format
`1.<major*10000 + minor*100 + patch>.x`) — still a `1.x`, so a `>=1.10500, <2` constraint selects it. The
`<2` bound is not the blocker; moving your release line onto 2.0 is gated by it being *released* (a
stable `v2.0-cyanoptera` ci-tools ref and real `duckdb-shared-libs` assets), not by a crate version bound.

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

- **A pinned test runner is not the engine under test.** `make configure` builds `configure/venv`
  once and never refreshes the Python `duckdb` inside it (the recipe hangs off the directory, so a
  second `make` skips it), and a stale runner refuses a freshly built extension. Upgrade it in place —
  these are the two paths `base.Makefile` itself uses:

```shell
configure/venv/Scripts/python.exe -m pip install --upgrade "duckdb==1.5.6"   # Windows
configure/venv/bin/python3       -m pip install --upgrade "duckdb==1.5.6"   # Linux / macOS
```

- **The header pin and the engine are separate.** Updating `libduckdb-sys` changes what you may call,
  not what will load you — and vice versa.

## WebAssembly

The same rules apply, with one extra caveat: the engine is fixed inside the DuckDB-Wasm bundle, so
you cannot pick it at `LOAD` time. An unstable build therefore has to be served by the exact dev build
carrying the matching release, while a stable build only needs an engine at or above its floor. How
this site pins that build is in [Preloaded extensions](./docs-kit/preloaded-extensions.md).

How the `wasm_*` artifacts are *built* is a different version story — the Rust the CI installs, the
emsdk/binaryen it pins, and the ci-tools ref they come from — covered in the next section.

## The wasm build toolchain: Rust ↔ emsdk ↔ ci-tools

Everything above is about which **DuckDB** a binary targets. Building the `wasm_*` artifacts is a
separate version problem with three knobs that have to line up, and — the trap — **each one is set in
a different repository**:

| Knob | Who sets it | What it controls |
| --- | --- | --- |
| The **Rust toolchain** the wasm build installs | the ci-tools ref you call (`uses: .../_extension_distribution.yml@<ref>` and the `ci_tools_version` input) | whether `cargo build` will even accept your dependency tree |
| The **emsdk / binaryen** (`3.1.71` → `wasm-opt` v120) | ci-tools' `setup-emsdk` step | which wasm features `emcc`'s post-link optimizer understands |
| The **link flags** (`-O3` vs `-O0`) | **your `Makefile`** (`link_wasm_release` / `link_wasm_debug`) | whether that optimizer runs at all |

The first two are pinned by the official pipeline. The emsdk pin is a **compatibility window, not an
exact-match rule.** Measured against the pinned host `duckdb-wasm 1.33.1-dev65.0` (built with ~3.1.71):
extensions linked with emsdk **3.1.74**, **4.0.23** and **5.0.7** all load and run fine (554 docs
examples, 0 unexpected), but ones linked with **6.0.0** and **6.0.10** build yet fail at `LOAD` with
`Could not load dynamic lib`. The cut-off is a clean emscripten **5 → 6** break: **5.0.7** (the last 5.x)
loads, **6.0.0** (the first 6.x) does not. So a nearby or moderately-newer emsdk is tolerated; one across
that boundary, whose side-module emscripten runtime imports no longer match the host's, is rejected. Pin to the CI's emsdk as the known-good choice and
do not assume any newer emsdk will load.

### Two independent failure modes

**1 · Compile stage (`cargo build`) — your dependency MSRV outruns the pinned Rust.**
The reusable workflow installs a specific Rust for the wasm job. Historically that lagged the native
job: the native build used 1.97.1 while the wasm "Build Wasm module" step still pinned 1.86.0. Any
tree whose dependencies require a newer rustc fails at `cargo build` —

```text
error: rustc 1.86.0 is not supported by the following packages:
  <crate> requires rustc 1.89
```

— before it ever reaches `emcc`. This is ci-tools [issue #385](https://github.com/duckdb/extension-ci-tools/issues/385),
fixed by [PR #394](https://github.com/duckdb/extension-ci-tools/pull/394), which raises the wasm Rust to
1.97.1 and unifies it with the native job. Two levers sit at different layers: bump `ci_tools_version` /
`@ref` to a ref that carries the fix (that only reaches *your own* release pipeline), **or** keep the pinned
ci-tools and override the installed Rust from your *repository* with a root `rust-toolchain.toml`
(`channel = "1.89.0"`, plus the wasm target under `targets = [...]`). The `dtolnay/rust-toolchain` action only
sets a rustup *default*, which a `rust-toolchain.toml` outranks — so `cargo` picks up the newer Rust, and
because the file travels with the checkout it also fixes the community-registry build (see below).

**2 · Link stage (`wasm-opt`) — modern Rust emits features old binaryen cannot parse.**
Recent Rust (`>=1.89`) compiling for `wasm32-unknown-emscripten` produces a module that declares
post-MVP target-features — `exception-handling`, `bulk-memory-opt`, and, once the module is big enough
(many distinct function types → an overlong indirect-call table), `call-indirect-overlong`. `emcc` reads
those declarations and forwards them to `wasm-opt` as `--enable-*`. emsdk 3.1.71 ships binaryen v120,
which does not recognise the newer names, so `emcc -O3` dies:

```text
Unknown option '--enable-bulk-memory-opt'
emcc: error: '...wasm-opt ... --enable-call-indirect-overlong ...' failed (returned 1)
```

`wasm-opt` is only an **optimizer**; the `wasm-ld` module is valid on its own and loads fine in
DuckDB-Wasm (the browser runtime supports the proposals, binaryen v120 merely cannot rewrite them). So
the escape with full functionality kept is to **skip the optimizer**: override the link to `-O0` in
your `Makefile`, placed *after* the `include` so it wins over `base.Makefile` — and under the same
`ifneq ($(DUCKDB_WASM_PLATFORM),)` guard base uses, or you also redefine the (empty) native target and
linux/macos/windows builds fail with `emcc: command not found`:

```make
# Makefile, after include extension-ci-tools/.../base.Makefile
ifneq ($(DUCKDB_WASM_PLATFORM),)
link_wasm_release:
	emcc $(EXTENSION_BUILD_PATH)/release/$(EXTENSION_LIB_FILENAME) -o $(EXTENSION_BUILD_PATH)/release/$(EXTENSION_FILENAME_NO_METADATA) -O0 -sSIDE_MODULE=2 -sEXPORTED_FUNCTIONS="_$(EXTENSION_NAME)_init_c_api"
endif
```

Deleting this override (going back to `-O3`) is correct once the pinned emsdk's binaryen understands the
features — i.e. when DuckDB bumps the wasm emsdk.

### CI does not read your `extension-ci-tools` submodule

The submodule is only the source of the makefiles for **local** `just build` / `make` / `just
test_wasm`. GitHub CI ignores your submodule's pinned commit: it fetches the reusable workflow at
`uses: ...@<ref>` and re-checks-out `extension-ci-tools` into the same path at `ci_tools_version`. So
the wasm toolchain you exercise locally can differ from the one CI (and the registry) use — keep the
local submodule ref, the `@ref`, and `ci_tools_version` aligned, or "green locally" will not mean
"green in CI".

### The community registry build uses its own pins

`duckdb/community-extensions/.github/workflows/build.yml` calls `_extension_distribution.yml` at **its
own** ci-tools ref / `ci_tools_version` default, passing only `override_repository` + `override_ref`
(your repo and commit SHA). Your project's `ci_tools_version` therefore reaches neither the registry nor
its Rust choice — so if the registry's pinned Rust is below your dependency MSRV, its wasm job fails the
same way. Bumping `ci_tools_version` cannot help, but a repository `rust-toolchain.toml` (above) **does**
reach the registry, because it travels with `override_ref`. The project-side levers are therefore: pin the
Rust with `rust-toolchain.toml`, or exclude wasm in `description.yml`:

```yaml
extension:
  excluded_platforms: "wasm_mvp;wasm_eh;wasm_threads"
```

Drop that line once the registry moves past #394. See
[Community extension docs](./community-extension-docs.md).

### Diagnosing it

- Compare the two `.a` files byte-for-byte for the feature strings (`bulk-memory-opt`,
  `call-indirect-overlong`, `exception-handling`) to see which artifact asks for newer binaryen.
- Confirm the Rust actually in play per directory: `rustc --version`, `rustup override list`.
- In CI: `gh run view <id> --json jobs` for per-platform conclusions, `gh api .../actions/jobs/<job>`
  for the failing **step name** (compile vs link), `gh run view --job=<id> --log` once the run finishes.

Concrete case: `duckfn_statrs` wraps `statrs 0.19` / `nalgebra 0.35`, which need rustc ≥ 1.89. With the
`v1.5-variegata` pin the wasm job failed at #385's 1.86. The adopted fix stays on `v1.5-variegata` and
overrides Rust from the repository itself — a root `rust-toolchain.toml` (`channel = "1.89.0"` plus the wasm
target), which outranks the dtolnay action's rustup default — **plus** the `-O0` override above (old binaryen
→ skip the optimizer). Verified green via `workflow_dispatch`: native and all three wasm build, no Release
touched. (An earlier variant borrowed `ci_tools_version: main` to reach 1.97.1; the in-repo file is preferred
because it also propagates to the registry.) The wasm artifacts ship un-optimized (~1.5 MB).

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