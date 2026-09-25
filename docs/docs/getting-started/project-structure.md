---
title: Project structure
sidebar_position: 2
description: The one rule this repository's layout follows — every crate root declares the same extension module — and the two errors you get when you break it.
---

# Project structure

This repository has two crate roots and exactly one rule: **every crate root includes `src/extension/` —
a *directory* module — and nothing re-exports another root.** Get that right and nesting works at any
depth, and the command-line tool sees every function. Get it wrong and you get `error[E0583]` the first
time a module gains a submodule.

```text
src/
├─ lib.rs              #[cfg(feature = "quack")] mod extension;        the runtime and the extension:
│                      #[cfg(feature = "quack")] mod extension_entry;  crate-type = ["rlib", "cdylib", "staticlib"]
├─ bin/
│  └─ duckfn.rs        #[path = "../extension/mod.rs"] mod extension;  CLI (bin target `duckfn-cli`)
└─ extension/
   ├─ mod.rs           mod demo; mod functions; mod types;
   ├─ entry.rs         duckfn_entrypoint!("duckfn");
   ├─ demo/
   ├─ functions/
   └─ types/
```

Both roots point at `src/extension/mod.rs` — a *directory* module — so they see the same tree. The
entry point lives one file over, in `src/extension/entry.rs`, because it may only be defined once: the
lib carries it, while the CLI links that lib and therefore includes `extension/mod.rs` only.

## One lib, three crate types

`crate-type` cannot be overridden per target, so the lib lists every type it is needed as and each
platform picks its own: `rlib` for dependents, `cdylib` for the native extension DuckDB loads, and
`staticlib` for WebAssembly, where `emcc` does the final link and wants a `.a`:

```toml
[lib]
crate-type = ["rlib", "cdylib", "staticlib"]
```

Both extension artefacts therefore come out of one compilation of `src/extension/`, which matters: a
second copy of the tree would register every function twice, and on wasm the duplicate entry symbol
fails to link outright.

The upstream template takes the other road, and it is the one to take whenever a project needs a
separate root for the wasm target: a `[[example]]` whose own root declares `mod extension;`.

```toml
[[example]]
# crate-type can't be (at the moment) be overriden for specific targets
path = "src/wasm_lib.rs"
crate-type = ["staticlib"]
```

That keeps the roots side by side and lets the wasm build avoid linking the native `cdylib` under
emscripten at all — but only as long as that example does not depend on the package's own lib.

Such a root is **not** an alias for `src/lib.rs`; the official upstream template instead writes:

```rust
// src/wasm_lib.rs, the official template
mod lib;
```

…and that is where `error[E0583]` comes from. `mod lib;` resolves to `src/lib.rs`, and from that
point on `lib.rs` is a **file** module: its children are looked up *beside* it, under `src/lib/`.
A `mod demo;` written inside `src/lib.rs` is therefore searched for at `src/lib/demo.rs` instead of
`src/extension/demo.rs`, and rustc reports:

```
error[E0583]: file not found for module `demo`
 --> src\lib.rs:3:1

error[E0583]: file not found for module `types`
 --> src\lib.rs:4:1
```

A flat `lib.rs` hides the problem; nesting exposes it. Declaring the same path in both roots
avoids it entirely, because `mod demo;` inside `src/extension/mod.rs` is looked up at
`src/extension/demo.rs` or `src/extension/demo/mod.rs`.

That is why a wasm root like that one is three lines long, and why every module you add goes under
`src/extension/` rather than next to a crate root.

## The command-line tool is a second root

`src/bin/duckfn.rs` (see
[community extension docs](../community-extension-docs.md)) is a crate
root of its own too, and it has to end up with the same tree in its binary. It cannot simply
depend on the library, for two reasons:

- **Paths.** A crate root under `src/bin/` resolves `mod extension;` to `src/bin/extension.rs`, not
  to `src/extension/`. `#[path]` points it back at the directory module the other two
  roots use.
- **Registration.** The documentation metadata behind `#[duck_*]` is collected by `inventory`'s
  static constructors, which only fire for object files that are really linked into the final
  binary. Depending on the library lets the linker drop those modules, and the exported CSV comes
  out empty — silently, with no error to chase.

So it declares the module itself:

```rust
#[path = "../extension/mod.rs"]
mod extension;
```

It deliberately skips `extension/entry.rs`: the lib it links already defines the entry symbol, and a
second definition is a duplicate-symbol error (`LNK2005` on Windows). Its Cargo target is called
`duckfn-cli` rather than `duckfn` like its file, because this package's cdylib produces a `duckfn`
artefact too and the two Windows `.pdb` files would collide. A downstream project has no such clash
and keeps the plain `duckfn` name.

Adding a new crate root to the project means the same two lines, pointed at
`src/extension/mod.rs`. Nothing else changes.

## The IDE flags a separate wasm root

If your project uses a wasm example like the one above, RustRover or rust-analyzer marks that file up
with errors that `make debug`, `just build` or `cargo duckdb-ext build` never reproduce.

An IDE checks *every* target by default (`cargo check --all-targets`), which compiles that example
for your host platform as well — a configuration it was never written for. Gate the file on the
target architecture; on any other target it compiles to nothing:

```rust
#![cfg(target_arch = "wasm32")]
#![allow(special_module_name)]

mod extension;
```

The second attribute silences the lint about a crate root that is not named `lib.rs`.

## See also

- [Create a project](./create-a-project.md) — the template this layout comes from.
- [Build and release](../build-and-release.md#webassembly) — how the WebAssembly target is built.
- [Community extension docs](../community-extension-docs.md) — what `src/bin/duckfn.rs` is for.
- [Troubleshooting](../troubleshooting.md) — problems that are not about the layout.
