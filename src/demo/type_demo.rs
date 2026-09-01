
mod count_down_m{
    use quack_rs::prelude::{BindInfo, LogicalType, Value};
    use easy_duckdb_extension::{duck_error, DuckBindArgs, DuckResult, DuckValueType};
    use easy_duckdb_extension_macro::DuckStruct;

    #[derive(Debug,Clone,DuckStruct)]
    pub struct TableFunArgs{
        pub start: i32,
        pub b: Option<i32>,
    }
    impl DuckBindArgs for TableFunArgs {
        fn read_args(bind: &BindInfo) -> DuckResult<Self> {
            let value: Value = unsafe { bind.get_named_parameter_value("start") };
            let value1:Option<i32> = i32::read_by_duck_value(&value)?;
            Ok(
                TableFunArgs {
                    start: value1.ok_or(duck_error("start cannot be null"))?,
                    b: value1,
                }
            )
        }

        fn param_logical() -> Vec<LogicalType> {
            todo!()
        }

        fn named_param_logical() -> Vec<(String, LogicalType)> {
            todo!()
        }
    }
}