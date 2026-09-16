//! `#[derive(DuckStruct)]` 的实现：把具名结构体的每个字段映射成一列/一个 STRUCT 子字段。
//!
//! Implementation of `#[derive(DuckStruct)]`: maps every field of a named struct onto a column /
//! a STRUCT child field.

use crate::attr_args::DuckFunctionMacroArgs;
use crate::macro_utils::{TokenStream2Result, to_snake_case};
use darling::FromDeriveInput;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::__private::TokenStream2;
use syn::spanned::Spanned;
use syn::{Data, DataStruct, DeriveInput, Fields, FieldsNamed, Type};

/// `#[duck(...)]` 属性的解析结果。
///
/// 这里只提供 `FromDeriveInput` 的「外壳」，真正的字段配置全部通过
/// `#[darling(flatten)]` 委托给 [`DuckFunctionMacroArgs`]（[`darling::FromMeta`]）——被函数属性宏
/// 写穿到 `DuckArgsImpl` 上的 `#[duck(...)]` 属性因此在 `DuckArgs` 中只定义一次。
///
/// Parse result of the `#[duck(...)]` attribute. This only provides the `FromDeriveInput`
/// shell; every field configuration is delegated to [`DuckFunctionMacroArgs`] ([`darling::FromMeta`])
/// through `#[darling(flatten)]`, so the `#[duck(...)]` written through onto `DuckArgsImpl` by
/// the attribute macros is declared only once, in `DuckArgs`.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(duck))]
struct DuckDeriveMacroArgs {
    /// `#[duck(...)]` 的字段配置，复用属性宏侧的唯一配置源。
    ///
    /// The field configuration of `#[duck(...)]`, reusing the single source of truth shared with
    /// the attribute macros.
    #[darling(flatten)]
    args: DuckFunctionMacroArgs,
}

