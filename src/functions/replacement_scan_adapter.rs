//! replacement scan 适配层：把 DuckDB 不认识的表名（通常是文件路径）重定向到表函数。
//!
//! Replacement-scan adapter: redirects table names unknown to DuckDB (usually file paths) to
//! a table function.

use crate::{DuckExtraInfo, DuckOptionResult, DuckResult, panic_to_string, raw_extra_info};
use libduckdb_sys::duckdb_replacement_scan_info;
use quack_rs::prelude::{Connection, ReplacementScanInfo};
use std::panic::catch_unwind;

/// 把「DuckDB 不认识的表名（通常是文件路径）」重定向到某个表函数。
///
/// DuckDB 遇到未知表引用时会按注册顺序调用所有 replacement scan 回调
/// （字符串字面量也算，例如 `SELECT * FROM 'data.points'`），每个回调可以：
///
/// - **接管**：`handle_path` 返回 `Ok(Some(table_function))`，适配层调用
///   [`ReplacementScanInfo::set_function`] 把扫描重定向到该表函数，并把表名/路径
///   作为**第一个 VARCHAR 参数**传给这个表函数（见 [`ReplacementScanAdapter::handle_info`]）；
/// - **不管**：返回 `Ok(None)`，DuckDB 继续尝试下一个 replacement scan 回调；
/// - **报错**：返回 `Err`，整条查询直接以该错误结束。
///
/// 由于回调对「所有未解析的表名」都会被调用，`handle_path` 一定要在路径不匹配时
/// 返回 `Ok(None)`，否则会把别人的表名也抢过来。
///
/// Redirects table names unknown to DuckDB (usually file paths) to a table function. Whenever
/// DuckDB encounters an unresolved table reference (string literals included, e.g.
/// `SELECT * FROM 'data.points'`) it calls every registered replacement-scan callback in
/// registration order, and each callback may: **take over** (return
/// `Ok(Some(table_function))`; the adapter calls [`ReplacementScanInfo::set_function`] to
/// redirect the scan and passes the table name/path as the **first VARCHAR argument** to that
/// table function — see [`ReplacementScanAdapter::handle_info`]), **pass** (return
/// `Ok(None)`, letting DuckDB try the next callback), or **fail** (return `Err`, ending the
/// whole query with that error). Because the callback is invoked for *every* unresolved table
/// name, `handle_path` must return `Ok(None)` when the path does not match; otherwise it
/// would hijack other tables.
///
/// # 示例 / Examples
///
/// ```ignore
/// struct MyScan;
///
/// impl duckfn::ReplacementScanAdapter for MyScan {
///     const NAME: &'static str = "MyScan";
///
///     fn handle_path(path: &str) -> duckfn::DuckOptionResult<String> {
///         if path.ends_with(".myformat") {
///             return Ok(Some("read_myformat".to_string()));
///         }
///         Ok(None)
///     }
/// }
/// ```
///
/// 一般不用手写这个 impl，直接用 `#[duck_replacement_scan]` 作用在
/// `fn(path: &str) -> DuckOptionResult<String>` 上即可。
///
/// Usually you do not implement this manually: annotate
/// `fn(path: &str) -> DuckOptionResult<String>` with `#[duck_replacement_scan]` instead.
pub trait ReplacementScanAdapter: Sized + 'static {
    /// 只用于标识回调（replacement scan 本身没有 SQL 名字）。
    ///
    /// Used to identify the callback only (a replacement scan has no SQL name of its own).
    const NAME: &'static str;

    /// 在 `c` 背后的数据库上注册本 replacement scan 回调。
    ///
    /// Registers this replacement-scan callback on the database behind `c`.
    ///
    /// # Errors
    ///
    /// 目前注册本身不会失败，返回 `DuckResult` 是为了和 inventory 里的
    /// `DuckRegisterFn`（`fn(&Connection) -> DuckResult<()>`）对齐。
    ///
    /// Registration itself cannot currently fail; the `DuckResult` return type exists to
    /// match the inventory `DuckRegisterFn` signature (`fn(&Connection) -> DuckResult<()>`).
    fn register(c: &Connection) -> DuckResult<()> {
        // 附加数据走注册参数 extra_data：回调的第 3 个参数就是它（见 [`Self::scan_callback`]）。
        //
        // The extra data travels as the registration argument `extra_data`: it is the third
        // parameter of the callback (see [`Self::scan_callback`]).
        let (extra_data, delete_callback) = match raw_extra_info(Self::extra_info()) {
            Some((ptr, destroy)) => (ptr, destroy),
            None => (std::ptr::null_mut(), None),
        };
        // SAFETY: c.as_raw_database() 是 DuckDB 交给扩展的有效 duckdb_database，
        // Self::scan_callback 的签名满足 ReplacementScanFn 的要求；extra_data 由
        // `DuckExtraInfo::into_raw` 产生，delete_callback 与它配对。
        //
        // SAFETY: c.as_raw_database() is a valid `duckdb_database` handed to the extension by
        // DuckDB, Self::scan_callback matches ReplacementScanFn, `extra_data` comes from
        // `DuckExtraInfo::into_raw` and `delete_callback` matches it.
        unsafe {
            c.register_replacement_scan(Self::scan_callback, extra_data, delete_callback);
        }
        Ok(())
    }

    /// 注册期附加的数据（DuckDB replacement scan 的 `extra_data`）；默认不附加。
    ///
    /// 数据在数据库关闭时由 DuckDB 调用析构回调释放，因此类型必须是 `Send + Sync + 'static`。
    ///
    /// Function-level data attached at registration time (a DuckDB replacement scan's `extra_data`);
    /// nothing is attached by default. DuckDB frees it through the destructor when the database is
    /// closed, so the type must be `Send + Sync + 'static`.
    fn extra_info() -> Option<DuckExtraInfo> {
        None
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` / `table_name` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `info` and `table_name` are guaranteed valid by DuckDB.
    unsafe extern "C" fn scan_callback(
        info: duckdb_replacement_scan_info,
        table_name: *const ::std::os::raw::c_char,
        data: *mut ::std::os::raw::c_void,
    ) {
        // SAFETY: table_name 是 DuckDB 传入的以 NUL 结尾的 C 字符串。
        let path = unsafe { std::ffi::CStr::from_ptr(table_name) };
        // 表名不是合法 UTF-8 时无法当作路径处理：什么都不做，
        // 让 DuckDB 继续尝试其他回调（或最终报「表不存在」）。
        let Ok(path) = path.to_str() else {
            return;
        };

        // SAFETY: data 是注册时交出去的 `DuckExtraInfo` 裸指针（没挂附加数据时为 null）。
        //
        // SAFETY: `data` is the raw `DuckExtraInfo` pointer handed to DuckDB at registration time
        // (null when nothing was attached).
        let extra = unsafe { DuckExtraInfo::from_raw(data) };

        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let scan_info = unsafe { ReplacementScanInfo::new(info) };
        match catch_unwind(|| Self::handle_info_with_extra(&scan_info, path, extra)) {
            Ok(Ok(())) => {}
            // 业务错误：交给 DuckDB 变成查询错误。
            Ok(Err(e)) => scan_info.set_error(e.as_str()),
            // panic 不跨 FFI：转成查询错误。
            Err(e) => scan_info.set_error(&panic_to_string(e)),
        }
    }

    /// 接管时把 `path` 注册为目标表函数的第一个 VARCHAR 参数。
    ///
    /// `handle_path` 返回 `Ok(None)` 时什么都不做；返回 `Err` 时错误会冒泡到
    /// [`Self::scan_callback`]，最终成为 `duckdb_replacement_scan_set_error`。
    ///
    /// When taking over, registers `path` as the first VARCHAR argument of the target table
    /// function. If `handle_path` returns `Ok(None)` nothing happens; if it returns `Err`, the
    /// error propagates to [`Self::scan_callback`] and ends up as
    /// `duckdb_replacement_scan_set_error`.
    fn handle_info(info: &ReplacementScanInfo, path: &str) -> DuckResult<()> {
        if let Some(table_fn) = Self::handle_path(path)? {
            info.set_function(&table_fn).add_varchar_parameter(path);
        }
        Ok(())
    }

    /// 同 [`Self::handle_info`]，但带上 [`Self::extra_info`] 挂的附加数据。
    ///
    /// 默认忽略 `extra` 并转调 [`Self::handle_info`]；需要读 `extra_info` 时重写本方法。
    ///
    /// Same as [`Self::handle_info`] but carrying the data attached through [`Self::extra_info`].
    /// By default it ignores `extra` and delegates to [`Self::handle_info`]; override it to read the
    /// `extra_info`.
    fn handle_info_with_extra(
        info: &ReplacementScanInfo,
        path: &str,
        extra: Option<&DuckExtraInfo>,
    ) -> DuckResult<()> {
        let _ = extra;
        Self::handle_info(info, path)
    }

    /// 决定表名（路径）由哪个表函数接管：
    /// `Ok(Some("read_xxx"))` 重定向，`Ok(None)` 表示不管，`Err` 让查询失败。
    ///
    /// Decides which table function takes over the table name (path): `Ok(Some("read_xxx"))`
    /// redirects, `Ok(None)` passes, and `Err` fails the query.
    fn handle_path(path: &str) -> DuckOptionResult<String>;
}
