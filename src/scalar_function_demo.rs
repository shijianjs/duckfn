use libduckdb_sys::duckdb_connection;
use quack_rs::connection::Connection;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::Registrar;
use tuple_transpose::TupleTranspose;
use crate::aggregate_function_demo;
use crate::scalar_function_wrapper::{
    ScalarFunctionAdapter,
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

impl ScalarFunctionAdapter for DoubleIt {
    const NAME: &'static str = "double_it5";
    type Args = (Option<i64>,);
    type Output = i64;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| v * 2)
    }
}


pub struct FirstWordTuple;

impl ScalarFunctionAdapter for FirstWordTuple {
    const NAME: &'static str = "first_word_tuple";
    type Args = (Option<String>,);
    type Output = String;

    fn apply(args: Self::Args) -> Option<Self::Output> {
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

impl ScalarFunctionAdapter for AddItTuple {
    const NAME: &'static str = "add_it_tuple";
    type Args = (Option<i64>, Option<i64>);
    type Output = i64;
    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v, v2)| v + v2)
    }
}


pub unsafe fn register(connection: &Connection) -> Result<(), ExtensionError> {
    unsafe {
        let builders = vec![
            DoubleIt::register_builder(),
            FirstWordTuple::register_builder(),
            AddItTuple::register_builder(),
        ];

        for builder in builders {
            connection.register_scalar(builder)?;
        }
    }
    Ok(())
}