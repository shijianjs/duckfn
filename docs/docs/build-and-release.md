---
title: Build and release
sidebar_position: 6
description: Local build and test commands, the WebAssembly target, and how DuckDB's official pipeline turns a version tag into published binaries.
---

# Build and release

## Local builds

The `Makefile` includes DuckDB's official `extension-ci-tools` makefiles, so the commands are the
same ones every DuckDB extension uses:

| Command | What it does |
| --- | --- |
| `make configure` | Creates the Python venv, records the target platform and the extension version. Needs Python 3 and network access; run it once. |
| `make debug` | Builds the `cdylib`, appends extension metadata, and copies the result to `build/debug/extension/<name>/<name>.duckdb_extension`. |
| `make release` | The same with optimisations. |
| `make test` | Runs the sqllogictest suite against the debug build. |
| `make clean` / `make clean_all` | Drop build artefacts; `clean_all` also drops `configure/`. |

The repository wraps the common combinations in its `Justfile`:

```bash
just build                  # cargo duckdb-ext build -- --features quack
just sql "SELECT double_it5(21);"          # build, then LOAD and run one statement
just test                   # make configure debug test
just doc                    # cargo doc -p duckfn
```

Four settings in the `Makefile` are worth knowing:

```make
EXTENSION_NAME=duckfn
USE_UNSTABLE_C_API=1
TARGET_DUCKDB_VERSION=v1.5.5
TARGET_INFO += --features quack
```

`USE_UNSTABLE_C_API=1` is what makes the built extension loadable only with `-unsigned`, and only in
a compatible DuckDB version. `TARGET_DUCKDB_VERSION` names the version the metadata is written for.
`EXTENSION_NAME` has to match `duckfn_entrypoint!` in `src/extension/entry.rs` and the `require` lines
of the sqllogictest files. `TARGET_INFO += --features quack` is what gets the example compiled: it
lives behind the `quack` feature (off by default), and `TARGET_INFO` is the one variable DuckDB's
shared makefiles splice verbatim into `cargo build`. Without it the build would hand back a cdylib
with no entry-point symbol.

## WebAssembly

The wasm build needs the Emscripten target once:

```bash
rustup target add wasm32-unknown-emscripten
just build_wasm
```

Because `emcc` performs the final link, that target needs a `staticlib` rather than a `cdylib`. Since
`crate-type` cannot be overridden per target, the lib simply lists both — `["rlib", "cdylib",
"staticlib"]` — and the wasm build picks the `.a` up. Both extension artefacts then come out of one
compilation of `src/extension/`, which matters: a second copy of the tree would register every function
twice, and the duplicate entry symbol fails to link on wasm. `make` does the rest — it links the archive
into a side module with `emcc` and appends the extension metadata. See
[Project structure](./getting-started/project-structure.md) for the alternative (a separate wasm root
target, as the upstream template uses) and what it costs.

## Continuous integration

`.github/workflows/MainDistributionPipeline.yml` delegates the heavy lifting to DuckDB's shared
workflow:

```yaml
on:
  push:
    tags: ['v*.*.*']
  workflow_dispatch:

jobs:
  duckdb-stable-build:
    uses: duckdb/extension-ci-tools/.github/workflows/_extension_distribution.yml@v1.5-variegata
    with:
      duckdb_version: v1.5.5
      ci_tools_version: v1.5-variegata
      extension_name: duckfn
      extra_toolchains: rust;python3
      exclude_archs: 'linux_amd64_musl'
```

That shared workflow builds and tests the extension for every supported platform, including the
WebAssembly targets. The repository only has to say which version of DuckDB and which toolchains to
use.

### Release

A second job turns the pushed tag into a GitHub Release:

1. Download every `duckfn-*-extension-*` artefact.
2. Collect the `*.duckdb_extension` and `*.duckdb_extension.wasm` files into
   `duckfn-<arch>.duckdb_extension`.
3. Build release notes from the commit list since the previous `v*` tag.
4. Create the release, or upload to it if it already exists.

So bumping the version is: tag `vX.Y.Z`, push the tag, and wait for both jobs.

## Publishing the crates

The two library crates are published in dependency order — `duckfn` depends on
`duckfn-macro = "={{DUCKFN_VERSION}}"`, so the macro crate has to exist on crates.io first:

```bash
just publish_dry   # cargo publish -p duckfn-macro --dry-run, then -p duckfn
just publish
```

Versions come from the workspace:

```toml
[workspace.package]
version = "{{DUCKFN_VERSION}}"
rust-version = "1.86"
```

The example extension ships as part of this package instead of as a crate of its own, so it is never
uploaded separately.

The published `duckfn` package is more than the runtime: the `include` list in the root `Cargo.toml`
packs the example extension (`src/extension/**`, `src/bin/duckfn.rs`), its
sqllogictest suite (`test/sql/**/*.test`), the documentation sources (`docs/README.md`,
`docs/docs/**` and the Simplified Chinese translations under `docs/i18n/`), `demo.sh`, the READMEs
and the license. Unpacking the crate therefore hands you both the full documentation and a runnable
example — which is the point of keeping them in one package. `cargo package -p duckfn --list` prints
the exact file list.

The example only compiles when the `quack` feature is on (`cargo build --features quack`), so its
presence leaves a dependency on `duckfn` untouched.

## Documentation site

The Docusaurus site in `docs/` is deployed by `.github/workflows/DeployDocs.yml` on every `v*.*.*`
tag — the same tags that start the extension build — and on demand from the Actions tab. Ordinary
commits do not build it. It reads the Pages URL from `actions/configure-pages`, builds both locales
with `npm run build`, and publishes the result.

The `github-pages` environment is protected, so the tag pattern `v*.*.*` has to be listed in
`Settings -> Environments -> github-pages -> Deployment branches and tags` for the deployment to be
accepted.

## Next

- [Contributing](./contributing.md) — the day-to-day workflow.
- [Quick start](./getting-started/quick-start.md) — a minimal build.
