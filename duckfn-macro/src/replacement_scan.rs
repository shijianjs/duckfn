//! `#[duck_replacement_scan]` 的代码生成实现。
//!
//! Code generation behind `#[duck_replacement_scan]`.

use crate::common::{ItemFnWrapper, handle_duck_function};
use crate::macro_utils::{TokenStream2Result, extract_generic_arg_type};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::{ReturnType, Type};

/// `#[duck_replacement_scan(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_replacement_scan(...)]` accepts. Each attribute macro declares only the
/// keys it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckReplacementScanArgs {
    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成注册函数，交给
    /// `#[duck_custom_register]` 手动注册。
    pub(crate) auto_register: Option<bool>,
}

/// `#[duck_replacement_scan]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_replacement_scan]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(
        attr,
        item,
        |wrapper: ItemFnWrapper<DuckReplacementScanArgs>| wrapper.build_replacement_scan(),
    )
}

impl ItemFnWrapper<DuckReplacementScanArgs> {
    /// `#[duck_replacement_scan]`：把 `fn(path: &str) -> Option<String> / DuckOptionResult<String>`
    /// 变成一个 replacement scan 回调。
    ///
    /// 生成的同名模块里导出 `ReplacementScanImpl`（实现 `duckfn::ReplacementScanAdapter`）
    /// 和 `replacement_scan_register`，供 `#[duck_custom_register]` 手动注册使用。
    ///
    /// `#[duck_replacement_scan]`: turns
    /// `fn(path: &str) -> Option<String> / DuckOptionResult<String>` into a replacement-scan
    /// callback. The generated same-named module exports `ReplacementScanImpl` (implementing
    /// `duckfn::ReplacementScanAdapter`) and `replacement_scan_register` for manual registration
    /// via `#[duck_custom_register]`.
    pub(crate) fn build_replacement_scan(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let item_fn = &self.item_fn;
        let path_expr = self.replacement_scan_path_expr()?;
        let return_clause = self.build_replacement_scan_return_clause()?;
        let function_register = self.replacement_scan_register()?;

        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;

                pub struct ReplacementScanImpl;

                impl duckfn::ReplacementScanAdapter for ReplacementScanImpl{
                    const NAME: &'static str = stringify!(#name);

                    fn handle_path(path: &str) -> duckfn::DuckOptionResult<String> {
                        let result = #name(#path_expr);
                        #return_clause
                    }
                }

                /// 手动注册入口：在 `#[duck_custom_register]` 里调用。
                ///
                /// Manual registration entry point: call it inside `#[duck_custom_register]`.
                pub fn replacement_scan_register(
                    c: &quack_rs::prelude::Connection,
                ) -> duckfn::DuckResult<()> {
                    use duckfn::ReplacementScanAdapter;
                    ReplacementScanImpl::register(c)
                }

                #function_register
            }
        })
    }

    /// 生成 replacement scan 的自动注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the automatic registration for a replacement scan; produces nothing when
    /// `auto_register = false`.
    fn replacement_scan_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.inventory_submit(quote! {
            use duckfn::ReplacementScanAdapter;
            ReplacementScanImpl::register(c)
        })
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// 回调函数的唯一参数就是 DuckDB 传进来的表名/路径。
    ///
    /// 入参是引用（`&str`）时直接透传，是 `String` 时复制一份。
    ///
    /// The callback's single parameter is the table name/path passed in by DuckDB. A reference
    /// (`&str`) is forwarded as is, a `String` is copied.
    fn replacement_scan_path_expr(&self) -> TokenStream2Result {
        let args = self.args();
        if args.len() != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                REPLACEMENT_SCAN_SIGNATURE_HINT,
            ));
        }
        match args[0].resolve_type()? {
            // `path: &str`：直接透传
            Type::Reference(_) => Ok(quote! { path }),
            // `path: String`：复制一份
            _ => Ok(quote! { path.to_string() }),
        }
    }

    /// 按「可空性 + 字符串种类」四种组合生成 replacement scan 的返回收尾代码，
    /// 统一把结果收敛成 `DuckOptionResult<String>`。
    ///
    /// Generates the return epilogue of a replacement scan for the four combinations of
    /// nullability and string kind, always normalising the result into
    /// `DuckOptionResult<String>`.
    fn build_replacement_scan_return_clause(&self) -> TokenStream2Result {
        let (result_type, str_kind) = self.replacement_scan_return_type()?;
        Ok(match (result_type, str_kind) {
            (DuckReplacementScanResult::Option, DuckStrKind::Owned) => quote! { Ok(result) },
            (DuckReplacementScanResult::Option, DuckStrKind::Borrowed) => {
                quote! { Ok(result.map(::std::string::ToString::to_string)) }
            }
            (DuckReplacementScanResult::OptionResult, DuckStrKind::Owned) => quote! { result },
            (DuckReplacementScanResult::OptionResult, DuckStrKind::Borrowed) => {
                quote! { Ok(result?.map(::std::string::ToString::to_string)) }
            }
        })
    }

    /// 解析 replacement scan 的返回类型，得到「可空性 + 字符串种类」。
    ///
    /// 只接受 `Option<..>` 与 `DuckOptionResult<..>` 两种外层形式，内层必须是
    /// `String` / `&str`；否则报编译错误。
    ///
    /// Parses the replacement-scan return type into "nullability + string kind". Only
    /// `Option<..>` and `DuckOptionResult<..>` are accepted as the outer form and the inner type
    /// must be `String` / `&str`; otherwise a compile error is reported.
    fn replacement_scan_return_type(
        &self,
    ) -> syn::Result<(DuckReplacementScanResult, DuckStrKind)> {
        let ReturnType::Type(_, ty) = &self.item_fn.sig.output else {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.output,
                REPLACEMENT_SCAN_RETURN_TYPE_HINT,
            ));
        };

        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                let result_type = match segment.ident.to_string().as_str() {
                    "Option" => Some(DuckReplacementScanResult::Option),
                    "DuckOptionResult" => Some(DuckReplacementScanResult::OptionResult),
                    _ => None,
                };
                if let Some(result_type) = result_type {
                    let inner = extract_generic_arg_type(segment).ok_or_else(|| {
                        syn::Error::new_spanned(ty, REPLACEMENT_SCAN_RETURN_TYPE_HINT)
                    })?;
                    let str_kind = Self::replacement_scan_str_kind(inner).ok_or_else(|| {
                        syn::Error::new_spanned(ty, REPLACEMENT_SCAN_RETURN_TYPE_HINT)
                    })?;
                    return Ok((result_type, str_kind));
                }
            }
        }

        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            REPLACEMENT_SCAN_RETURN_TYPE_HINT,
        ))
    }

    /// 识别 `String` / `&'static str`（可空与不可空都用同一套收尾）。
    ///
    /// Recognises `String` / `&'static str` (both nullable and non-nullable share the same
    /// epilogue).
    fn replacement_scan_str_kind(ty: &Type) -> Option<DuckStrKind> {
        match ty {
            Type::Path(type_path) => type_path
                .path
                .segments
                .last()
                .filter(|s| s.ident == "String")
                .map(|_| DuckStrKind::Owned),
            Type::Reference(type_ref) => match &*type_ref.elem {
                Type::Path(type_path)
                    if type_path
                        .path
                        .segments
                        .last()
                        .is_some_and(|s| s.ident == "str") =>
                {
                    Some(DuckStrKind::Borrowed)
                }
                _ => None,
            },
            _ => None,
        }
    }
}

