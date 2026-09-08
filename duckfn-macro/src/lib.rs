mod duck_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;
mod entrypoint;
mod attr_args;

use quote::quote;
use syn::{LitStr};
use crate::duck_function::ItemFnWrapper;
use crate::macro_utils::{TokenStream2Result, handle_token_stream2_result};
use darling::FromMeta;
use proc_macro::TokenStream;
use syn::parse::Parser;
use syn::{DeriveInput, ItemFn, Meta, parse_macro_input};
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

#[cfg(test)]
mod tests {
    use super::*;
}
