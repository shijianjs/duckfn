use std::ffi::{CStr, CString};
use libduckdb_sys::{duckdb_connection, duckdb_query, duckdb_result, DuckDBSuccess, duckdb_result_error, duckdb_destroy_result};
use quack_rs::prelude::{Connection, ExtensionError};

/// Register a SQL macro from a sql string.
pub fn register_sql_macro_str(c: &Connection, sql: &str) -> Result<(), ExtensionError> {
    unsafe { execute_sql(c.as_raw_connection(), sql) }
}

/// Copy from `quack_rs::sql_macro`, origin is not public.
///
/// Executes a SQL statement on `con`, surfacing any `DuckDB` error.
///
/// Always calls `duckdb_destroy_result`, even on failure.
///
/// # Safety
///
/// `con` must be a valid, open [`duckdb_connection`].
unsafe fn execute_sql(con: duckdb_connection, sql: &str) -> Result<(), ExtensionError> {
    let c_sql = CString::new(sql)
        .map_err(|_| ExtensionError::new("SQL statement contains interior null bytes"))?;

    // Zero-initialize: duckdb_result contains only integer and pointer fields,
    // all of which are valid when zero / null.
    //
    // SAFETY: duckdb_result is a C struct; zero is a valid bit pattern for every field.
    let mut result: duckdb_result = unsafe { std::mem::zeroed() };

    // SAFETY: con is valid; c_sql is a valid nul-terminated C string.
    let rc = unsafe { duckdb_query(con, c_sql.as_ptr(), &raw mut result) };

    // Extract the error message before freeing, because duckdb_result_error
    // returns a pointer into the result's internal buffer.
    let outcome = if rc == DuckDBSuccess {
        Ok(())
    } else {
        // SAFETY: result was populated by duckdb_query; duckdb_result_error
        // returns a pointer valid until duckdb_destroy_result.
        let ptr = unsafe { duckdb_result_error(&raw mut result) };
        let msg = if ptr.is_null() {
            "DuckDB macro registration failed (no error message available)".to_string()
        } else {
            // SAFETY: ptr is a valid nul-terminated C string owned by the result.
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        };
        Err(ExtensionError::new(msg))
    };

    // SAFETY: result was populated by duckdb_query and must always be freed.
    unsafe { duckdb_destroy_result(&raw mut result) };

    outcome
}