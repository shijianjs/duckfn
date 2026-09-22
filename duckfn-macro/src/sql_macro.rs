//! `#[duck_sql_macro]` 的代码生成实现。
//!
//! Code generation behind `#[duck_sql_macro]`.

use crate::common::{DuckDocArgs, DuckDocArgsProvider, ItemFnWrapper, handle_duck_function};
use crate::macro_utils::{TokenStream2Result, extract_generic_arg_type};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::ReturnType;
use syn::Type;

/// `#[duck_sql_macro(...)]` 支持的全部参数（目前只有文档参数）。
///
/// Every argument `#[duck_sql_macro(...)]` accepts (so far only the documentation ones).
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckSqlMacroArgs {
    /// 文档参数：`description` / `comment` / `example`（`examples`）。
    ///
    /// Documentation arguments: `description` / `comment` / `example` (`examples`).
    #[darling(flatten)]
    pub(crate) doc: DuckDocArgs,
}

/// 让公共代码拿到 `#[duck_sql_macro]` 的文档参数。
///
/// Hands `#[duck_sql_macro]`'s documentation arguments to the shared code.
impl DuckDocArgsProvider for DuckSqlMacroArgs {
    fn duck_doc(&self) -> DuckDocArgs {
        self.doc.clone()
    }
}

/// `#[duck_sql_macro]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_sql_macro]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(
        attr,
        item,
        |wrapper: ItemFnWrapper<DuckSqlMacroArgs>| wrapper.build_sql_macro(),
    )
}

impl<A: DuckDocArgsProvider> ItemFnWrapper<A> {
    /// 生成 `#[duck_sql_macro]`：原函数 + 一条 inventory 提交，初始化时注册/执行返回的 SQL。
    ///
    /// 按返回类型分四种收尾方式：`SqlMacro` / `DuckResult<SqlMacro>` 直接交给
    /// `register_sql_macro`；`String` / `&'static str` 及其 `DuckResult` 变体则通过
    /// `register_sql_macro_str` 作为 SQL 文本执行。
    ///
    /// Generates `#[duck_sql_macro]`: the original function plus an inventory submission that
    /// registers or executes the returned SQL during initialisation. Four epilogues are chosen by
    /// return type: `SqlMacro` / `DuckResult<SqlMacro>` go to `register_sql_macro`, while
    /// `String` / `&'static str` and their `DuckResult` variants are executed as SQL text through
    /// `register_sql_macro_str`.
    pub(crate) fn build_sql_macro(&self) -> TokenStream2Result {
        let name = self.name();
        let item_fn = &self.item_fn;

        let register_body = match self.sql_macro_return_type()? {
            DuckSqlMacroResult::SqlMacro => quote! {
                let builder: quack_rs::prelude::SqlMacro = #name();
                use quack_rs::prelude::Registrar;
                unsafe { c.register_sql_macro(builder) }
            },
            DuckSqlMacroResult::ResultSqlMacro => quote! {
                let builder: duckfn::DuckResult<quack_rs::prelude::SqlMacro> = #name();
                use quack_rs::prelude::Registrar;
                unsafe { c.register_sql_macro(builder?) }
            },
            // 直接返回 SQL 字符串，经 duckfn::register_sql_macro_str 执行
            DuckSqlMacroResult::Str => quote! {
                let sql = #name();
                duckfn::register_sql_macro_str(c, &sql)
            },
            DuckSqlMacroResult::ResultStr => quote! {
                let sql = #name();
                duckfn::register_sql_macro_str(c, &sql?)
            },
        };

        let doc_submit = self.doc_inventory_submit(&name.to_string())?;
        Ok(quote! {
            #item_fn
            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn: |c| {
                        #register_body
                    }
                }
            }
            #doc_submit
        })
    }

    /// 解析 `#[duck_sql_macro]` 的返回类型。
    ///
    /// Recognises `SqlMacro` / `DuckResult<SqlMacro>` / `String`（或 `&'static str`）/
    /// `DuckResult<...>`；其他形式报编译错误。
    ///
    /// Parses the return type of `#[duck_sql_macro]`. `SqlMacro` / `DuckResult<SqlMacro>` /
    /// `String` (or `&'static str`) / `DuckResult<...>` are recognised; anything else is a compile
    /// error.
    fn sql_macro_return_type(&self) -> syn::Result<DuckSqlMacroResult> {
        let ReturnType::Type(_, ty) = &self.item_fn.sig.output else {
            return Err(syn::Error::new_spanned(
                self.item_fn.sig.output.to_owned(),
                SQL_MACRO_RETURN_TYPE_HINT,
            ));
        };

        if Self::is_str_type(ty) {
            return Ok(DuckSqlMacroResult::Str);
        }

        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "SqlMacro" {
                    return Ok(DuckSqlMacroResult::SqlMacro);
                }
                if segment.ident == "DuckResult" {
                    if let Some(inner) = extract_generic_arg_type(segment) {
                        if Self::is_str_type(inner) {
                            return Ok(DuckSqlMacroResult::ResultStr);
                        }
                        if Self::is_sql_macro_type(inner) {
                            return Ok(DuckSqlMacroResult::ResultSqlMacro);
                        }
                    }
                }
            }
        }

        Err(syn::Error::new_spanned(
            self.item_fn.sig.output.to_owned(),
            SQL_MACRO_RETURN_TYPE_HINT,
        ))
    }

    /// 判断类型是否为字符串（`String` / `str` / `&str`，自动穿透引用）。
    ///
    /// Whether the type is a string (`String` / `str` / `&str`; references are followed).
    fn is_str_type(ty: &Type) -> bool {
        match ty {
            Type::Path(type_path) => type_path
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "String" || s.ident == "str"),
            Type::Reference(type_ref) => Self::is_str_type(&type_ref.elem),
            _ => false,
        }
    }

    /// 判断类型是否为 `SqlMacro`。
    ///
    /// Whether the type is `SqlMacro`.
    fn is_sql_macro_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "SqlMacro")
        )
    }
}

/// SQL 宏返回类型的外层形式。
///
/// The outer form of a SQL-macro return type.
#[derive(Clone, Copy)]
enum DuckSqlMacroResult {
    /// `-> SqlMacro`
    SqlMacro,
    /// `-> DuckResult<SqlMacro>`
    ResultSqlMacro,
    /// `-> String` / `-> &'static str`
    Str,
    /// `-> DuckResult<String>` / `-> DuckResult<&'static str>`
    ResultStr,
}

const SQL_MACRO_RETURN_TYPE_HINT: &str = r#"Only like
    `-> SqlMacro`: plain sql macro;
    `-> DuckResult<SqlMacro>`: handle input err;
    `-> String` / `-> &'static str`: plain sql string, executed directly;
    `-> DuckResult<String>` / `-> DuckResult<&'static str>`;
is supported"#;
