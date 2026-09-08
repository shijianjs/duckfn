mod duck_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;
use quote::quote;
use syn::{LitStr};
use crate::duck_function::ItemFnWrapper;
use crate::macro_utils::{TokenStream2Result, handle_token_stream2_result};
use darling::FromMeta;
use proc_macro::TokenStream;
use syn::parse::Parser;
use syn::{DeriveInput, ItemFn, Meta, parse_macro_input};

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

fn handle_duck_function(
    _attr: TokenStream,
    item: TokenStream,
    run: fn(ItemFnWrapper) -> TokenStream2Result,
) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let duck_args: DuckArgs = match syn::parse(_attr.clone()) {
        Ok(v) => v,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };
    
    let wrapper = ItemFnWrapper { 
        item_fn:input,
        attr: _attr.into(),
        duck_args,
    };
    let result = run(wrapper);
    handle_token_stream2_result(result)
}

#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckArgs {
    pub named_param_from: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
}
