use quack_rs::error::ExtensionError;
use quack_rs::prelude::ScalarFunctionInfo;
use std::panic::{UnwindSafe, catch_unwind};
use quack_rs::aggregate::AggregateFunctionInfo;

/// quack_rs结果类型
pub type DuckResult<T> = Result<T, ExtensionError>;
pub type DuckOptionResult<T> = DuckResult<Option<T>>;

/// 创建quack_rs错误
pub fn duck_error(message: impl Into<String>) -> ExtensionError {
    ExtensionError::new(message)
}

pub fn panic_to_duck_error(e: Box<dyn std::any::Any + Send>) -> ExtensionError {
    duck_error(panic_to_string(e))
}

pub fn vec_option_to_ref<T>(vec: &[Option<T>]) -> Vec<Option<&T>> {
    vec.iter().map(Option::as_ref).collect()
}

/// 将panic转换成字符串
pub fn panic_to_string(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
/// 处理scalar函数的panic
pub fn duck_scalar_unwind<F: FnOnce() -> R + UnwindSafe, R>(info: &ScalarFunctionInfo, f: F) {
    let unwind = catch_unwind(f);
    if let Err(e) = unwind {
        info.set_error(&panic_to_string(e));
    }
}
/// 处理aggregate函数的panic
pub fn duck_aggregate_unwind<F: FnOnce() -> R + UnwindSafe, R>(info: &AggregateFunctionInfo, f: F) {
    let unwind = catch_unwind(f);
    if let Err(e) = unwind {
        info.set_error(&panic_to_string(e));
    }
}
