use proc_macro2::Ident;
use quote::quote;
use syn::__private::TokenStream2;
use syn::spanned::Spanned;
use syn::{
    Data, DataStruct, DeriveInput, Fields, FieldsNamed, GenericArgument, Path, PathArguments, Type,
    TypePath,
};

pub(crate) fn duck_struct_derive(input: DeriveInput) -> syn::Result<TokenStream2> {
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = input.to_owned().data
    {
        let fields = named
            .into_iter()
            .map(|f| FieldWrapper { field: f })
            .collect();
        let context = DuckStructContext {
            input: input.to_owned(),
            fields,
        };
        return context.build_duck_value_type_impl();
    } else {
        return Err(syn::Error::new(
            input.span(),
            "Only named fields are allowed",
        ));
    }
}

struct DuckStructContext {
    input: DeriveInput,
    fields: Vec<FieldWrapper>,
}

impl DuckStructContext {
    fn struct_name(&self) -> &syn::Ident {
        &self.input.ident
    }
    fn build_duck_value_type_impl(&self) -> syn::Result<TokenStream2> {
        let struct_name = self.struct_name();
        Ok(quote! {

            impl easy_duckdb_extension::duck_value_type_convertor::DuckValueType for #struct_name {
                fn type_id() -> TypeId {
                    TypeId::Struct
                }

                fn logical_type() -> LogicalType {
                    LogicalType::struct_type_from_logical(&vec![(N::FIELD_NAMES[0], F0::logical_type())])
                }
            }
        })
    }
}

struct FieldWrapper {
    field: syn::Field,
}
impl FieldWrapper {
    fn field_name(&self) -> &Option<Ident> {
        &self.field.ident
    }
    fn require_field_name(&self) -> syn::Result<&Ident> {
        self.field
            .ident
            .as_ref()
            .ok_or(syn::Error::new(self.field.span(), "Field name is required"))
    }
    fn is_option(&self) -> darling::Result<&Type> {
        darling::util::extract_option::from_ref(&self.field.ty)
    }

    /// 穿透Option的类型
    /// - 如果是Option类型，则返回Option内部的类型
    /// - 如果不是Option类型，则返回当前类型
    /// - 只支持单层Option
    fn type_or_through_option(&self) -> &Type {
        match self.is_option() {
            Ok(ty) => ty,
            Err(e) => &self.field.ty,
        }
    }

    fn assert_impl_duck_value_type(&self) -> syn::Result<TokenStream2>{
        let ty = self.type_or_through_option();
        Ok(quote! {
            easy_duckdb_extension::duck_value_type_convertor::assert_impl_duck_value_type::<#ty>()
        })
    }
    fn logical_type(&self)-> syn::Result<TokenStream2>{
        let ty = self.type_or_through_option().to_owned();
        let name = self.require_field_name()?;
        Ok(quote! {
            (#name, #ty::logical_type())
        })
    }
}

fn get_option_inner(ty: &Type) -> (bool, &Type) {
    get_type_inner(ty, "Option")
}

fn get_type_inner<'a>(ty: &'a Type, name: &str) -> (bool, &'a Type) {
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(v) = segments.iter().next() {
            if v.ident == name {
                let t = match &v.arguments {
                    PathArguments::AngleBracketed(a) => match a.args.iter().next() {
                        Some(GenericArgument::Type(t)) => {
                            return (true, t);
                        }
                        _ => {}
                    },
                    (_) => {}
                };
            }
        }
    }
    (false, ty)
}
