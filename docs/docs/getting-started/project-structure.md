---
title: Project structure
sidebar_position: 2
description: The one rule an extension project follows — every crate root declares the same extension module — and the two errors you get when you break it.
---

# Project structure

A project built with `duckfn` has three crate roots and exactly one rule: **every crate root
declares `mod extension;`, and nothing re-exports another root.** Get that right and nesting works
at any depth, the IDE stops complaining about the WebAssembly file, and the command-line tool sees
every function. Get it wrong and you get `error[E0583]` the first time a module gains a submodule.

```text
src/
├─ lib.rs              mod extension;                       native, crate-type = ["cdylib"]
├─ wasm_lib.rs         mod extension;                       wasm example, crate-type = ["staticlib"]
├─ bin/
│  └─ duckfn.rs        #[path = "../extension/mod.rs"]
│                      mod extension;                       CLI, cargo run --bin duckfn
└─ extension/
   ├─ mod.rs           mod demo; mod functions; mod types;
   │                   duckfn_entrypoint!("my_ext");
   ├─ demo/
   ├─ functions/
   └─ types/
```

All three roots point at `src/extension/mod.rs` — a *directory* module — so they all see the same
tree, and `duckfn_entrypoint!` sits in exactly one place.

## Keep the two entry points identical

`crate-type` cannot be chosen per target, and the two targets need different ones: `cdylib` when
compiling natively, `staticlib` for WebAssembly. So `Cargo.toml` carries a second crate root as an
extra example:

```toml
[[example]]
# crate-type can't be (at the moment) be overriden for specific targets
path = "src/wasm_lib.rs"
crate-type = ["staticlib"]
```

That file is **not** an alias for `src/lib.rs` — it declares `mod extension;` itself. The official
upstream template instead writes:

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

That is why `src/lib.rs` and `src/wasm_lib.rs` are three lines each, and why every module you add
goes under `src/extension/` rather than next to a crate root.

## The command-line tool is a third root

`src/bin/duckfn.rs` (see [community extension docs](../community-extension-docs.md)) is a crate
root of its own too, and it has to end up with the same tree in its binary. It cannot simply
depend on the library, for two reasons:

- **Paths.** A crate root under `src/bin/` resolves `mod extension;` to `src/bin/extension.rs`, not
  to `src/extension/`. `#[path]` points it back at the directory module the other two roots use.
- **Registration.** The documentation metadata behind `#[duck_*]` is collected by `inventory`'s
  static constructors, which only fire for object files that are really linked into the final
  binary. Depending on the library lets the linker drop those modules, and the exported CSV comes
  out empty — silently, with no error to chase.

So it declares the module itself:

```rust
#[path = "../extension/mod.rs"]
mod extension;
```

Adding a new crate root to the project means the same two lines, pointed at
`src/extension/mod.rs`. Nothing else changes.

## The IDE flags `src/wasm_lib.rs`

RustRover or rust-analyzer marks up `src/wasm_lib.rs` with errors that `make debug`, `just build`
or `cargo duckdb-ext build` never reproduce.

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
- [Contributing](../contributing.md) — the duckfn repository itself, whose layout deliberately
  differs: there the example extension sits inside the published package, so its library carries the
  native, wasm and `rlib` crate types itself instead of adding a separate wasm root.
- [Troubleshooting](../troubleshooting.md) — problems that are not about the layout.
