mod count_down_m {

    #[derive(Debug, Clone, easy_duckdb_extension_macro::DuckStruct)]
    pub struct TableFunArgs {
        pub start: i32,
        pub b: Option<i32>,
    }
    // impl easy_duckdb_extension::DuckBindArgs for TableFunArgs {
    //     fn read_bind_args(
    //         bind: &quack_rs::prelude::BindInfo,
    //     ) -> easy_duckdb_extension::DuckResult<Self> {
    //         use easy_duckdb_extension::DuckValueType;
    //         Ok(TableFunArgs {
    //             start: i32::read_by_duck_value(
    //                 &(unsafe { bind.get_parameter_value(0) }),
    //             )?
    //             .ok_or_else(|| easy_duckdb_extension::duck_error("start cannot be null"))?,
    //             b: i32::read_by_duck_value(&(unsafe { bind.get_named_parameter_value("b") }))?,
    //         })
    //     }
    //
    //     fn bind_param_logical() -> Vec<(
    //         Option<impl Into<String>>,
    //         quack_rs::prelude::LogicalType,
    //     )> {
    //         use easy_duckdb_extension::DuckValueType;
    //         vec![
    //             (None, i32::logical_type()),
    //             (Some("b"), i32::logical_type()),
    //         ]
    //     }
    // }
}

#[test]
fn aa() {
    let a = String::from("a") + "b";
    println!("{}", a)
}