use crate::{DuckOptionResult, DuckResult, panic_to_string};
use libduckdb_sys::duckdb_replacement_scan_info;
use quack_rs::prelude::{Connection, ReplacementScanBuilder, ReplacementScanInfo};
use std::panic::catch_unwind;

pub trait ReplacementScanAdapter {
    fn register(c: &Connection) {
        // Low-level: pass raw extra_data and an optional delete callback.
        unsafe {
            ReplacementScanBuilder::register(
                c.as_raw_database(),  // duckdb_database
                Self::scan_callback,  // ReplacementScanFn
                std::ptr::null_mut(), // extra_data (or a raw pointer)
                None,                 // delete_callback
            )
        }
    }

    unsafe extern "C" fn scan_callback(
        info: duckdb_replacement_scan_info,
        table_name: *const ::std::os::raw::c_char,
        _data: *mut ::std::os::raw::c_void,
    ) {
        let path = unsafe { std::ffi::CStr::from_ptr(table_name) }.to_str();

        if let Ok(path) = path {
            let scan_info = unsafe { ReplacementScanInfo::new(info) };
            let r = catch_unwind(|| {
                let result = Self::handle_info(&scan_info, path);
                if let Err(e) = result {
                    scan_info.set_error(e.as_str());
                    return;
                }
            });
            if let Err(e) = r {
                scan_info.set_error(&panic_to_string(e));
            }
        }
    }

    fn handle_info(info: &ReplacementScanInfo, path: &str) -> DuckResult<()> {
        if let Some(table_fn) = Self::handle_path(path)? {
            info.set_function(&table_fn).add_varchar_parameter(path);
        }
        Ok(())
    }

    /// Determines the table function to use based on the file path.
    /// 示例：
    /// ```
    ///     fn handle_path(path: &str) -> DuckOptionResult<String> {
    ///         if path.ends_with(".myformat") {
    ///             return Ok(Some("read_myformat".to_string()));
    ///         }
    ///         Ok(None)
    ///     }
    /// ```
    fn handle_path(path: &str) -> DuckOptionResult<String> {
        Ok(None)
    }
}
