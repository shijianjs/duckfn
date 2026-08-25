use syn::__private::TokenStream2;
use syn::spanned::Spanned;
use syn::{Data, DataStruct, DeriveInput, Fields, FieldsNamed};

pub(crate) fn duck_struct_derive(input: DeriveInput) -> syn::Result<TokenStream2> {
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = input.data
    {

    } else {
        return Err(syn::Error::new(
            input.span(),
            "Only named fields are allowed",
        ));
    }

    todo!()
}
