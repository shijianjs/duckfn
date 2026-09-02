
mod count_down_m{

    #[derive(Debug,Clone,easy_duckdb_extension_macro::DuckStruct)]
    pub struct TableFunArgs{
        pub start: i32,
        pub b: Option<i32>,
    }
    impl easy_duckdb_extension::DuckBindArgs for TableFunArgs {
        fn read_args(bind: &quack_rs::prelude::BindInfo) -> easy_duckdb_extension::DuckResult<Self> {
            use easy_duckdb_extension::DuckValueType;
            let value1:Option<i32> = i32::read_by_duck_value(&(unsafe { bind.get_named_parameter_value("start") }))?;
            Ok(
                TableFunArgs {
                    start: value1.ok_or_else(||easy_duckdb_extension::duck_error("start cannot be null"))?,
                    b: value1,
                }
            )
        }

        fn named_param_logical() -> std::vec::Vec<(std::option::Option<impl std::convert::Into<std::string::String>>, quack_rs::prelude::LogicalType)> {
            use easy_duckdb_extension::DuckValueType;
            vec![
                (Some("start"), i32::logical_type()),
                (Some("b"), i32::logical_type()),
            ]
        }

    }
}