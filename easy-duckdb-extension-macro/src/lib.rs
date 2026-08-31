mod duck_scalar_function;
mod duck_struct_derive;
pub(crate) mod macro_utils;

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn, parse_macro_input};
use crate::macro_utils::{handle_token_stream2_result, TokenStream2Result};

#[proc_macro_derive(DuckStruct)]
pub fn duck_struct_derive(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = duck_struct_derive::duck_struct_derive(derive_input);
    handle_token_stream2_result(result)
}
#[proc_macro_attribute]
pub fn duck_scalar_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let result = duck_scalar_function::duck_scalar_function(input);
    handle_token_stream2_result(result)
}


#[cfg(test)]
mod tests {
    use super::*;
}
