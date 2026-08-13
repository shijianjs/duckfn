// src/lib.rs
pub mod aggregate_function_demo;
pub mod scalar_function_demo;
mod scalar_function_official_demo;
pub mod wrapper;

use libduckdb_sys::duckdb_connection;
use quack_rs::connection::Connection;
use quack_rs::entry_point_v2;
use quack_rs::error::ExtensionError;
use wrapper::scalar_function_wrapper::ScalarFunctionAdapter;

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let con: duckdb_connection = connection.as_raw_connection();
    unsafe {
        scalar_function_demo::register(connection)?;
        // DoubleIt::register(con)?;
        // FirstWordTuple::register(con)?;
        // AddItTuple::register(con)?;
        aggregate_function_demo::register_aggregate_demo(con)?;
    }
    Ok(())
}

/// 符号名称必须为 {extension_name}_init_c_api ，全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
entry_point_v2!(rusty_quack_init_c_api, register);
