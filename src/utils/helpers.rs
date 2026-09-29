//! 错误与 panic 处理辅助：结果别名、错误构造、panic 转查询错误。
//!
//! Error and panic helpers: result aliases, error construction and converting panics into
//! query errors.
//!
//! # panic 兜底仅在原生生效 / `catch_unwind` works only natively
//!
//! 这里的 `*_unwind` 包装（以及各适配层复用的它们）用 `std::panic::catch_unwind` 把用户函数体
//! 里的 `panic!` 兜住、转成查询错误 —— 这只在原生平台成立（crate 以 `panic = "unwind"` 构建）。
//! **在 wasm / 浏览器（`wasm32-unknown-emscripten`）上失效**：该目标无法在 wasm↔JS 边界展开，
//! `catch_unwind` 的 landing pad 接不到 panic，它会逃进 JS 胶水层、表现为
//! `RangeError: Maximum call stack size exceeded`（栈溢出），而不是一条可读的查询错误。
//! 因此**绝不要用 `panic!` 报错**：可恢复错误一律返回 `Err(duck_error(..))` / [`DuckOptionResult`]。
//! 详见文档站的 Troubleshooting。
//!
//! These `*_unwind` wrappers catch a user `panic!` via `std::panic::catch_unwind` and turn it into
//! a query error — but only on native (the crate builds with `panic = "unwind"`). **On wasm /
//! the browser (`wasm32-unknown-emscripten`) they are ineffective**: that target cannot unwind
//! across the wasm↔JS boundary, so the `catch_unwind` landing pad never sees the panic; it escapes
//! into the JS shim and surfaces as `RangeError: Maximum call stack size exceeded`, not a readable
//! query error. So **never use `panic!` to signal an error** — return `Err(duck_error(..))` /
//! [`DuckOptionResult`] instead. See the Troubleshooting page on the docs site.

use quack_rs::error::ExtensionError;
use quack_rs::prelude::ScalarFunctionInfo;
use std::panic::{UnwindSafe, catch_unwind};
use quack_rs::aggregate::AggregateFunctionInfo;

/// 本 crate 的结果类型别名：错误统一为 quack-rs 的 [`ExtensionError`]。
///
/// Result type alias of this crate: the error is always quack-rs' [`ExtensionError`].
pub type DuckResult<T> = Result<T, ExtensionError>;
/// 「可能为空的结果」类型别名，`Ok(None)` 表示 SQL NULL。
///
/// Alias for a fallible, nullable result; `Ok(None)` represents SQL NULL.
pub type DuckOptionResult<T> = DuckResult<Option<T>>;

/// 由任意字符串构造一个 quack-rs 错误。
///
/// Creates a quack-rs error from any string-like value.
pub fn duck_error(message: impl Into<String>) -> ExtensionError {
    ExtensionError::new(message)
}

/// 把 `catch_unwind` 捕获到的 panic payload 转成 [`ExtensionError`]。
///
/// Converts the panic payload caught by `catch_unwind` into an [`ExtensionError`].
pub fn panic_to_duck_error(e: Box<dyn std::any::Any + Send>) -> ExtensionError {
    duck_error(panic_to_string(e))
}

/// 把 `Vec<Option<T>>` 转成 `Vec<Option<&T>>`，用于按引用批量写入向量。
///
/// Converts `Vec<Option<T>>` into `Vec<Option<&T>>`, used to write a batch of values by
/// reference.
pub fn vec_option_to_ref<T>(vec: &[Option<T>]) -> Vec<Option<&T>> {
    vec.iter().map(Option::as_ref).collect()
}

/// 将panic转换成字符串
///
/// 支持 `&str` 与 `String` 两种 payload，其他情况返回 `"unknown panic"`。
///
/// Converts a panic payload into a string. Both `&str` and `String` payloads are supported;
/// anything else yields `"unknown panic"`.
pub fn panic_to_string(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// 处理scalar函数的panic：执行 `f`，panic 时转成查询错误写回 `info`。
///
/// 仅原生生效；wasm/浏览器上 `catch_unwind` 兜不住（见模块文档），panic 会变成栈溢出。
///
/// Handles panics in scalar functions: runs `f` and, on panic, reports the message as a query
/// error through `info`. Native only — on wasm the unwind cannot be caught (see the module docs).
pub fn duck_scalar_unwind<F: FnOnce() -> R + UnwindSafe, R>(info: &ScalarFunctionInfo, f: F) {
    let unwind = catch_unwind(f);
    if let Err(e) = unwind {
        info.set_error(&panic_to_string(e));
    }
}

/// 处理aggregate函数的panic：执行 `f`，panic 时转成查询错误写回 `info`。
///
/// 仅原生生效；wasm/浏览器上兜不住（见模块文档）。
///
/// Handles panics in aggregate functions: runs `f` and, on panic, reports the message as a
/// query error through `info`. Native only — ineffective on wasm (see the module docs).
pub fn duck_aggregate_unwind<F: FnOnce() -> R + UnwindSafe, R>(info: &AggregateFunctionInfo, f: F) {
    let unwind = catch_unwind(f);
    if let Err(e) = unwind {
        info.set_error(&panic_to_string(e));
    }
}
