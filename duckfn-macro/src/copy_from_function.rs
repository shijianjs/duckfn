//! `#[duck_copy_from_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_copy_from_function]`.

use crate::common::{ItemFnWrapper, handle_duck_function};
use crate::macro_utils::TokenStream2Result;
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::__private::TokenStream2;
use syn::{GenericArgument, PathArguments, Type};

/// `#[duck_copy_from_function(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_copy_from_function(...)]` accepts. Each attribute macro declares only the
/// keys it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckCopyFromFunctionArgs {
    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成 builder，交给
    /// `#[duck_custom_register]` 手动注册。
    pub(crate) auto_register: Option<bool>,
}

/// `#[duck_copy_from_function]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_copy_from_function]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(
        attr,
        item,
        |wrapper: ItemFnWrapper<DuckCopyFromFunctionArgs>| wrapper.build_copy_from_function(),
    )
}

impl ItemFnWrapper<DuckCopyFromFunctionArgs> {
    /// 生成 COPY FROM 的实现体：`CopyFromFunctionAdapter`（行级读取）+ 注册函数。
    ///
    /// 被标注的函数就是「取下一批行」：参数里一个是 `&mut Reader`（读取器状态，需实现
    /// `duckfn::DuckCopyFromReader`），一个是 `limit: usize`（本批最多取多少行），顺序可互换；
    /// 返回 `DuckResult<Vec<DuckDynamicRow>>`，空 `Vec` 表示流结束。
    ///
    /// Generates the COPY FROM implementation: `CopyFromFunctionAdapter` (row-level reading) plus
    /// the registration function. The annotated function is the "take the next batch" step: one
    /// parameter is `&mut Reader` (the reader state, implementing `duckfn::DuckCopyFromReader`) and
    /// another is `limit: usize` (how many rows this batch may hold), in either order, and it
    /// returns `DuckResult<Vec<DuckDynamicRow>>` where an empty `Vec` ends the stream.
    pub(crate) fn build_copy_from_function(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let item_fn = &self.item_fn;
        let (reader_type, call_args) = self.copy_from_signature()?;
        let function_register = self.copy_from_function_register()?;

        Ok(quote! {
            #item_fn

            #vis mod #name {
                use super::*;

                pub struct CopyFromFunctionImpl;

                impl duckfn::CopyFromFunctionAdapter for CopyFromFunctionImpl {
                    const NAME: &'static str = stringify!(#name);
                    type Reader = #reader_type;

                    fn next_batch(
                        reader: &mut Self::Reader,
                        limit: usize,
                    ) -> duckfn::DuckResult<Vec<duckfn::DuckDynamicRow>> {
                        #name(#(#call_args),*)
                    }
                }

                /// 手动注册入口：在 `#[duck_custom_register]` 里调用。
                ///
                /// Manual registration entry point: call it inside `#[duck_custom_register]`.
                pub fn copy_from_register(c: &duckfn::Connection) -> duckfn::DuckResult<()> {
                    use duckfn::CopyFromFunctionAdapter;
                    unsafe { CopyFromFunctionImpl::register(c) }
                }

                #function_register
            }
        })
    }

    /// 生成 COPY FROM 的注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the copy-from registration; produces nothing when `auto_register = false`.
    fn copy_from_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.inventory_submit(quote! {
            copy_from_register(c)
        })
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// 解析 COPY FROM 的签名：恰好两个参数 —— 一个 `&mut Reader` 和一个 `limit: usize` ——
    /// 且返回类型是 `DuckResult<Vec<DuckDynamicRow>>`。
    ///
    /// 返回 reader 类型与调用被标注函数时的实参序列（reader 位置传 `reader`，limit 位置传 `limit`）。
    ///
    /// Parses the COPY FROM signature: exactly two parameters — one `&mut Reader` and one
    /// `limit: usize` — plus a `DuckResult<Vec<DuckDynamicRow>>` return type. It returns the reader
    /// type and the argument sequence used to call the annotated function.
    fn copy_from_signature(&self) -> syn::Result<(Type, Vec<TokenStream2>)> {
        let args = self.args();
        if args.len() != COPY_SIGNATURE_ARITY {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                COPY_FROM_SIGNATURE_HINT,
            ));
        }

        let mut reader_type: Option<Type> = None;
        let mut limit_count = 0usize;
        let mut call_args: Vec<TokenStream2> = Vec::with_capacity(args.len());
        for arg in &args {
            if arg.is_agg_state() {
                // 第二个 `&mut` 参数没有意义：reader 只能有一个。
                if reader_type.is_some() {
                    return Err(syn::Error::new_spanned(
                        &self.item_fn.sig.inputs,
                        COPY_FROM_SIGNATURE_HINT,
                    ));
                }
                reader_type = Some(arg.resolve_state_type()?.clone());
                call_args.push(quote! { reader });
            } else if is_usize_type(arg.resolve_type()?) {
                limit_count += 1;
                call_args.push(quote! { limit });
            } else {
                return Err(syn::Error::new_spanned(
                    &self.item_fn.sig.inputs,
                    COPY_FROM_SIGNATURE_HINT,
                ));
            }
        }

        if reader_type.is_none() || limit_count != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                COPY_FROM_SIGNATURE_HINT,
            ));
        }

        self.copy_from_return_type()?;
        Ok((reader_type.expect("checked above"), call_args))
    }

    /// 校验 COPY FROM 的返回类型是 `DuckResult<Vec<DuckDynamicRow>>`。
    ///
    /// Validates that the COPY FROM function returns `DuckResult<Vec<DuckDynamicRow>>`.
    fn copy_from_return_type(&self) -> syn::Result<()> {
        if let Some(Type::Path(vec_path)) = self.duck_result_inner() {
            if let Some(vec) = vec_path
                .path
                .segments
                .last()
                .filter(|segment| segment.ident == "Vec")
            {
                if let PathArguments::AngleBracketed(generic_args) = &vec.arguments {
                    if let Some(GenericArgument::Type(Type::Path(item))) = generic_args.args.first()
                    {
                        if item
                            .path
                            .segments
                            .last()
                            .is_some_and(|segment| segment.ident == "DuckDynamicRow")
                        {
                            return Ok(());
                        }
                    }
                }
            }
        }
        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            COPY_FROM_SIGNATURE_HINT,
        ))
    }
}

/// 判断类型是不是 `usize`（路径前缀不限）。
///
/// Whether the type is `usize` (any path prefix).
fn is_usize_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "usize"))
}

/// COPY 函数签名要求恰好两个参数（writer/reader + 数据）。
///
/// A copy-function signature takes exactly two parameters (writer/reader plus the data).
const COPY_SIGNATURE_ARITY: usize = 2;

const COPY_FROM_SIGNATURE_HINT: &str = r#"Only like
    `fn my_read(reader: &mut MyReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>>`: the arguments are the reader state (a `&mut` type implementing `duckfn::DuckCopyFromReader`) and the maximum number of rows in this batch, in either order, and the return type is `DuckResult<Vec<DuckDynamicRow>>` (an empty `Vec` ends the stream);
is supported"#;
