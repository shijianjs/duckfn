use crate::scalar_function_wrapper::{
    OneArgScalarFunctionAdapter, ScalarFunction, TwoArgScalarFunctionAdapter,
};

pub struct DoubleIt;

impl OneArgScalarFunctionAdapter for DoubleIt {
    const NAME: &'static str = "double_it5";
    type Arg1Type = i64;
    type ResultType = i64;
    fn apply(v: i64) -> i64 {
        v * 2
    }
}

pub struct AddIt;

impl TwoArgScalarFunctionAdapter for AddIt {
    const NAME: &'static str = "add_it5";
    type Arg1Type = i64;
    type Arg2Type = i64;
    type ResultType = i64;
    fn apply(v: i64, v2: i64) -> i64 {
        v + v2
    }
}

pub struct FirstWord;

impl OneArgScalarFunctionAdapter for FirstWord {
    const NAME: &'static str = "first_word4";
    type Arg1Type = String;
    type ResultType = String;
    fn apply(v: String) -> String {
        v.split_whitespace().next().unwrap_or("").to_string()
    }
}
pub struct FirstWordTuple;

impl ScalarFunction for FirstWordTuple {
    const NAME: &'static str = "first_word_tuple";
    type Args = (Option<String>,);
    type Result = String;

    fn apply(args: Self::Args) -> Option<Self::Result> {
        args.0.map(|v| {
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
        args.0.zip(args.1).map(|(v, v2)| v + v2)
    }
}
