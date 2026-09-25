/// 和wasm_lib同样的路径，
/// 解决官方方式mod路径不一致不能嵌套的问题：
/// error[E0583]: file not found for module demo --> src\lib.rs:3:1
/// error[E0583]: file not found for module types --> src\lib.rs:4:1
mod extension;