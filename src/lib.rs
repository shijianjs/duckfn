// src/lib.rs
pub mod scalar_function_wrapper;
pub mod value_type_convertor;
pub mod scalar_function_demo;

use crate::scalar_function_wrapper::{OneArgScalarFunctionAdapter, ScalarFunctionAdapter, TwoArgScalarFunctionAdapter};
use libduckdb_sys::duckdb_connection;
use quack_rs::connection::Connection;
use quack_rs::entry_point_v2;
use quack_rs::error::ExtensionError;
use scalar_function_demo::{AddIt, DoubleIt, FirstWord};

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let con: duckdb_connection = connection.as_raw_connection();
    unsafe {
        DoubleIt::register(con)?;
        AddIt::register(con)?;
        FirstWord::register(con)?;
    }
    Ok(())
}

entry_point_v2!(rusty_quack_init_c_api, register);
