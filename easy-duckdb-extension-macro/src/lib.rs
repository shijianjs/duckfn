mod duck_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;

use crate::duck_function::ItemFnWrapper;
use crate::macro_utils::{TokenStream2Result, handle_token_stream2_result};
use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn, parse_macro_input};

#[proc_macro_derive(DuckStruct,attributes(duck))]
pub fn duck_struct_derive(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = duck_struct_derive::duck_struct_derive(derive_input);
    handle_token_stream2_result(result)
}
#[proc_macro_attribute]
pub fn duck_scalar_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let result = ItemFnWrapper::new(input).build_scalar_function();
    handle_token_stream2_result(result)
}
#[proc_macro_attribute]
pub fn duck_aggregate_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let result = ItemFnWrapper::new(input).build_aggregate_function();
    handle_token_stream2_result(result)
}

#[cfg(test)]
mod tests {
    use super::*;
}
