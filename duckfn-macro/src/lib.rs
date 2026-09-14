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
#[proc_macro_attribute]
pub fn duck_sql_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_sql_macro())
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
