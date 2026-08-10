use tuple_transpose::TupleTranspose;
use crate::scalar_function_wrapper::{
    OneArgScalarFunctionAdapter, ScalarFunction, TwoArgScalarFunctionAdapter,
};


///
///
/// ```shell
/// cargo duckdb-ext build; duckdb -unsigned -c "
///   LOAD './target/debug/rusty_quack.duckdb_extension';
///   SELECT double_it5(3);
///   ";
/// ```
pub struct DoubleIt;

impl ScalarFunction for DoubleIt {
    const NAME: &'static str = "double_it5";
    type Args = (Option<i64>,);
    type Result = i64;

    fn apply(args: Self::Args) -> Option<Self::Result> {
        args.transpose().map(|(v,)| v * 2)
    }
}


pub struct FirstWordTuple;

impl ScalarFunction for FirstWordTuple {
    const NAME: &'static str = "first_word_tuple";
    type Args = (Option<String>,);
    type Result = String;

    fn apply(args: Self::Args) -> Option<Self::Result> {
        args.transpose().map(|(v,)| {
            v.as_str()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
    }
}

pub struct AddItTuple;

impl ScalarFunction for AddItTuple {
    const NAME: &'static str = "add_it_tuple";
    type Args = (Option<i64>, Option<i64>);
    type Result = i64;
    fn apply(args: Self::Args) -> Option<Self::Result> {
        args.transpose().map(|(v, v2)| v + v2)
    }
}
