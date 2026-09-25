---
title: Contributing
sidebar_position: 7
description: Setting up the repository, the day-to-day commands, how the tests are organised, and the conventions to follow.
---

# Contributing

## Prerequisites

| | |
| --- | --- |
| Rust | 1.86 or newer — the workspace sets `rust-version = "1.86"` and uses edition 2024. |
| Python 3 + network | Only for `make configure`, which builds the sqllogictest runner's virtualenv. |
| `make` | Drives the DuckDB `extension-ci-tools` makefiles. |
| [`just`](https://github.com/casey/just) *(optional)* | The `Justfile` wraps the common commands. |
| [`cargo-duckdb-ext-tools`](https://github.com/redraiment/cargo-duckdb-ext-tools) *(optional)* | `cargo install cargo-duckdb-ext-tools` gives you `cargo duckdb-ext build`, which needs neither `make` nor a submodule checkout. |
| DuckDB CLI | For loading the extension by hand, and for debugging. |

`extension-ci-tools/` is a git submodule and the `Makefile` includes makefiles from it, so after a
fresh clone:

```bash
git submodule update --init --recursive
make configure
```

## Windows

Run `make` from **Git Bash**, not PowerShell or `cmd`: the makefiles and their helper scripts assume
a POSIX shell. Anything `make` reports as missing can usually be installed with
[Scoop](https://scoop.sh/):

```shell
scoop install make python
```

Cargo and `cargo duckdb-ext build` work in any shell, so Git Bash is only needed for the `make`
targets — `make configure`, `make test`, and the CI-equivalent commands.

## The workspace

| Member | Published | Notes |
| --- | --- | --- |
| `/` (`duckfn`) | yes | The runtime framework, the example extension and the workspace root. |
| `duckfn-macro/` | yes | The procedural macros; depends on the runtime for nothing, only on `darling`, `syn`, `quote`. |
| `src/extension/`, `test/sql/` | shipped, never compiled | The example extension (`duckfn`) with its sqllogictest suite: part of the `duckfn` package, switched on by the `quack` feature. |

The root manifest pins `duckfn-macro = "={{DUCKFN_VERSION}}"`, so the two crates always ship together.

One Cargo detail worth knowing: the example extension is **part of the `duckfn` package** — the module
tree in `src/extension/` plus the `src/bin/duckfn.rs` entry point — rather than
a crate of its own, because cargo never packages a subdirectory that contains its own `Cargo.toml`.
What compiles it is the `quack` feature, off by default: `make debug` passes it on through
`TARGET_INFO += --features quack` in the root `Makefile`. A dependency on `duckfn` therefore sees the
same dependency tree as before — sources present in the package, nothing compiled. Use
`cargo test -p duckfn` for the runtime's own tests, or `cargo test --workspace` for everything.

## Day-to-day commands

```bash
make debug                              # build the extension
just sql "SELECT double_it5(21);"        # rebuild and run one statement
make test                               # run the sqllogictest suite
just doc                                # build the rustdoc for duckfn
```

`make test` runs `make configure debug test` through `just test`.

`.cargo/config.toml` statically links the C runtime on `x86_64-pc-windows-msvc`; nothing else needs
to be configured per platform.

## Debugging

The extension code runs **inside the `duckdb` process**, so attach the debugger to that process
instead of launching something yourself:

1. Build with debug symbols — `make debug`, or `just build` (which runs
   `cargo duckdb-ext build -- --features quack`).
2. Start DuckDB and keep the session alive, for example `duckdb -unsigned`.
3. `LOAD '/path/to/my_ext.duckdb_extension';` in that session.
4. In the IDE, attach to the running `duckdb` process — in RustRover that is
   [Attach to process](https://www.jetbrains.com/help/rust/attach-to-process.html).
5. Set a breakpoint in your function and run the SQL that calls it, e.g. `SELECT double_it(21);`.

The shared library only enters the process at `LOAD`, so a breakpoint set earlier starts resolving
from that point on. `just sql "<SQL>"` is a quick way to run one statement by hand while the
debugger is attached.

## Tests

DuckDB's own sqllogictest runner is the practical choice: it exercises the extension through SQL,
exactly the way DuckDB calls it, and it is what CI runs. It needs the `make` flow to be set up
(`make configure` once, then `make test`).

Tests are sqllogictest files under `test/sql/`, mirroring the source layout:

```
test/sql/demo/       <source file>.test
test/sql/functions/  <source file>.test
test/sql/types/      <type>_scalar_echo.test, <type>_table_echo.test
```

A file starts by requiring the extension, then pairs SQL with its expected output:

```sql
require duckfn

query I
SELECT double_it5(21);
----
42

statement error
SELECT CAST('abc' AS INTEGER);
----
not an integer: "abc"
```

When you add a function, add the matching `.test` file: the expected values there are what the
documentation quotes, so they are the source of truth for behaviour. Type codes used in the files
include `I` (integer), `T` (text), `R` (real) and combinations such as `IT`; list, map, struct and
array values are compared as text after `CAST(… AS VARCHAR)`.

## Documentation

The site lives in `docs/`. Every English page under `docs/docs/` needs its Simplified Chinese
counterpart at the same path under
`docs/i18n/zh-Hans/docusaurus-plugin-content-docs/current/`:

- Translate the body and the reader-facing front matter (`title`, `description`).
- Keep `sidebar_position` identical so both sidebars stay in the same order.
- Link between pages with relative file paths (`./types.md`, `../guide/types.md`) so each language
  links to its own pages.

```bash
cd docs
npm start                # http://localhost:3000
npm start -- --locale zh-Hans
npm run build            # must pass for both locales; broken links fail the build
```

## Conventions

- Match the style of the surrounding code. The workspace is not `rustfmt`-clean today, so running
  `cargo fmt` across the tree would rewrite files unrelated to your change — format only what you
  touched. Keep `cargo clippy` quiet.
- Error messages start with the name of the function that produced them, e.g.
  `dfn_table_checked: n must be >= 0`.
- User-facing code stays free of `unsafe`; the only accepted exceptions are the explicit
  registration paths, which need `unsafe { c.register_scalar(…) }` and friends.
- New attribute arguments go into the argument struct of the macro that actually needs them
  (`duckfn-macro/src/<macro>.rs`); each macro declares only its own keys and no longer forwards its
  arguments to the derive macros.
- When behaviour changes, update the sqllogictest expectation first, then the docs page that quotes
  it, then the READMEs (`README.md` and `README.zh-CN.md`).

## Next

- [Architecture](./internals/architecture.md) — where the code you are about to change lives.
- [Build and release](./build-and-release.md) — the CI and release flow.
