use quack_rs::aggregate::AggregateFunctionBuilder;
use quack_rs::aggregate::builder::OverloadBuilder;
use quack_rs::prelude::{LogicalType, ScalarFunctionBuilder, ScalarOverloadBuilder};

pub trait BuilderWithParams: Sized {

    fn builder_param_logical(self, logical_type: LogicalType) -> Self;

    fn with_params(self, params: Vec<LogicalType>) -> Self {
        let mut builder = self;
        for param in params {
            builder = builder.builder_param_logical(param);
        }
        builder
    }
}

impl BuilderWithParams for ScalarFunctionBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}

impl BuilderWithParams for ScalarOverloadBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}

impl BuilderWithParams for AggregateFunctionBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}
impl BuilderWithParams for OverloadBuilder {
    fn builder_param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
}
