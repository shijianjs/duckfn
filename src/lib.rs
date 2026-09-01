mod demo;
pub mod wrapper;

use demo::{aggregate_function_demo, scalar_function_demo, table_function_demo};
use easy_duckdb_extension::ScalarFunctionAdapter;
use quack_rs::connection::Connection;
use quack_rs::entry_point_v2;
use quack_rs::error::ExtensionError;

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    unsafe {
        scalar_function_demo::register(connection)?;
        // DoubleIt::register(con)?;
        // FirstWordTuple::register(con)?;
        // AddItTuple::register(con)?;
        aggregate_function_demo::register(connection)?;
        table_function_demo::register(connection)?;
    }
    Ok(())
}

/// 符号名称必须为 {extension_name}_init_c_api ，全部小写，仅包含下划线。如果符号缺失或名称错误，DuckDB 将无法加载扩展。
entry_point_v2!(rusty_quack_init_c_api, register);
