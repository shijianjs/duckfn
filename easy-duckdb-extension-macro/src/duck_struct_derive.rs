use crate::macro_utils::{TokenStream2Result, add_colon2_token, extract_option};
use darling::FromDeriveInput;
use proc_macro2::Ident;
use quote::quote;
use syn::__private::TokenStream2;
use syn::spanned::Spanned;
use syn::{
    Data, DataStruct, DeriveInput, Fields, FieldsNamed, GenericArgument, Path, Type, TypePath,
};
use syn_match::path_match;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(duck))]
struct DuckMacroArgs {
    pub named_param_from: Option<String>,
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
        let mut ts = self.build_duck_value_type_impl()?;
        ts.extend(self.build_duck_columns_impl()?);
        Ok(ts)
    }

    fn build_duck_value_type_impl(&self) -> TokenStream2Result {
        let struct_name = self.struct_name();
        let logical_types = self.fields_to_code(|f| f.name_type_pair())?;
        let field_readers = self.fields_to_code(|f| f.field_reader())?;
        let assert_impl_duck_value_type =
            self.fields_to_code(|f| f.assert_impl_duck_value_type())?;
        let read_valid = self.fields_to_code(|f| f.read_valid())?;
        let field_writer_batch = self.fields_to_code(|f| f.field_writer_batch())?;
        let write_valid = self.fields_to_code(|f| f.write_valid())?;

        Ok(quote! {
            impl ::easy_duckdb_extension::DuckValueType for #struct_name {
                fn type_id() -> ::quack_rs::prelude::TypeId {
                    ::quack_rs::prelude::TypeId::Struct
                }

                fn logical_type() -> ::quack_rs::prelude::LogicalType {
                    #(#assert_impl_duck_value_type;)*
                    ::quack_rs::prelude::LogicalType::struct_type_from_logical(&Vec::from([
                        #(#logical_types),*
                    ]))
                }

                fn create_reader_from_vector(vector: ::libduckdb_sys::duckdb_vector, size: usize) -> ::easy_duckdb_extension::DuckValueReader {
                    let mut reader = ::easy_duckdb_extension::DuckValueReader::new_from_vector(vector, size);
                    reader.child_reader = Vec::from([
                        #(#field_readers),*
                    ]);
                    reader
                }

                fn read_valid(reader: &::easy_duckdb_extension::DuckValueReader, row: usize) -> Option<Self> {
                    let readers = &reader.child_reader;
                    Some(Self {
                        #(#read_valid)*
                    })
                }

                fn create_writer_batch(vector: ::libduckdb_sys::duckdb_vector, output_vec: &[Option<&Self>]) -> ::easy_duckdb_extension::DuckValueWriter {
                    let mut writer = ::easy_duckdb_extension::DuckValueWriter::new_from_vector(vector);

                    writer.child_writer = Vec::from([
                        #(#field_writer_batch),*
                    ]);

                    writer
                }

                fn write_valid(writer: &mut ::easy_duckdb_extension::DuckValueWriter, idx: usize, vo: &Self) {
                    #(#write_valid)*
                }
            }
        })
    }

    fn build_duck_columns_impl(&self) -> TokenStream2Result {
        let struct_name = self.struct_name();
        let read_valid = self.fields_to_code(|f| f.read_valid())?;
        let name_type_pair = self.fields_to_code(|f| f.name_type_pair())?;
        let reader_by_trunk = self.fields_to_code(|f| f.reader_by_trunk())?;
        let write_columns_batch = self.fields_to_code(|f| f.write_columns_batch())?;
        Ok(quote! {
            impl ::easy_duckdb_extension::DuckColumns for #struct_name {

                fn create_column_readers(chunk: &quack_rs::data_chunk::DataChunk) -> Vec<::easy_duckdb_extension::DuckValueReader> {
                    use easy_duckdb_extension::DuckValueType;

                    Vec::from([
                        #(#reader_by_trunk),*
                    ])
                }
                fn read_columns(readers: &[::easy_duckdb_extension::DuckValueReader], row: usize) -> Option<Self> {
                    use easy_duckdb_extension::DuckValueType;

                    Some(Self {
                        #(#read_valid)*
                    })
                }

                fn named_column_types() -> Vec<(impl Into<String>, ::quack_rs::prelude::LogicalType)> {
                    use easy_duckdb_extension::DuckValueType;

                    Vec::from([
                        #(#name_type_pair),*
                    ])
                }

                fn write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>) {
                    use easy_duckdb_extension::DuckValueType;

                    #(#write_columns_batch)*
                }
            }
        })
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
    fn new(field: syn::Field, index: usize) -> FieldWrapper {
        let mut wrapper = FieldWrapper {
            field,
            index,
            is_named_param: false,
        };
        // wrapper.init();
        wrapper
    }

    // fn init(&mut self) -> &mut FieldWrapper {
    //     add_colon2_token(&mut self.field.ty);
    //     self
    // }

    fn field_name(&self) -> &Option<Ident> {
        &self.field.ident
    }
    fn require_field_name(&self) -> syn::Result<&Ident> {
        self.field
            .ident
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
            ::easy_duckdb_extension::assert_impl_duck_value_type::<#ty>()
        })
    }
    fn name_type_pair(&self) -> TokenStream2Result {
        let ty = self.logical_type()?;
        let name = self.require_field_name()?.to_string();
        Ok(quote! {
            (#name, #ty)
        })
    }
    fn logical_type(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        Ok(quote! {
            #ty::logical_type()
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
    fn field_reader(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            #ty::struct_field_reader(&reader, #index)
        })
    }
    //    fn create_readers(chunk: &quack_rs::data_chunk::DataChunk) -> Vec<crate::DuckValueReader> {
    //         vec![
    //             A::create_reader(chunk, 0),
    //             B::create_reader(chunk, 1)
    //         ]
    //     }
    fn reader_by_trunk(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            #ty::create_reader(chunk, #index)
        })
    }

    //    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
    //         Some(Self {
    //             f0: F0::read(&reader.child_reader[0], row),
    //             f1: F1::read(&reader.child_reader[1], row),
    //             field_names_type: PhantomData,
    //         })
    //     }
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
            #field_name: #ty::read(&readers[#index], row) #try_op,
        })
    }

    fn is_option(&self) -> bool {
        self.extract_option().is_some()
    }

    ///     fn create_writer_batch(vector: libduckdb_sys::duckdb_vector, output_vec: &[Option<&Self>]) -> easy_duckdb_extension::DuckValueWriter {
    //         let mut writer = easy_duckdb_extension::DuckValueWriter::new_from_vector(vector);
    //
    //         writer.child_writer = vec![
    //             F0::struct_field_writer_batch(
    //                 &writer,
    //                 0,
    //                 &output_vec
    //                     .iter()
    //                     .map(|x| x.as_ref().and_then(|v| v.f0.as_ref()))
    //                     .collect::<Vec<_>>(),
    //             ),
    //             F1::struct_field_writer_batch(
    //                 &writer,
    //                 1,
    //                 &output_vec
    //                     .iter()
    //                     .map(|x| x.as_ref().and_then(|v| v.f1.as_ref()))
    //                     .collect::<Vec<_>>(),
    //             ),
    //         ];
    //
    //         writer
    //     }
    fn field_writer_batch(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let field_name = self.require_field_name()?;
        let index = self.index;
        let count_converter = if self.is_option() {
            quote! { |v| v.#field_name.as_ref() }
        } else {
            quote! { |v| Some(&v.#field_name) }
        };
        Ok(quote! {
            #ty::struct_field_writer_batch(&writer, #index, &output_vec
                    .iter()
                    .map(|x| x.as_ref().and_then(#count_converter))
                    .collect::<Vec<_>>())
        })
    }

    //     fn write_valid(writer: &mut easy_duckdb_extension::DuckValueWriter, idx: usize, vo: &Self) {
    //         F0::write(&mut writer.child_writer[0], idx, &vo.f0);
    //         F1::write_valid(&mut writer.child_writer[1], idx, &vo.f1);
    //     }
    fn write_valid(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let field_name = self.require_field_name()?;
        let index = self.index;
        if self.is_option() {
            Ok(quote! {
                #ty::write(&mut writer.child_writer[#index], idx, vo.#field_name.as_ref());
            })
        } else {
            Ok(quote! {
                #ty::write_valid(&mut writer.child_writer[#index], idx, &vo.#field_name);
            })
        }
    }

    //                fn write_columns_batch(chunk: &DataChunk, row: &Vec<Option<&Self>>) {
    //                     A::write_batch(unsafe{ chunk.vector(0) }, &row.iter()
    //                         .map(|r| r.and_then(|r| r.0.as_ref()))
    //                         .collect::<Vec<_>>());
    //                     B::write_batch(unsafe{ chunk.vector(1) }, &row.iter()
    //                         .map(|r| r.map(|r| &r.1))
    //                         .collect::<Vec<_>>());
    //                 }
    fn write_columns_batch(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let field_name = self.require_field_name()?;
        let index = self.index;
        if self.is_option() {
            Ok(quote! {
                #ty::write_batch(unsafe{ chunk.vector(#index) }, &row.iter()
                    .map(|o| o.and_then(|r| r.#field_name.as_ref()))
                    .collect::<Vec<_>>());
            })
        } else {
            Ok(quote! {
                #ty::write_batch(unsafe{ chunk.vector(#index) }, &row.iter()
                    .map(|o| o.map(|r| &r.#field_name))
                    .collect::<Vec<_>>());
            })
        }
    }
}
