//! `duckfn_entrypoint!` 的实现：校验扩展名并生成 DuckDB 入口符号。
//!
//! Implementation of `duckfn_entrypoint!`: validates the extension name and generates the
//! DuckDB entry-point symbol.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

/// `duckfn_entrypoint!("name")` 的实现。
///
/// 输入必须是一个非空字符串字面量，且只含小写字母、数字和下划线（DuckDB 对入口符号名有
/// 此要求）。校验通过后展开为 `quack_rs::entry_point_v2!({name}_init_c_api, ...)`，把扩展
/// 初始化指向 `duckfn::register_all_duckfn`；校验失败返回编译错误。
///
/// Implementation of `duckfn_entrypoint!("name")`. The input must be a non-empty string literal
/// containing only lowercase letters, digits and underscores (DuckDB requires this of entry-point
/// symbol names). On success it expands to `quack_rs::entry_point_v2!({name}_init_c_api, ...)`
/// pointing extension initialisation at `duckfn::register_all_duckfn`; on failure it returns a
/// compile error.
pub fn duckfn_entrypoint(input: TokenStream) -> TokenStream {
    let extension_name = parse_macro_input!(input as LitStr);

    let name = extension_name.value();

    if name.is_empty() {
        return syn::Error::new(
            extension_name.span(),
            "extension name must not be empty",
        )
            .to_compile_error()
            .into();
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return syn::Error::new(
            extension_name.span(),
            "extension name must contain only lowercase letters, digits, and underscores",
        )
            .to_compile_error()
            .into();
    }

    let symbol_name = format!("{name}_init_c_api");

    let symbol: syn::Ident = match syn::parse_str(&symbol_name) {
        Ok(symbol) => symbol,
        Err(_) => {
            return syn::Error::new(
                extension_name.span(),
                format!("invalid extension name `{name}`"),
            )
                .to_compile_error()
                .into();
        }
    };

    quote! {
        /// 符号名称必须为 `{name}_init_c_api`，
        /// 全部小写，仅包含下划线。
        /// 如果符号缺失或名称错误，DuckDB 将无法加载扩展。
        ///
        /// The symbol name must be `{name}_init_c_api`, all lowercase and containing only
        /// underscores. If the symbol is missing or misnamed, DuckDB cannot load the extension.
        quack_rs::entry_point_v2!(
            #symbol,
            duckfn::register_all_duckfn
        );
    }.into()
}