/// `#[duck_replacement_scan]` 的返回形式：都带「管不管」的语义，
/// 因为 DuckDB 会对每个未解析的表名调用回调，回调必须能拒绝。
///
/// The return forms of `#[duck_replacement_scan]`, all carrying a "take over or not" meaning,
/// because DuckDB calls the callback for every unresolved table name and the callback must be able
/// to decline.
#[derive(Clone, Copy)]
enum DuckReplacementScanResult {
    /// `-> Option<String>` / `-> Option<&'static str>`
    Option,
    /// `-> DuckOptionResult<String>` / `-> DuckOptionResult<&'static str>`
    OptionResult,
}

/// 返回的表函数名是拥有所有权的 `String` 还是 `&'static str`。
///
/// Whether the returned table-function name is an owned `String` or a `&'static str`.
#[derive(Clone, Copy)]
enum DuckStrKind {
    /// `String`
    Owned,
    /// `&'static str`
    Borrowed,
}

const REPLACEMENT_SCAN_RETURN_TYPE_HINT: &str = r#"Only like
    `-> Option<String>` / `-> Option<&'static str>`: redirect when matched;
    `-> DuckOptionResult<String>` / `-> DuckOptionResult<&'static str>`: redirect when matched, may fail the query;
is supported"#;

const REPLACEMENT_SCAN_SIGNATURE_HINT: &str = r#"Only like
    `fn my_scan(path: &str) -> DuckOptionResult<String>`: takes the unresolved table name (usually a file path) and returns the table function to use;
is supported"#;
