//! `#[duck_table_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_table_function]`.

use crate::common::{DuckDocArgs, DuckDocArgsProvider, ItemFnWrapper, handle_duck_function};
use crate::macro_utils::{TokenStream2Result, extract_generic_arg_type, iterator_item_type};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::{GenericArgument, PathArguments, ReturnType, Type};

/// `#[duck_table_function(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_table_function(...)]` accepts. Each attribute macro declares only the
/// keys it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckTableFunctionArgs {
    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成 builder，交给
    /// `#[duck_custom_register]` 手动注册。
    pub(crate) auto_register: Option<bool>,

    /// 表函数的命名参数从哪个开始
    ///
    /// The field name from which table-function named parameters start.
    pub(crate) named_param_from: Option<String>,

    /// `#[duck_table_function(dynamic_columns = true)]`：输出列在 bind 阶段动态确定。
    ///
    /// 开启后函数不再返回行迭代器，而是返回「schema + 行迭代器」的
    /// `duckfn::DuckDynamicTable`（或 `DuckResult<DuckDynamicTable>`）：列名与列类型可以来自
    /// 文件头、字典表、远端 schema 等外部元数据。默认 `false`，保持原有的静态列行为。
    ///
    /// `#[duck_table_function(dynamic_columns = true)]`: the output columns are decided dynamically
    /// during bind. With this on, the function no longer returns a row iterator but a
    /// `duckfn::DuckDynamicTable` (or `DuckResult<DuckDynamicTable>`) carrying "schema + row
    /// iterator": the column names and types may come from external metadata such as a file header,
    /// a dictionary table or a remote schema. Defaults to `false`, keeping the static-column
    /// behaviour.
    pub(crate) dynamic_columns: Option<bool>,

    /// 文档参数：`description` / `comment` / `example`（`examples`）。
    ///
    /// Documentation arguments: `description` / `comment` / `example` (`examples`).
    #[darling(flatten)]
    pub(crate) doc: DuckDocArgs,
}

/// 让公共代码拿到 `#[duck_table_function]` 的文档参数。
///
/// Hands `#[duck_table_function]`'s documentation arguments to the shared code.
impl DuckDocArgsProvider for DuckTableFunctionArgs {
    fn duck_doc(&self) -> DuckDocArgs {
        self.doc.clone()
    }
}

/// `#[duck_table_function]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_table_function]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(attr, item, |wrapper: ItemFnWrapper<DuckTableFunctionArgs>| {
        wrapper.build_table_function()
    })
}

impl ItemFnWrapper<DuckTableFunctionArgs> {
    /// 生成表函数：同名模块 + `TableFunctionImpl` + 自动注册（可按参数关闭）。
    ///
    /// `named_param_from` 会作为 `#[duck(named_param_from = "...")]` 写到生成的 `DuckArgsImpl` 上，
    /// 这是本宏唯一需要透传给 `#[derive(DuckStruct)]` 的键。
    ///
    /// Generates the table function: a same-named module, `TableFunctionImpl` and automatic
    /// registration (which can be disabled by arguments). `named_param_from` is written as
    /// `#[duck(named_param_from = "...")]` on the generated `DuckArgsImpl` — the only key this
    /// macro has to forward to `#[derive(DuckStruct)]`.
    pub(crate) fn build_table_function(&self) -> TokenStream2Result {
        let sql_name = self.sql_name(None);
        let duck_function_impl = self.build_table_function_impl()?;
        self.common_build(
            &self.args(),
            self.args.named_param_from.as_deref(),
            &sql_name,
            duck_function_impl,
        )
    }

