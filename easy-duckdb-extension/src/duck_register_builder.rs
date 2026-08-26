use crate::value_types::duck_value_type::DuckTypeInfo;
use quack_rs::aggregate::AggregateFunctionBuilder;
use quack_rs::prelude::{LogicalType, ScalarFunctionBuilder, ScalarOverloadBuilder, TypeId};

pub trait RegisterBuilder: Sized {
    fn param(self, type_id: TypeId) -> Self;

    fn param_logical(self, logical_type: LogicalType) -> Self;

    fn returns(self, type_id: TypeId) -> Self;

    fn returns_logical(self, logical_type: LogicalType) -> Self;

    fn with_return_type(self, return_info: DuckTypeInfo) -> Self {
        self.returns_logical(return_info.logical_type)
    }

    fn with_params(self, params: Vec<DuckTypeInfo>) -> Self {
        let mut builder = self;
        for param in params {
            builder = builder.param_logical(param.logical_type);
            // if let Some(t) = param.type_id {
            //     builder = builder.param(t)
            // } else if let Some(t) = param.logical_type {
            //     builder = builder.param_logical(t)
            // }
        }
        builder
    }
}

impl RegisterBuilder for ScalarFunctionBuilder {
    fn param(self, type_id: TypeId) -> Self {
        self.param(type_id)
    }
    fn param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
    fn returns(self, type_id: TypeId) -> Self {
        self.returns(type_id)
    }
    fn returns_logical(self, logical_type: LogicalType) -> Self {
        self.returns_logical(logical_type)
    }
}

impl RegisterBuilder for ScalarOverloadBuilder {
    fn param(self, type_id: TypeId) -> Self {
        self.param(type_id)
    }
    fn param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
    fn returns(self, type_id: TypeId) -> Self {
        self.returns(type_id)
    }
    fn returns_logical(self, logical_type: LogicalType) -> Self {
        self.returns_logical(logical_type)
    }
}

impl RegisterBuilder for AggregateFunctionBuilder {
    fn param(self, type_id: TypeId) -> Self {
        self.param(type_id)
    }
    fn param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
    fn returns(self, type_id: TypeId) -> Self {
        self.returns(type_id)
    }
    fn returns_logical(self, logical_type: LogicalType) -> Self {
        self.returns_logical(logical_type)
    }
}
