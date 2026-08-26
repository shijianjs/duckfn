use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::__private::TokenStream2;
use syn::spanned::Spanned;
use syn::{
    Data, DataStruct, DeriveInput, Fields, FieldsNamed, GenericArgument, Path, PathArguments, Type,
    TypePath,
};
use crate::macro_utils::add_vec_turbofish;

pub(crate) fn duck_struct_derive(input: DeriveInput) -> syn::Result<TokenStream2> {
    if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = input.to_owned().data
    {
        let fields = named
            .into_iter()
            .enumerate()
            .map(|(index, f)| FieldWrapper::new(f, index))
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
        let logical_types = self.logical_types()?;
        let field_readers = self.fields_to_code(|f| f.field_reader())?;
        let assert_impl_duck_value_type = self.fields_to_code(|f| f.assert_impl_duck_value_type())?;
        Ok(quote! {
            impl easy_duckdb_extension::DuckValueType for #struct_name {
                fn type_id() -> TypeId {
                    TypeId::Struct
                }

                fn logical_type() -> LogicalType {
                    #(#assert_impl_duck_value_type;)*
                    LogicalType::struct_type_from_logical(&vec![
                        #(#logical_types),*
                    ])
                }

                fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> easy_duckdb_extension::DuckValueReader {
                    let mut reader = easy_duckdb_extension::DuckValueReader::new_from_vector(vector, size);
                    reader.child_reader = vec![
                        #(#field_readers),*
                    ];
                    reader
                }

            }
        })
    }

    fn logical_types(&self) -> syn::Result<Vec<TokenStream2>> {
        self.fields_to_code(|f| f.logical_type())
    }

    fn fields_to_code(
        &self,
        x: fn(&FieldWrapper) -> syn::Result<TokenStream2>,
    ) -> syn::Result<Vec<TokenStream2>> {
        self.fields
            .iter()
            .map(x)
            .into_iter()
            .collect::<syn::Result<Vec<_>>>()
    }
}

struct FieldWrapper {
    field: syn::Field,
    index: usize,
}
impl FieldWrapper {

    fn new(field: syn::Field, index: usize) -> FieldWrapper {
        let mut wrapper = FieldWrapper { field, index };
        wrapper.init();
        wrapper
    }

    fn init(&mut self) -> &mut FieldWrapper {
        add_vec_turbofish(&mut self.field.ty);
        self
    }

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

    fn assert_impl_duck_value_type(&self) -> syn::Result<TokenStream2> {
        let ty = self.type_or_through_option();
        Ok(quote! {
            easy_duckdb_extension::assert_impl_duck_value_type::<#ty>()
        })
    }
    fn logical_type(&self) -> syn::Result<TokenStream2> {
        let ty = self.type_or_through_option();
        let name = self.require_field_name()?.to_string();
        Ok(quote! {
            (#name, #ty::logical_type())
        })
    }

    //     fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
    //         let mut reader = DuckValueReader::new_from_vector(vector, size);
    //         reader.child_reader = vec![
    //             F0::struct_field_reader(&reader, 0),
    //             F1::struct_field_reader(&reader, 1),
    //         ];
    //         reader
    //     }
    fn field_reader(&self) -> syn::Result<TokenStream2> {
        let ty = self.type_or_through_option();
        let index = self.index;
        Ok(quote! {
            #ty::struct_field_reader(&reader, #index)
        })
    }
    //    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
    //         Self {
    //             f0: F0::read(&reader.child_reader[0], row),
    //             f1: F1::read(&reader.child_reader[1], row),
    //             field_names_type: PhantomData,
    //         }
    //     }
    fn read_valid(&self, reader: &syn::Ident, row: &syn::Ident) -> syn::Result<TokenStream2> {
        let ty = self.type_or_through_option();
        let field_name = self.require_field_name()?;
        Ok(quote! {
            #ty::read_valid(&#reader.child_reader[#field_name], #row)
        })
    }
}
