use crate::macro_utils::{TokenStream2Result, add_colon2_token, extract_option};
use darling::FromDeriveInput;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::__private::TokenStream2;
use syn::spanned::Spanned;
use syn::{
    Data, DataStruct, DeriveInput, Fields, FieldsNamed, GenericArgument, Path, Type, TypePath,
};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(duck))]
struct DuckMacroArgs {
    pub named_param_from: Option<String>,
    pub auto_register: Option<bool>,
}

pub(crate) fn duck_struct_derive(input: DeriveInput) -> TokenStream2Result {
    let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { named, .. }),
        ..
    }) = input.to_owned().data
    else {
        return Err(syn::Error::new(
            input.span(),
            "Only named fields are allowed",
        ));
    };
    let macro_args = DuckMacroArgs::from_derive_input(&input)?;
    let mut start_named_param = false;
    let mut fields: Vec<FieldWrapper> = Vec::new();
    for (index, f) in named.into_iter().enumerate() {
        let mut wrapper = FieldWrapper::new(f, index);
        if start_named_param {
            wrapper.is_named_param = true;
        } else if let Some(named_param_from) = &macro_args.named_param_from {
            if wrapper.require_field_name()?.to_string() == *named_param_from {
                start_named_param = true;
                wrapper.is_named_param = true;
            }
        }
        fields.push(wrapper)
    }
    if macro_args.named_param_from.is_some() && !start_named_param {
        return Err(syn::Error::new(
            input.span(),
            format!(
                "named_param_from field `{}` not found",
                macro_args.named_param_from.as_ref().unwrap()
            ),
        ));
    }
    let context = DuckStructContext {
        input: input.to_owned(),
        fields,
        macro_args,
    };
    context.build_all()
}

struct DuckStructContext {
    input: DeriveInput,
    fields: Vec<FieldWrapper>,
    macro_args: DuckMacroArgs,
}

impl DuckStructContext {
    fn struct_name(&self) -> &syn::Ident {
        &self.input.ident
    }

    fn build_all(&self) -> TokenStream2Result {
        let mut ts = self.build_duck_struct_impl()?;
        Ok(ts)
    }
    fn build_duck_struct_impl(&self) -> TokenStream2Result {
        let struct_name = self.struct_name();
        let assert_impl_duck_value_type =
            self.fields_to_code(|f| f.assert_impl_duck_value_type())?;
        let named_columns_type_fn = self.fields_to_code(|f| f.s_named_columns_type_fn())?;
        let named_param_from = self.s_named_param_from()?;
        let child_readers = self.fields_to_code(|f| f.s_child_readers())?;
        let read_valid = self.fields_to_code(|f| f.read_valid())?;
        let read_duck_values = self.fields_to_code(|f| f.s_read_duck_values())?;
        let write_columns_batch = self.fields_to_code(|f| f.s_write_columns_batch())?;
        let create_writer_batch = self.fields_to_code(|f| f.s_create_writer_batch())?;
        let write_valid = self.fields_to_code(|f| f.s_write_valid())?;
        let write_finish = self.fields_to_code(|f| f.write_finish())?;

        Ok(quote! {
            impl ::duckfn::DuckStructTrait for #struct_name {
                fn s_named_columns_type_fn() -> &'static [(&'static str, fn() -> quack_rs::prelude::LogicalType)] {
                    use duckfn::DuckValueType;
                    #(#assert_impl_duck_value_type;)*
                    &[
                        #(#named_columns_type_fn),*
                    ]
                }

                fn s_named_param_from() -> Option<String> {
                    #named_param_from
                }

                fn s_child_readers(
                    row_count: usize,
                    vectors: Vec<libduckdb_sys::duckdb_vector>,
                ) -> Vec<duckfn::DuckValueReader> {
                    use duckfn::DuckValueType;
                    Vec::from([
                        #(#child_readers),*
                    ])
                }

                fn s_read_columns(readers: &[duckfn::DuckValueReader], row: usize) -> Option<Self> {
                    use duckfn::DuckValueType;
                    Some(Self {
                        #(#read_valid),*
                    })
                }

                fn s_read_duck_values(
                    values: &Vec<Option<&quack_rs::value::Value>>,
                ) -> duckfn::DuckResult<Self> {
                    use duckfn::DuckValueType;
                    Ok(Self {
                        #(#read_duck_values),*
                    })
                }

                fn s_write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>) {
                    #(#write_columns_batch;)*
                }


                fn s_create_writer_batch(
                    struct_writer: &duckfn::DuckValueWriter,
                    output_vec: &[Option<&Self>],
                ) -> Vec<duckfn::DuckValueWriter> {
                    Vec::from([
                        #(#create_writer_batch),*
                    ])
                }

