mod duck_struct_derive;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(DuckStruct)]
pub fn duck_struct_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match duck_struct_derive::duck_struct_derive(input) {
        Ok(token) => TokenStream::from(token),
        Err(err) => TokenStream::from(err.to_compile_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;


}
