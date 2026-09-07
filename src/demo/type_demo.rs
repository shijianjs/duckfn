use duckfn_macro::duck_table_function;

#[derive(Default, Debug, Clone, duckfn_macro::DuckStruct)]
pub struct CountDownOutput {
    n: i64,
}
// 可以处理入参异常，出去的不处理
//创建数据源可能失败，但 iterator 一旦创建成功，后续只产生正常数据。
fn demo3(start: i64) -> duckfn::DuckResult<impl Iterator<Item = CountDownOutput>> {
    Ok(table_fun_simple(start))
}
// 都不处理
//参数已经成功解析，后面不会产生 error，也不会产生 NULL。
#[duck_table_function]
fn table_fun_simple(start: i64) -> impl Iterator<Item = CountDownOutput> {
    (0..start).rev()
        .map(|x| CountDownOutput { n: x })
}

// 全功能
//底层/高级模式
fn demo1(start: i64) -> duckfn::DuckFullIteratorResult<CountDownOutput> {
    Ok(Box::new(table_fun_simple(start).map(|x| {
        if x.n==5{
            Ok(None)
        }else {
            Ok(Some(x))
        }
    })))
}

pub mod demo1{
    use super::*;

    pub struct TableFunctionImpl;

    #[derive(Default, Debug, Clone, duckfn_macro::DuckStruct)]
    #[duck(named_param_from = "start")]
    pub struct TableFunctionArgs {
        start: i64,
    }

    impl duckfn::TableFunctionAdapter for TableFunctionImpl {
        const NAME: &'static str = "count_down_s";
        type Args = TableFunctionArgs;
        type Output = CountDownOutput;

        fn init_data_iterator(
            args: Self::Args,
        ) -> duckfn::DuckResult<duckfn::DuckFullIterator<Self::Output>> {
            let a =1;
            if a ==4 {
                let result = table_fun_simple(args.start);
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