    /// 生成表函数实现体：`TableFunctionImpl`（返回行迭代器）+ builder 导出 + 自动注册。
    ///
    /// Generates the table-function implementation: `TableFunctionImpl` (returning the row
    /// iterator), the builder export and the automatic registration.
    fn build_table_function_impl(&self) -> TokenStream2Result {
        if self.dynamic_columns() {
            return self.build_dynamic_table_function_impl();
        }
        let name = self.name();

        let (_, return_type) = self.table_return_type()?;
        let return_clause = self.build_table_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;
        let function_register = self.table_function_register()?;

        Ok(quote! {
            use duckfn::TableFunctionAdapter;

            pub struct TableFunctionImpl;

            impl duckfn::TableFunctionAdapter for TableFunctionImpl {
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #return_type;

                fn init_data_iterator(
                    args: Self::Args,
                ) -> duckfn::DuckFullIteratorResult<Self::Output> {
                    let result = #name(
                        #(#get_data),*
                    );
                    #return_clause
                }
            }

            pub fn table_function_builder() -> duckfn::DuckResult<quack_rs::prelude::TableFunctionBuilder> {
                TableFunctionImpl::table_function_builder()
            }

            #function_register

        })
    }

    /// 生成动态列表函数实现体：`DynamicTableFunctionAdapter`（返回「schema + 行迭代器」）+
    /// builder 导出 + 自动注册。
    ///
    /// 与静态列表函数同构：`bind` 阶段解析参数、调用被标注函数拿到 `DuckDynamicTable`，把它拆成
    /// schema（用来声明结果列）与行迭代器（作为 scan 状态）；scan 阶段按 schema 批量写行。
    ///
    /// Generates the dynamic-column table-function implementation:
    /// `DynamicTableFunctionAdapter` (returning "schema + row iterator"), the builder export and the
    /// automatic registration. It mirrors the static flavour: bind parses the arguments and calls
    /// the annotated function to get a `DuckDynamicTable`, splitting it into a schema (used to
    /// declare the result columns) and the row iterator (the scan state); scan writes rows out by
    /// that schema.
    fn build_dynamic_table_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let return_clause = self.build_dynamic_table_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;
        let function_register = self.table_function_register()?;

        Ok(quote! {
            use duckfn::DynamicTableFunctionAdapter;

            pub struct TableFunctionImpl;

            impl duckfn::DynamicTableFunctionAdapter for TableFunctionImpl {
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;

                fn bind(
                    args: Self::Args,
                ) -> duckfn::DuckResult<duckfn::DuckDynamicTable> {
                    let result = #name(
                        #(#get_data),*
                    );
                    #return_clause
                }
            }

            pub fn table_function_builder() -> duckfn::DuckResult<quack_rs::prelude::TableFunctionBuilder> {
                TableFunctionImpl::table_function_builder()
            }

            #function_register

        })
    }

    /// 按动态表函数返回类型生成收尾代码，统一收敛成 `DuckResult<DuckDynamicTable>`。
    ///
    /// - `-> DuckDynamicTable`：直接 `Ok(result)`；
    /// - `-> DuckResult<DuckDynamicTable>`：原样返回。
    ///
    /// Generates the epilogue for a dynamic table-function return type, normalising it into
    /// `DuckResult<DuckDynamicTable>`: `-> DuckDynamicTable` becomes `Ok(result)` while
    /// `-> DuckResult<DuckDynamicTable>` is returned as is.
    fn build_dynamic_table_return_clause(&self) -> TokenStream2Result {
        match self.dynamic_table_return_type()? {
            DuckDynamicTableResult::Plain => Ok(quote! { Ok(result) }),
            DuckDynamicTableResult::Result => Ok(quote! { result }),
        }
    }

    /// 解析动态表函数的返回类型：只接受 `-> DuckDynamicTable` 与
    /// `-> DuckResult<DuckDynamicTable>`，其他形式报编译错误（附上动态模式的用法提示）。
    ///
    /// Parses a dynamic table-function return type: only `-> DuckDynamicTable` and
    /// `-> DuckResult<DuckDynamicTable>` are accepted; anything else is a compile error carrying
    /// the dynamic-mode usage hint.
    fn dynamic_table_return_type(&self) -> syn::Result<DuckDynamicTableResult> {
        if let ReturnType::Type(_, ty) = &self.item_fn.sig.output {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "DuckDynamicTable" {
                        return Ok(DuckDynamicTableResult::Plain);
                    }
                    if segment.ident == "DuckResult" {
                        if let Some(Type::Path(inner_path)) = extract_generic_arg_type(segment) {
                            if inner_path
                                .path
                                .segments
                                .last()
                                .is_some_and(|s| s.ident == "DuckDynamicTable")
                            {
                                return Ok(DuckDynamicTableResult::Result);
                            }
                        }
                    }
                }
            }
        }

        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            DYNAMIC_TABLE_RETURN_TYPE_HINT,
        ))
    }

    /// 生成表函数的注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the table-function registration; produces nothing when `auto_register = false`.
    fn table_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.common_inventory_submit(quote! {
            let builder = table_function_builder()?;
            unsafe { c.register_table(builder)}
        })
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// `#[duck_table_function(dynamic_columns = true)]`
    ///
    /// 是否走「bind 阶段动态确定输出列」的通路，默认 `false`（沿用编译期静态列）。
    ///
    /// `#[duck_table_function(dynamic_columns = true)]`: whether to take the "output columns decided
    /// during bind" path; defaults to `false` (the compile-time static columns).
    fn dynamic_columns(&self) -> bool {
        self.args.dynamic_columns.unwrap_or(false)
    }

    /// 解析表函数的返回类型，得到「外层形式 + 行类型」。
    ///
    /// 支持 `impl Iterator<Item = T>`、`DuckResult<impl Iterator<Item = T>>`、
    /// `DuckFullIteratorResult<T>`；其他形式报编译错误。
    ///
    /// Parses a table-function return type into "outer form + row type". `impl Iterator<Item = T>`,
    /// `DuckResult<impl Iterator<Item = T>>` and `DuckFullIteratorResult<T>` are supported;
    /// anything else is a compile error.
    fn table_return_type(&self) -> syn::Result<(DuckTableResult, &Type)> {
        if let ReturnType::Type(_, ty) = self.return_type() {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "DuckFullIteratorResult" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                                return Ok((DuckTableResult::Full, inner));
                            }
                        }
                    }

                    if segment.ident == "DuckResult" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(GenericArgument::Type(Type::ImplTrait(impl_trait))) =
                                args.args.first()
                            {
                                if let Some(item) = iterator_item_type(impl_trait) {
                                    return Ok((DuckTableResult::ResultIterator, item));
                                }
                            }
                        }
                    }
                }
            }

            if let Type::ImplTrait(impl_trait) = &**ty {
                if let Some(item) = iterator_item_type(impl_trait) {
                    return Ok((DuckTableResult::SimpleIterator, item));
                }
            }
        }

        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            "Only like
                `-> impl Iterator<Item = SomeDuckStruct>`: simple;
                `-> DuckResult<impl Iterator<Item = SomeDuckStruct>>`: handle input err;
                `-> DuckFullIteratorResult<SomeDuckStruct>`: handle input err and output row err;
            is supported",
        ))
    }

    /// 按表函数返回类型生成收尾代码，统一收敛成 `DuckFullIteratorResult<Output>`。
    ///
    /// Generates the epilogue for a table-function return type, normalising it into
    /// `DuckFullIteratorResult<Output>`.
    fn build_table_return_clause(&self) -> TokenStream2Result {
        let (result_type, _) = self.table_return_type()?;
        match result_type {
            DuckTableResult::Full => Ok(quote! { result }),
            DuckTableResult::ResultIterator => Ok(quote! { Ok(Box::new(result?.map(|x| Ok(Some(x))))) }),
            DuckTableResult::SimpleIterator => Ok(quote! { Ok(Box::new(result.map(|x| Ok(Some(x))))) }),
        }
    }
}

