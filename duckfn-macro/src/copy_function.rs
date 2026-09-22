//! `#[duck_copy_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_copy_function]`.

use crate::common::{DuckDocArgs, DuckDocArgsProvider, ItemFnWrapper, handle_duck_function};
use crate::macro_utils::TokenStream2Result;
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::__private::TokenStream2;
use syn::Type;

/// `#[duck_copy_function(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_copy_function(...)]` accepts. Each attribute macro declares only the keys
/// it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckCopyFunctionArgs {
    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成 builder，交给
    /// `#[duck_custom_register]` 手动注册。
    pub(crate) auto_register: Option<bool>,

    /// 文档参数：`description` / `comment` / `example`（`examples`）。
    ///
    /// Documentation arguments: `description` / `comment` / `example` (`examples`).
    #[darling(flatten)]
    pub(crate) doc: DuckDocArgs,
}

/// 让公共代码拿到 `#[duck_copy_function]` 的文档参数。
///
/// Hands `#[duck_copy_function]`'s documentation arguments to the shared code.
impl DuckDocArgsProvider for DuckCopyFunctionArgs {
    fn duck_doc(&self) -> DuckDocArgs {
        self.doc.clone()
    }
}

/// `#[duck_copy_function]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_copy_function]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(attr, item, |wrapper: ItemFnWrapper<DuckCopyFunctionArgs>| {
        wrapper.build_copy_function()
    })
}