/// `#[derive(DuckStruct)]` 的入口。
///
/// 只支持「具名字段的结构体」；解析 `#[duck(...)]` 参数后，从 `named_param_from` 指定的字段
/// 开始把其后的字段标记为命名参数，最后生成 [`DuckStructTrait`] 实现。
///
/// Entry point of `#[derive(DuckStruct)]`. Only structs with named fields are supported. After
/// parsing the `#[duck(...)]` arguments it marks the fields from `named_param_from` onwards as
/// named parameters and finally generates the `DuckStructTrait` implementation.
///
/// # Errors
///
/// 结构体不是具名字段结构体、`named_param_from` 指定的字段不存在时返回编译错误。
///
/// Returns a compile error when the input is not a named-field struct or the field named by
/// `named_param_from` does not exist.
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
    let macro_args = DuckDeriveMacroArgs::from_derive_input(&input)?;
    let mut start_named_param = false;
    let mut fields: Vec<FieldWrapper> = Vec::new();
    for (index, f) in named.into_iter().enumerate() {
        let mut wrapper = FieldWrapper::new(f, index);
        if start_named_param {
            wrapper.is_named_param = true;
        } else if let Some(named_param_from) = &macro_args.args.named_param_from {
            if wrapper.require_field_name()? == named_param_from {
                start_named_param = true;
                wrapper.is_named_param = true;
            }
        }
        fields.push(wrapper)
    }
    if macro_args.args.named_param_from.is_some() && !start_named_param {
        return Err(syn::Error::new(
            input.span(),
            format!(
                "named_param_from field `{}` not found",
                macro_args.args.named_param_from.as_ref().unwrap()
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

/// 代码生成的上下文：原始输入 + 已包装的字段 + 属性参数。
///
/// Code-generation context: the original input, the wrapped fields and the attribute arguments.
struct DuckStructContext {
    /// derive 的原始输入（用于取结构体名、报错位置）。
    ///
    /// The original derive input (used for the struct name and error spans).
    input: DeriveInput,
    /// 按声明顺序排列的字段包装。
    ///
    /// The wrapped fields in declaration order.
    fields: Vec<FieldWrapper>,
    /// 解析后的 `#[duck(...)]` 参数。
    ///
    /// The parsed `#[duck(...)]` arguments.
    macro_args: DuckDeriveMacroArgs,
}

impl DuckStructContext {
    /// 被 derive 的结构体名。
    ///
    /// The name of the derived struct.
    fn struct_name(&self) -> &syn::Ident {
        &self.input.ident
    }

    /// 生成全部代码：`DuckStructTrait` 实现，以及（`create_type = true` 时）加载期建类型的注册。
    ///
    /// Generates all the code: the `DuckStructTrait` implementation plus — for `create_type = true`
    /// — the load-time type registration.
    fn build_all(&self) -> TokenStream2Result {
        let impl_ts = self.build_duck_struct_impl()?;
        let type_registration = self.build_type_registration()?;
        Ok(quote! {
            #impl_ts
            #type_registration
        })
    }

    /// 生成「加载期创建命名 STRUCT 类型」的注册函数与 inventory 提交。
    ///
    /// 只在 `#[duck(create_type = true)]` 时生成：字段类型不在这里拼 SQL —— 注册函数把
    /// `<Self as DuckValueType>::logical_type()`（也就是引擎眼里的那个 STRUCT 逻辑类型）交给
    /// [`duckfn::register_named_type`]，由它递归渲染成 SQL，因此自定义字段类型同样适用。
    ///
    /// Generates the registration function and the inventory submission that create the named
    /// STRUCT type at load time. Only emitted for `#[duck(create_type = true)]`. The field types are
    /// not assembled here: the registrar hands `<Self as DuckValueType>::logical_type()` to
    /// `duckfn::register_named_type`, which renders it recursively — so custom field types work.
    fn build_type_registration(&self) -> TokenStream2Result {
        if !self.macro_args.args.create_type.unwrap_or(false) {
            return Ok(quote!());
        }
        if let Some(param) = self.input.generics.params.first() {
            return Err(syn::Error::new(
                param.span(),
                "`#[duck(create_type = true)]` is not supported on a generic struct: the SQL type \
                 must be concrete",
            ));
        }

        let struct_name = self.struct_name();
        let module_ident = format_ident!("__duck_struct_{}", to_snake_case(&struct_name.to_string()));
        let sql_name = self
            .macro_args
            .args
            .sql_name
            .clone()
            .unwrap_or_else(|| to_snake_case(&struct_name.to_string()));

        Ok(quote! {
            #[allow(non_snake_case)]
            mod #module_ident {
                /// 加载期把 STRUCT 类型建进 catalog（`CREATE TYPE IF NOT EXISTS ...`，幂等）。
                ///
                /// Creates the STRUCT type in the catalog at load time (`CREATE TYPE IF NOT
                /// EXISTS ...`, idempotent).
                pub fn register(connection: &::duckfn::Connection) -> ::duckfn::DuckResult<()> {
                    ::duckfn::register_named_type(
                        connection,
                        #sql_name,
                        <super::#struct_name as ::duckfn::DuckValueType>::logical_type(),
                    )
                }
            }

            ::duckfn::inventory_submit! {
                ::duckfn::DuckFunctionItem {
                    register_fn: #module_ident::register,
                }
            }
        })
    }

    /// 生成 `impl DuckStructTrait for #struct_name`，逐字段拼出各 `s_*` 方法体。
    ///
    /// Generates `impl DuckStructTrait for #struct_name`, assembling every `s_*` method body from
    /// the fields.
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
        let write_null = self.fields_to_code(|f| f.write_null())?;
        let write_finish = self.fields_to_code(|f| f.write_finish())?;

        Ok(quote! {
            impl ::duckfn::DuckStructTrait for #struct_name {
                fn s_named_columns_type_fn() -> &'static [::duckfn::DuckNamedColumnType] {
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
                    values: &[Option<&quack_rs::value::Value>],
                ) -> duckfn::DuckResult<Self> {
                    use duckfn::DuckValueType;
                    Ok(Self {
                        #(#read_duck_values),*
                    })
                }

                fn s_write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &[Option<&Self>]) {
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

                fn s_write_null(writer: &mut duckfn::DuckValueWriter, row: usize) {
                    use duckfn::DuckValueType;
                    unsafe { writer.vector_writer.set_null(row) };
                    #(#write_null;)*
                }

                fn s_write_finish(writer: &mut ::duckfn::DuckValueWriter) {
                    use duckfn::DuckValueType;
                    #(#write_finish;)*
                }

            }
        })
    }

    /// 生成 `s_named_param_from` 的方法体：有配置则返回 `Some("字段名")`，否则返回 `None`。
    ///
    /// Generates the `s_named_param_from` body: `Some("field")` when configured, otherwise `None`.
    fn s_named_param_from(&self) -> TokenStream2Result {
        if let Some(name) = self.macro_args.args.named_param_from.as_ref() {
            Ok(quote! {
                Some(#name.to_string())
            })
        } else {
            Ok(quote! {
                None
            })
        }
    }



    /// 对每个字段调用 `x` 生成代码并收集；任一字段出错则整体失败。
    ///
    /// Calls `x` for every field to generate code and collects the results; a failure on any field
    /// fails the whole list.
    fn fields_to_code(
        &self,
        x: fn(&FieldWrapper) -> TokenStream2Result,
    ) -> syn::Result<Vec<TokenStream2>> {
        self.fields
            .iter()
            .map(x)
            .collect::<syn::Result<Vec<_>>>()
    }
}

/// 单个字段的包装：字段本体 + 下标 + 是否命名参数。
///
/// Wrapper around one field: the field itself, its index and whether it is a named parameter.
struct FieldWrapper {
    /// 被包装的结构体字段。
    ///
    /// The wrapped struct field.
    field: syn::Field,
    /// 字段在结构体中的下标（也即列/子字段下标）。
    ///
    /// The field's index in the struct (also the column / child-field index).
    index: usize,
    /// 是否为表函数的命名参数。
    ///
    /// Whether the field is a table-function named parameter.
    is_named_param: bool,
}
impl FieldWrapper {

    /// 生成 `s_write_columns_batch` 里的单条语句：把本字段批量写入第 `index` 列。
    ///
    /// Generates one statement of `s_write_columns_batch`: writes this field into column `index`
    /// in one batch.
    fn s_write_columns_batch(&self) -> TokenStream2Result {
        let index = self.index;
        let get_data = self.get_option_data()?;
        // Self::s_write_column_batch(chunk, row, 0, |v| Some(&v.count));
        Ok(quote! {
            Self::s_write_column_batch(chunk, row, #index, |v|#get_data)
        })
    }

    /// 生成 `s_create_writer_batch` 里的单条元素：为第 `index` 个子字段创建写入器。
    ///
    /// Generates one element of `s_create_writer_batch`: creates a writer for child field `index`.
    fn s_create_writer_batch(&self) -> TokenStream2Result {
        let index = self.index;
        let get_data = self.get_option_data()?;
        // Self::s_create_field_writer_batch(struct_writer, 0, output_vec, |v| Some(&v.count)),
        Ok(quote! {
            Self::s_create_field_writer_batch(struct_writer, #index, output_vec, |v|#get_data)
        })
    }

    /// 生成 `s_write_valid` 里的单条语句：写入本字段的第 `row` 行。
    ///
    /// Generates one statement of `s_write_valid`: writes row `row` of this field.
    fn s_write_valid(&self) -> TokenStream2Result {
        let index = self.index;
        let get_data = self.get_option_data()?;
        // Self::s_write_field(writer, row, 0, Some(&v.count));
        Ok(quote! {
            Self::s_write_field(writer, row, #index, #get_data)
        })
    }

    /// 生成「从结构体引用取出本字段引用」的表达式：一律 `Some(&v.#field)`。
    ///
    /// 字段类型本身可以是 `Option<T>`，于是得到 `Option<&Option<T>>`；「字段是 NULL」
    /// 由 `Option<T>` 自己的写入实现处理（`write_valid` 收到 `None` 时写 NULL），
    /// 因此这里不再区分可空与否。
    ///
    /// Generates the expression that borrows this field from a struct reference: always
    /// `Some(&v.field)`. A field of type `Option<T>` therefore yields `Option<&Option<T>>` and
    /// "the field is NULL" is handled by `Option<T>`'s own write implementation (`write_valid`
    /// turns `None` into a NULL write), so nullability is not branched on here any more.
    fn get_option_data(&self) -> TokenStream2Result {
        let field_name = self.require_field_name()?;
        Ok(quote! { Some(&v.#field_name) })
    }

    /// 生成 `s_read_duck_values` 里的单条初始化：可空性由字段类型自己决定
    /// （`T` 遇到 NULL 或缺省时报错，`Option<T>` 取到 `None`）。
    ///
    /// Generates one initialiser of `s_read_duck_values`: nullability is decided by the field
    /// type itself (a NULL or missing value is an error for `T` and `None` for `Option<T>`).
    fn s_read_duck_values(&self) -> TokenStream2Result {
        let id = self.require_field_name()?;
        let name = id.to_string();
        let index = self.index;
        let ty = self.duck_value_type();
        Ok(quote! {
           #id: Self::s_read_field_by_duck_value::<#ty>(values[#index], #name)?
        })
    }

    /// 生成 `s_child_readers` 里的单个读取器：`<#ty>::create_reader_from_vector(vectors[#index], row_count)`。
    ///
    /// Generates one reader of `s_child_readers`:
    /// `<#ty>::create_reader_from_vector(vectors[#index], row_count)`.
    fn s_child_readers(&self) -> TokenStream2Result {
        // <i64>::create_reader_from_vector(vectors[0], row_count),
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            <#ty>::create_reader_from_vector(vectors[#index], row_count)
        })
    }

    /// 生成 `s_named_columns_type_fn` 里的单个 `(字段名, 逻辑类型构造函数)` 元素。
    ///
    /// Generates one `(field name, logical-type constructor)` element of
    /// `s_named_columns_type_fn`.
    fn s_named_columns_type_fn(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let name = self.require_field_name()?.to_string();
        Ok(quote! {
            // ("count", <i64>::logical_type)
            (#name, <#ty>::logical_type)
        })
    }

    /// 创建字段包装（`is_named_param` 初始为 `false`，由调用方按需置位）。
    ///
    /// Creates a field wrapper (`is_named_param` starts as `false` and is set by the caller).
    fn new(field: syn::Field, index: usize) -> FieldWrapper {
        // wrapper.init();
        FieldWrapper {
            field,
            index,
            is_named_param: false,
        }
    }

    // }
    /// 字段名（具名字段才有，即 `Option<Ident>`）。
    ///
    /// The field name (present for named fields, hence `Option<Ident>`).
    fn field_name(&self) -> &Option<Ident> {
        &self.field.ident
    }

    /// 要求字段必须有名字，否则返回编译错误。
    ///
    /// Requires the field to have a name, returning a compile error otherwise.
    fn require_field_name(&self) -> syn::Result<&Ident> {
        self.field_name()
            .as_ref()
            .ok_or(syn::Error::new(self.field.span(), "Field name is required"))
    }

    /// 字段的 `DuckValueType` 类型：直接用字段声明的类型。
    ///
    /// 可空字段的类型就是 `Option<T>` —— 它自己实现了 `DuckValueType`（逻辑类型与 `T` 相同、
    /// NULL 语义由 `from_null` 承载），所以这里不再把 `Option` 剥掉。
    ///
    /// 生成代码统一用 `<#ty>::方法(...)` 这种全限定写法，所以数组、元组这类「不是
    /// `Type::Path`」的类型也照样能用（`[T; N]::logical_type` 是解析不了的）。
    ///
    /// The field's `DuckValueType`: the declared type itself. A nullable field is simply
    /// `Option<T>`, which implements `DuckValueType` on its own (same logical type as `T`, NULL
    /// semantics carried by `from_null`), so `Option` is no longer stripped here. Generated code
    /// always uses the fully-qualified form `<#ty>::method(...)`, so types that are not a
    /// `Type::Path` (arrays, tuples) work as well — `[T; N]::logical_type` would not parse.
    fn duck_value_type(&self) -> Type {
        self.field.ty.to_owned()
    }

    /// 生成编译期断言语句，确认字段类型实现了 `DuckValueType`（未实现则在编译期报错）。
    ///
    /// Generates a compile-time assertion that the field type implements `DuckValueType` (an
    /// error is raised at compile time otherwise).
    fn assert_impl_duck_value_type(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        Ok(quote! {
            ::duckfn::assert_impl_duck_value_type::<#ty>()
        })
    }


    /// 生成 `s_read_columns` 里的单个字段初始化：`<#ty>::read_slot` 读一个槽位，
    /// NULL 时由字段类型决定是「取到 `Some(None)`」还是「整体返回 `None`」。
    ///
    /// Generates one field initialiser of `s_read_columns`: `<#ty>::read_slot` reads one slot and
    /// a NULL resolves through the field type — either to a `Some(None)` value or to a `None` that
    /// invalidates the whole row.
    fn read_valid(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let field_name = self.require_field_name()?;
        let index = self.index;

        Ok(quote! {
            #field_name: <#ty>::read_slot(&readers[#index], row)?
        })
    }


    /// 生成 `s_write_null` 里的单条语句：把本字段的子向量也置为 NULL。
    ///
    /// Generates one statement of `s_write_null`: marks this field's child vector NULL as well.
    fn write_null(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            <#ty>::write_null(&mut writer.child_writer[#index], row)
        })
    }

    /// 生成 `s_write_finish` 里的单条语句：递归完成本字段子写入器的收尾（如 LIST/MAP 长度）。
    ///
    /// Generates one statement of `s_write_finish`: recursively finishes this field's child writer
    /// (e.g. the length of a LIST/MAP).
    fn write_finish(&self) -> TokenStream2Result {
        let ty = self.duck_value_type();
        let index = self.index;
        Ok(quote! {
            <#ty>::write_finish(&mut writer.child_writer[#index])
        })
    }

}