/// 动态列表函数返回类型的外层形式。
///
/// The outer form of a dynamic-column table-function return type.
#[derive(Clone, Copy)]
enum DuckDynamicTableResult {
    /// `-> DuckDynamicTable`：直接把结果包成 `Ok`。
    ///
    /// `-> DuckDynamicTable`: the result is wrapped in `Ok`.
    Plain,
    /// `-> DuckResult<DuckDynamicTable>`：原样返回。
    ///
    /// `-> DuckResult<DuckDynamicTable>`: returned as is.
    Result,
}

const DYNAMIC_TABLE_RETURN_TYPE_HINT: &str = r#"With `#[duck_table_function(dynamic_columns = true)]` the function must build the result table
    (schema + row iterator) itself, so only these return types are supported:
    `-> DuckDynamicTable`: the schema and rows are produced directly;
    `-> DuckResult<DuckDynamicTable>`: same, but the bind step may fail;
    Build the schema with `duckfn::DuckResultSchema` and the rows with
    `duckfn::DuckDynamicRow` / `duckfn::DuckDynamicValue`, then return
    `duckfn::DuckDynamicTable::new(schema, Box::new(rows))`."#;

/// 表函数返回类型的外层形式。
///
/// The outer form of a table-function return type.
#[derive(Clone, Copy)]
enum DuckTableResult {
    /// `-> DuckFullIteratorResult<T>`：构建与逐行都可能出错。
    ///
    /// `-> DuckFullIteratorResult<T>`: construction and every row may fail.
    Full,
    /// `-> DuckResult<impl Iterator<Item = T>>`：构建可能出错。
    ///
    /// `-> DuckResult<impl Iterator<Item = T>>`: construction may fail.
    ResultIterator,
    /// `-> impl Iterator<Item = T>`：最简单，不出错。
    ///
    /// `-> impl Iterator<Item = T>`: simplest, cannot fail.
    SimpleIterator,
}
