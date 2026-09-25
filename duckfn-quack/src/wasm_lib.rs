#![cfg(target_arch = "wasm32")]
#![allow(special_module_name)]

/// 和wasm_lib同样的路径，
/// 解决官方方式mod路径不一致不能嵌套的问题：
/// error[E0583]: file not found for module demo --> src\lib.rs:3:1
/// error[E0583]: file not found for module types --> src\lib.rs:4:1
mod extension;

// To build the Wasm target, a `staticlib` crate-type is required
//
// This is different than the default needed in native, and there is
// currently no way to select crate-type depending on target.
//
// This file sole purpose is remapping the content of lib as an
// example, do not change the content of the file.
//
// To build the Wasm target explicitly, use:
//   cargo build --example $PACKAGE_NAME
