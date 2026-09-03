mod duck_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;

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
    let _args: DuckArgs = match syn::parse(_attr) {
        Ok(v) => v,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };
    let wrapper = ItemFnWrapper::new(input);
    let result = run(wrapper);
    handle_token_stream2_result(result)
}

#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckArgs {
    pub named_param_after: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
}
