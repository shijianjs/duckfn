mod duck_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;
mod entrypoint;
mod attr_args;

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

#[cfg(test)]
mod tests {
    use super::*;
}
