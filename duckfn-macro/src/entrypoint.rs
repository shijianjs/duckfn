use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};

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
        quack_rs::entry_point_v2!(
            #symbol,
            duckfn::register_all_duckfn
        );
    }.into()
}
