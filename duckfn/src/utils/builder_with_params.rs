//! builder 扩展 trait：让不同种类的函数 builder 都能用同一套「批量设置参数」的写法。
//!
//! Builder extension trait: gives the different function builders one shared "set the
//! parameter list in bulk" API.

use quack_rs::aggregate::AggregateFunctionBuilder;
use quack_rs::aggregate::builder::OverloadBuilder;
use quack_rs::prelude::{LogicalType, ScalarFunctionBuilder, ScalarOverloadBuilder};

/// 为各类函数 builder 提供统一的「追加一个参数」与「批量设置参数」能力。
///
/// quack-rs 里标量/聚合函数的各种 builder 都各自有 `param_logical`，签名却不完全一致；
/// 本 trait 把它们归一化，使适配层可以只写一份 `with_params(Self::Args::column_types())`。
///
/// Provides a uniform "append one parameter" and "set the parameter list in bulk" capability
/// for the various function builders. In quack-rs each scalar/aggregate builder has its own
/// `param_logical` with slightly different signatures; this trait normalises them so the
/// adapter layer can simply call `with_params(Self::Args::column_types())`.
pub trait BuilderWithParams: Sized {

    /// 追加一个参数（逻辑类型），返回更新后的 builder。
    ///
    /// Appends one parameter (by logical type) and returns the updated builder.
    fn builder_param_logical(self, logical_type: LogicalType) -> Self;

    /// 按顺序追加一批参数（逻辑类型），返回更新后的 builder。
    ///
    /// Appends a batch of parameters (by logical type) in order and returns the updated
    /// builder.
    fn with_params(self, params: Vec<LogicalType>) -> Self {
        let mut builder = self;
        for param in params {
            builder = builder.builder_param_logical(param);
        }
        builder
    }
}

// 为「独立标量函数」builder 实现参数追加。
//
// Parameter appending for the standalone scalar-function builder.
impl BuilderWithParams for ScalarFunctionBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}

// 为「标量函数集重载」builder 实现参数追加。
//
// Parameter appending for the scalar-overload builder.
impl BuilderWithParams for ScalarOverloadBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}

// 为「独立聚合函数」builder 实现参数追加。
//
// Parameter appending for the standalone aggregate-function builder.
impl BuilderWithParams for AggregateFunctionBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}

// 为「聚合函数集重载」builder 实现参数追加。
//
// Parameter appending for the aggregate-overload builder.
impl BuilderWithParams for OverloadBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}