                fn s_write_valid(writer: &mut duckfn::DuckValueWriter, row: usize, v: &Self) {
                    #(#write_valid;)*
                }

                fn s_write_finish(writer: &mut ::duckfn::DuckValueWriter) {
                    use duckfn::DuckValueType;
                    #(#write_finish;)*
                }

            }
        })
    }

    fn s_named_param_from(&self) -> TokenStream2Result {
        if let Some(name) = self.macro_args.named_param_from.as_ref() {
            Ok(quote! {
                Some(#name.to_string())
            })
        } else {
            Ok(quote! {
                None
            })
        }
    }



    fn fields_to_code(
        &self,
        x: fn(&FieldWrapper) -> TokenStream2Result,
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
    is_named_param: bool,
}
impl FieldWrapper {

    fn s_write_columns_batch(&self) -> TokenStream2Result {
        let index = self.index;
        let get_data = self.get_option_data()?;
        // Self::s_write_column_batch(chunk, row, 0, |v| Some(&v.count));
        Ok(quote! {
            Self::s_write_column_batch(chunk, row, #index, |v|#get_data)
        })
    }
    fn s_create_writer_batch(&self) -> TokenStream2Result {
        let index = self.index;
        let get_data = self.get_option_data()?;
        // Self::s_create_field_writer_batch(struct_writer, 0, output_vec, |v| Some(&v.count)),
        Ok(quote! {
            Self::s_create_field_writer_batch(struct_writer, #index, output_vec, |v|#get_data)
        })
    }
    fn s_write_valid(&self) -> TokenStream2Result {
        let index = self.index;
        let get_data = self.get_option_data()?;
        // Self::s_write_field(writer, row, 0, Some(&v.count));
        Ok(quote! {
            Self::s_write_field(writer, row, #index, #get_data)
        })
    }
    // fn init(&mut self) -> &mut FieldWrapper {
    //     add_colon2_token(&mut self.field.ty);
    //     self
    fn get_option_data(&self) -> TokenStream2Result {
        let field_name = self.require_field_name()?;
        Ok(if self.is_option() {
            quote! {  v.#field_name.as_ref() }
        } else {
            quote! { Some(&v.#field_name) }
        })
    }
    fn s_read_duck_values(&self) -> TokenStream2Result {
        let id = self.require_field_name()?;
        let name = id.to_string();
        let index = self.index;
        if self.is_option() {
            Ok(quote! {
               #id: Self::s_read_by_duck_value_option(values[#index])?
            })
        } else {
            Ok(quote! {
               #id: Self::s_read_by_duck_value_notnull(values[#index], #name)?
            })
        }
    }
    fn s_child_readers(&self) -> TokenStream2Result {
        // i64::create_reader_from_vector(vectors[0], row_count),
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            #ty::create_reader_from_vector(vectors[#index], row_count)
        })
    }

    fn s_named_columns_type_fn(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let name = self.require_field_name()?.to_string();
        Ok(quote! {
            // ("count", i64::logical_type)
            (#name, #ty::logical_type)
        })
    }

    fn new(field: syn::Field, index: usize) -> FieldWrapper {
        let mut wrapper = FieldWrapper {
            field,
            index,
            is_named_param: false,
        };
        // wrapper.init();
        wrapper
    }

    // }
    fn field_name(&self) -> &Option<Ident> {
        &self.field.ident
    }
    fn require_field_name(&self) -> syn::Result<&Ident> {
        self.field_name()
            .as_ref()
            .ok_or(syn::Error::new(self.field.span(), "Field name is required"))
    }

    fn extract_option(&self) -> Option<&Type> {
        // extract_option::from_ref(&self.field.ty)
        extract_option(&self.field.ty)
    }

    /// 穿透Option的类型
    /// - 如果是Option类型，则返回Option内部的类型
    /// - 如果不是Option类型，则返回当前类型
    /// - 只支持单层Option
    /// - 给泛型加上::，例如Vec<T> -> Vec::<T>
    fn duck_value_type(&self) -> Type {
        let x = if let Some(ty) = self.extract_option() {
            ty
        } else {
            &self.field.ty
        };
        let mut x1 = x.to_owned();
        add_colon2_token(&mut x1);
        x1
    }
    fn assert_impl_duck_value_type(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        Ok(quote! {
            ::duckfn::assert_impl_duck_value_type::<#ty>()
        })
    }


    fn read_valid(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let field_name = self.require_field_name()?;
        let index = self.index;
        let try_op = if self.is_option() {
            quote!()
        } else {
            quote!(?)
        };

        Ok(quote! {
            #field_name: #ty::read(&readers[#index], row) #try_op
        })
    }

    fn is_option(&self) -> bool {
        self.extract_option().is_some()
    }


    fn write_finish(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            #ty::write_finish(&mut writer.child_writer[#index])
        })
    }

}