impl ItemFnWrapper<DuckCopyFunctionArgs> {
    /// 生成 COPY 函数（`COPY ... TO (FORMAT xxx)`）：同名模块 + `CopyFunctionImpl` + builder
    /// 导出 + 自动注册（可按参数关闭）。
    ///
    /// 函数签名固定为「writer + 行批」两参，顺序可互换：
    ///
    /// ```ignore
    /// #[duck_copy_function]
    /// fn my_copy(writer: &mut MyWriter, rows: &[DuckDynamicRow]) -> DuckResult<()> { ... }
    /// ```
    ///
    /// 生成的模块导出 `copy_function_builder()` 与 `copy_function_register(connection)`，函数名
    /// 即 `COPY ... (FORMAT <函数名>)` 里的格式名。
    ///
    /// Generates the copy function (`COPY ... TO (FORMAT xxx)`): a same-named module,
    /// `CopyFunctionImpl`, the builder exports and the automatic registration (which can be
    /// disabled by arguments). The signature is fixed to "writer + rows" (in either order). The
    /// generated module exports `copy_function_builder()` and `copy_function_register(connection)`,
    /// and the function name is the format name used by `COPY ... (FORMAT <function name>)`.
    pub(crate) fn build_copy_function(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let item_fn = &self.item_fn;
        let (writer_type, call_args) = self.copy_to_signature()?;
        let function_register = self.copy_function_register()?;
        let doc_submit = self.doc_inventory_submit(&name.to_string())?;

        Ok(quote! {
            #item_fn

            #vis mod #name {
                use super::*;

                pub struct CopyFunctionImpl;

                impl duckfn::CopyToFunctionAdapter for CopyFunctionImpl {
                    const NAME: &'static str = stringify!(#name);
                    type Writer = #writer_type;

                    fn write_rows(
                        writer: &mut Self::Writer,
                        rows: &[duckfn::DuckDynamicRow],
                    ) -> duckfn::DuckResult<()> {
                        #name(#(#call_args),*)
                    }
                }

                /// 只生成 builder、不自动注册时使用。
                ///
                /// Used when the builder is generated without automatic registration.
                pub fn copy_function_builder()
                    -> duckfn::DuckResult<quack_rs::prelude::CopyFunctionBuilder>
                {
                    use duckfn::CopyToFunctionAdapter;
                    CopyFunctionImpl::copy_function_builder()
                }

                /// 手动注册入口：在 `#[duck_custom_register]` 里调用。
                ///
                /// Manual registration entry point: call it inside `#[duck_custom_register]`.
                pub fn copy_function_register(
                    c: &duckfn::Connection,
                ) -> duckfn::DuckResult<()> {
                    use duckfn::CopyToFunctionAdapter;
                    unsafe { CopyFunctionImpl::register(c) }
                }

                #function_register
            }

            #doc_submit
        })
    }

    /// 生成 COPY 函数的注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the copy-function registration; produces nothing when `auto_register = false`.
    fn copy_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.inventory_submit(quote! {
            copy_function_register(c)
        })
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// 解析 COPY TO 的签名：恰好两个参数 —— 一个 `&mut Writer` 和一个 `&[DuckDynamicRow]` ——
    /// 且返回类型是 `DuckResult<...>`。
    ///
    /// 返回 writer 类型与调用被标注函数时的实参序列（writer 位置传 `writer`，行批位置传 `rows`，
    /// 因此参数顺序怎么写都行）。
    ///
    /// Parses the COPY TO signature: exactly two parameters — one `&mut Writer` and one
    /// `&[DuckDynamicRow]` — plus a `DuckResult<...>` return type. It returns the writer type and the
    /// argument sequence used to call the annotated function (`writer` at the writer position and
    /// `rows` at the batch position, so the two may be written in either order).
    fn copy_to_signature(&self) -> syn::Result<(Type, Vec<TokenStream2>)> {
        let args = self.args();
        if args.len() != COPY_SIGNATURE_ARITY {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                COPY_TO_SIGNATURE_HINT,
            ));
        }

        let mut writer_type: Option<Type> = None;
        let mut rows_count = 0usize;
        let mut call_args: Vec<TokenStream2> = Vec::with_capacity(args.len());
        for arg in &args {
            if arg.is_agg_state() {
                // 第二个 `&mut` 参数没有意义：writer 只能有一个。
                if writer_type.is_some() {
                    return Err(syn::Error::new_spanned(
                        &self.item_fn.sig.inputs,
                        COPY_TO_SIGNATURE_HINT,
                    ));
                }
                writer_type = Some(arg.resolve_state_type()?.clone());
                call_args.push(quote! { writer });
            } else if is_dynamic_rows_ref(arg.resolve_type()?) {
                rows_count += 1;
                call_args.push(quote! { rows });
            } else {
                return Err(syn::Error::new_spanned(
                    &self.item_fn.sig.inputs,
                    COPY_TO_SIGNATURE_HINT,
                ));
            }
        }

        if writer_type.is_none() || rows_count != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                COPY_TO_SIGNATURE_HINT,
            ));
        }

        self.copy_to_return_type()?;
        Ok((writer_type.expect("checked above"), call_args))
    }

    /// 校验 COPY TO 的返回类型是 `DuckResult<...>`：sink 阶段出错要能让整条 `COPY` 失败。
    ///
    /// Validates that the copy function returns `DuckResult<...>`: a sink-phase error must be able
    /// to fail the whole `COPY`.
    fn copy_to_return_type(&self) -> syn::Result<()> {
        if self.duck_result_inner().is_some() {
            return Ok(());
        }
        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            COPY_TO_SIGNATURE_HINT,
        ))
    }
}

/// 判断类型是不是 `&[DuckDynamicRow]`（路径前缀不限，元素必须叫 `DuckDynamicRow`）。
///
/// Whether the type is `&[DuckDynamicRow]` (any path prefix, element named `DuckDynamicRow`).
fn is_dynamic_rows_ref(ty: &Type) -> bool {
    if let Type::Reference(type_ref) = ty {
        if let Type::Slice(slice) = &*type_ref.elem {
            if let Type::Path(element) = &*slice.elem {
                return element
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "DuckDynamicRow");
            }
        }
    }
    false
}

/// COPY 函数签名要求恰好两个参数（writer/reader + 数据）。
///
/// A copy-function signature takes exactly two parameters (writer/reader plus the data).
const COPY_SIGNATURE_ARITY: usize = 2;

const COPY_TO_SIGNATURE_HINT: &str = r#"Only like
    `fn my_copy(writer: &mut MyWriter, rows: &[DuckDynamicRow]) -> DuckResult<()>`: the arguments are the copy writer (a `&mut` type implementing `duckfn::DuckCopyToWriter`) and the batch of dynamic rows to write, in either order, and the return type is `DuckResult<()>`;
is supported"#;
