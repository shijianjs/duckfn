//! 扩展产物本体：原生扩展的 `cdylib`（`duckfn.dll` / `libduckfn.so`）与 WebAssembly 用的
//! `staticlib`（`libduckfn.a`）都由这个 target 产出（见根 `Cargo.toml` 的 `[[example]] duckfn`）。
//!
//! 它与下游模板（duckfn-extension-template）的 `[[example]]` 是同一个路子：示例树只在这里编一遍，
//! 入口符号与 `inventory` 的注册项都只有一份 —— 多编一遍会重复注册，wasm 上直接链接失败。
//! CLI（`src/bin/duckfn.rs`）另编一份只为收集注册项，那个是 executable，不会跟这里的符号撞。
//!
//! The extension artefact itself: this target produces the native extension's `cdylib`
//! (`duckfn.dll` / `libduckfn.so`) and the WebAssembly `staticlib` (`libduckfn.a`) — see
//! `[[example]] duckfn` in the root `Cargo.toml`.
//!
//! It follows the same route as the downstream template's `[[example]]`: the example tree is compiled
//! once, here, so the entry symbol and the `inventory` registrations exist exactly once — a second
//! copy would register everything twice and fail to link on wasm. The CLI (`src/bin/duckfn.rs`)
//! compiles another copy only to collect the registrations; that one is an executable and its symbols
//! do not clash with this one's.

#[path = "../test/extension/mod.rs"]
mod extension;

#[path = "../test/extension/entry.rs"]
mod extension_entry;
