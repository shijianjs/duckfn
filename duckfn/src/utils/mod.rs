//! 内部工具：builder 扩展 trait、错误/panic 转换辅助函数。
//!
//! Internal utilities: builder extension traits and error/panic conversion helpers.

/// builder 扩展 trait：统一为各种 builder 设置参数列表。
///
/// Builder extension trait: sets parameter lists uniformly for the various builders.
pub(crate) mod builder_with_params;
/// 结果类型别名、错误构造、panic 捕获辅助函数。
///
/// Result type aliases, error construction and panic-catching helpers.
pub(crate) mod helpers;

pub use builder_with_params::*;
pub use helpers::*;
