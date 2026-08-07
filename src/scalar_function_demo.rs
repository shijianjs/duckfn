use crate::scalar_function_wrapper::{OneArgScalarFunctionAdapter, TwoArgScalarFunctionAdapter};

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