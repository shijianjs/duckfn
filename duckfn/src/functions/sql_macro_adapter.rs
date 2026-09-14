//! SQL 宏适配层：在指定连接上执行 SQL 文本，用于注册 SQL 宏。
//!
//! SQL-macro adapter: executes raw SQL on a given connection to register SQL macros.

use std::ffi::{CStr, CString};
use libduckdb_sys::{duckdb_connection, duckdb_query, duckdb_result, DuckDBSuccess, duckdb_result_error, duckdb_destroy_result};
use quack_rs::prelude::{Connection, ExtensionError};

/// 在连接 `c` 上执行一段 SQL（通常是一条或多条 `CREATE OR REPLACE MACRO`）。
///
/// 供 `#[duck_sql_macro]`（函数返回 SQL 字符串）与 `duck_sql_macro_files!`（内联 .sql 文件）
/// 使用；SQL 执行失败会转成 [`ExtensionError`]。
///
/// Executes a piece of SQL on connection `c` (usually one or more
/// `CREATE OR REPLACE MACRO` statements). Used by `#[duck_sql_macro]` (functions returning a
/// SQL string) and `duck_sql_macro_files!` (inlined `.sql` files); a failure is converted into
/// an [`ExtensionError`].
///
/// # Errors
///
/// SQL 中含有 NUL 字节、连接无效或 DuckDB 执行报错时返回错误。
///
/// Returns an error if the SQL contains an interior NUL byte, the connection is invalid, or
/// DuckDB reports an execution error.
pub fn register_sql_macro_str(c: &Connection, sql: &str) -> Result<(), ExtensionError> {
    unsafe { execute_sql(c.as_raw_connection(), sql) }
}

/// 复制自 `quack_rs::sql_macro`，原实现不公开。
///
/// Copied from `quack_rs::sql_macro`; the original implementation is not public.
///
/// 在 `con` 上执行一条 SQL 语句，并把 DuckDB 的错误暴露为 [`ExtensionError`]。
/// 无论成功与否都会调用 `duckdb_destroy_result`。
///
/// Executes a SQL statement on `con`, surfacing any `DuckDB` error. Always calls
/// `duckdb_destroy_result`, even on failure.
///
/// # Safety
///
/// `con` 必须是有效的、已打开的 [`duckdb_connection`]。
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
