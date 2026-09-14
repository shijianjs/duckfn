mod duck_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;
mod entrypoint;
mod attr_args;
mod sql_macro_files;

use crate::macro_utils::handle_token_stream2_result;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};
use crate::attr_args::handle_duck_function;

#[proc_macro_derive(DuckStruct, attributes(duck))]
pub fn duck_struct_derive(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = duck_struct_derive::duck_struct_derive(derive_input);
    handle_token_stream2_result(result)
}
#[proc_macro_attribute]
pub fn duck_scalar_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_scalar_function())
}
#[proc_macro_attribute]
pub fn duck_aggregate_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_aggregate_function())
}
#[proc_macro_attribute]
pub fn duck_table_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_table_function())
}
#[proc_macro_attribute]
pub fn duck_custom_register(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_custom_register())
}
/// 把 `fn(源值) -> 目标值` 注册成 DuckDB 的 cast 函数，覆盖 `CAST(源 AS 目标)`。
///
/// 源类型来自唯一参数，目标类型来自返回类型；返回形式与 `duck_scalar_function` 一致：
///
/// ```ignore
/// #[duck_cast_function]
/// fn dfn_cast_str_to_int(s: String) -> DuckOptionResult<i32> {
///     s.parse().map_err(|_| duck_error("not an integer"))
/// }
///
/// // 允许把 NULL 带进函数体：入参写 Option<T>
/// #[duck_cast_function]
/// fn dfn_cast_bigint_to_double(v: Option<i64>) -> Option<f64> { ... }
///
/// // 允许 DuckDB 自动插入该转换
/// #[duck_cast_function(implicit_cost = 100)]
/// fn dfn_cast_str_to_bigint(s: String) -> i64 { ... }
/// ```
///
/// - `CAST(x AS T)` 出错 -> 整条查询失败（`set_error`）；
/// - `TRY_CAST(x AS T)` 出错 -> 该行输出 NULL 并记录行级错误（`set_row_error`）；
/// - 属性支持 `auto_register = false` / `implicit_cost = N`，生成模块里导出
///   `cast_function_builder()` 和 `cast_function_register()` 供手动注册。
#[proc_macro_attribute]
pub fn duck_cast_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_cast_function())
}

#[proc_macro_attribute]
pub fn duck_sql_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_sql_macro())
}

/// 把 `SELECT * FROM 'data.myformat'` 这类「未知表名/文件路径」重定向到某个表函数。
///
/// 函数签名只接受一个「表名（路径）」参数，返回目标表函数的名字：
///
/// ```ignore
/// #[duck_replacement_scan]
/// fn dfn_scan_points(path: &str) -> DuckOptionResult<String> {
///     if path.ends_with(".points") {
///         return Ok(Some("dfn_read_points".to_string()));
///     }
///     Ok(None)
/// }
/// ```
///
/// - `Ok(Some(table_function))`：接管，并把路径作为第一个 VARCHAR 参数传给该表函数；
/// - `Ok(None)`：不接管，DuckDB 继续尝试其他 replacement scan；
/// - `Err(..)` / panic：整条查询以该错误结束。
///
/// 返回值可以是 `Option<String>` / `Option<&'static str>` /
/// `DuckOptionResult<String>` / `DuckOptionResult<&'static str>`。
#[proc_macro_attribute]
pub fn duck_replacement_scan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_replacement_scan())
}





/// Generate DuckDB extension entry point.
///
/// ```ignore
/// duckfn_entrypoint!("rusty_quack");
/// ```
///
/// Expands to:
///
/// ```ignore
/// quack_rs::entry_point_v2!(
///     rusty_quack_init_c_api,
///     duckfn::register_all_duckfn
/// );
/// ```
#[proc_macro]
pub fn duckfn_entrypoint(input: TokenStream) -> TokenStream {
    entrypoint::duckfn_entrypoint(input)
}

/// 一次注册多个 SQL 脚本文件（快捷方式）。
///
/// ```ignore
/// duck_sql_macro_files!("sql/a.sql", "sql/b.sql", "sql/c.sql");
/// ```
///
/// 参数是可变多个字符串字面量（文件路径），支持尾随逗号，至少一个。
/// 每个文件在编译期用 `include_str!` 内联（路径相对「调用本宏的 .rs 文件」），
/// 扩展初始化时按书写顺序依次执行整段脚本 —— 一份脚本里可以有多条
/// 分号分隔的 `CREATE OR REPLACE MACRO` 语句。
///
/// 与 `#[duck_sql_macro]` 的关系：后者作用在函数上，由函数返回 SQL；
/// 本宏不需要写函数，直接把若干 .sql 文件注册出去。
#[proc_macro]
pub fn duck_sql_macro_files(input: TokenStream) -> TokenStream {
    sql_macro_files::duck_sql_macro_files(input)
}

#[cfg(test)]
mod tests {
    use super::*;
}
