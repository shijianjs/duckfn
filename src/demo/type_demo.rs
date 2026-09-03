
#[derive(Default, Debug, Clone, easy_duckdb_extension_macro::DuckStruct)]
struct CountDownOutput {
    n: i64,
}
// 可以处理入参异常，出去的不处理
//创建数据源可能失败，但 iterator 一旦创建成功，后续只产生正常数据。
fn demo3(start: i64) -> easy_duckdb_extension::DuckResult<impl Iterator<Item = CountDownOutput>> {
    Ok(demo4(start))
}
// 都不处理
//参数已经成功解析，后面不会产生 error，也不会产生 NULL。
fn demo4(start: i64) -> impl Iterator<Item = CountDownOutput> {
    (0..start).rev()
        .map(|x| CountDownOutput { n: x })
}

// 全功能
//底层/高级模式
fn demo1(start: i64) -> easy_duckdb_extension::DuckResult<easy_duckdb_extension::DuckDataIterator<CountDownOutput>> {
    Ok(Box::new(demo4(start).map(|x| {
        if x.n==5{
            Ok(None)
        }else {
            Ok(Some(x))
        }
    })))
}

pub mod demo1{
    use super::*;

    struct TableFunctionImpl;

    #[derive(Default, Debug, Clone, easy_duckdb_extension_macro::DuckStruct)]
    #[duck(named_param_from = "start")]
    struct TableFunctionArgs {
        start: i64,
    }

    impl easy_duckdb_extension::TableFunctionAdapter for TableFunctionImpl {
        const NAME: &'static str = "count_down_s";
        type Args = TableFunctionArgs;
        type Output = CountDownOutput;

        fn init_data_iterator(
            args: Self::Args,
        ) -> easy_duckdb_extension::DuckResult<easy_duckdb_extension::DuckDataIterator<Self::Output>> {
            let a =1;
            if a ==4 {
                let result = demo4(args.start);
                Ok(Box::new(result.map(|x| Ok(Some(x)))))
            } else if a ==3 {
                let result = demo3(args.start);
                Ok(Box::new(result?.map(|x| Ok(Some(x)))))
            } else {
                let result = demo1(args.start);
                result
            }
        }
    }
}