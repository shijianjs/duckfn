// src/lib.rs
pub mod scalar_function_wrapper;
pub mod duck_value_type_convertor;
pub mod scalar_function_demo;
pub mod aggregate_function_demo;
mod duck_args_type;

use crate::scalar_function_wrapper::{OneArgScalarFunctionAdapter, ScalarFunctionAdapter, TwoArgScalarFunctionAdapter};
use libduckdb_sys::duckdb_connection;
use quack_rs::connection::Connection;
use quack_rs::entry_point_v2;
use quack_rs::error::ExtensionError;
use scalar_function_demo::{AddIt, DoubleIt, FirstWord};
use crate::scalar_function_demo::AddItTuple;

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let con: duckdb_connection = connection.as_raw_connection();
    unsafe {
        DoubleIt::register(con)?;
        AddIt::register(con)?;
        FirstWord::register(con)?;
        AddItTuple::register(con)?;
        aggregate_function_demo::register_aggregate_demo(con)?;
    }
    Ok(())
}

entry_point_v2!(rusty_quack_init_c_api, register);
