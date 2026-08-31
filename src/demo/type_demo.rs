fn error_scalar_demo(input: i64,input2: i64) -> easy_duckdb_extension::DuckOptionResult<i64> {
    Ok(Some(input * 2))
}

pub mod error_scalar_demo {
    use super::*;

    #[derive(easy_duckdb_extension_macro::DuckStruct, Clone)]
    pub struct DuckArgsImpl {
        input: i64,
        input2: i64,
    }

    pub struct ScalarFunctionImpl;
    impl easy_duckdb_extension::ScalarFunctionAdapter for ScalarFunctionImpl {
        const NAME: &'static str = "error_scalar_demo";
        type Args = DuckArgsImpl;
        type Output = i64;

        fn apply(args: Self::Args) -> easy_duckdb_extension::DuckOptionResult<Self::Output> {
            let result = error_scalar_demo(
                args.input,
                args.input2
            );
            result
        }
    }
}
