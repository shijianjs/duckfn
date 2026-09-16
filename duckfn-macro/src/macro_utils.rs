//! 过程宏内部的工具函数：结果类型统一、类型形状识别、token 微调。
//!
//! Shared helpers inside the procedural macros: unified results, type-shape recognition and
//! token tweaks.

use proc_macro::TokenStream;
use syn::__private::TokenStream2;
use syn::{GenericArgument, PathArguments, Type, TypeImplTrait, TypeParamBound};

/// 常用过程宏返回值类型
///
/// 过程宏代码生成的常用返回值类型：成功得到一段 `TokenStream`，失败得到 `syn::Error`
/// （最终会变成编译错误）。
///
/// The result type commonly returned by the procedural macros: on success a `TokenStream` is
/// produced, on failure a `syn::Error` (turned into a compile error).
pub type TokenStream2Result = syn::Result<TokenStream2>;

/// 把 [`TokenStream2Result`] 收敛成 [`TokenStream`]：出错时转成编译错误输出。
///
/// Collapses a [`TokenStream2Result`] into a [`TokenStream`], converting an error into a
/// compile error.
pub fn handle_token_stream2_result(result: TokenStream2Result) -> TokenStream {
    match result {
        Ok(token) => TokenStream::from(token),
        Err(err) => TokenStream::from(err.to_compile_error()),
    }
}

/// 取路径段的第一个泛型类型实参，例如 `Vec<T>` -> `T`、`Option<i64>` -> `i64`。
///
/// Extracts the first generic type argument of a path segment, e.g. `Vec<T>` -> `T` or
/// `Option<i64>` -> `i64`.
pub fn extract_generic_arg_type(segment: &syn::PathSegment) -> Option<&Type> {
    if let PathArguments::AngleBracketed(args) = &segment.arguments {
        if let Some(GenericArgument::Type(ty)) = args.args.first() {
            Some(ty)
        } else {
            None
        }
    } else {
        None
    }
}

/// 要求泛型参数必须是类型参数，否则报编译错误。
///
/// Requires a generic argument to be a type, reporting a compile error otherwise.
#[allow(dead_code)]
pub fn require_generic_arg_type(x: &GenericArgument) -> syn::Result<&Type> {
    match x {
        GenericArgument::Type(ty) => Ok(ty),
        _ => Err(syn::Error::new_spanned(x, "expected a type")),
    }
}

/// 把 `PascalCase` / `camelCase` 标识符转成小写蛇形：`PriorityLevel` -> `priority_level`、
/// `HTTPCode` -> `http_code`。
///
/// 用于 `#[derive(DuckEnum)]` 的默认 SQL 类型名与内部辅助模块名。
///
/// Converts a `PascalCase` / `camelCase` identifier into lowercase snake_case
/// (`PriorityLevel` -> `priority_level`, `HTTPCode` -> `http_code`). Used by
/// `#[derive(DuckEnum)]` for the default SQL type name and the internal helper module.
#[must_use]
pub fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (index, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            let prev_is_lower_or_digit =
                index > 0 && (chars[index - 1].is_lowercase() || chars[index - 1].is_ascii_digit());
            let next_is_lower = chars
                .get(index + 1)
                .is_some_and(|next| next.is_lowercase());
            if index > 0 && (prev_is_lower_or_digit || next_is_lower) {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(*ch);
        }
    }
    out
}

/// 获取迭代器`impl Iterator<Item=T>`的`Item`类型
///
/// Extracts the `Item` type of an `impl Iterator<Item = T>` trait object.
pub fn iterator_item_type(impl_trait: &TypeImplTrait) -> Option<&Type> {
    for bound in &impl_trait.bounds {
        if let TypeParamBound::Trait(trait_bound) = bound {
            if let Some(segment) = trait_bound.path.segments.last() {
                if segment.ident == "Iterator" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        for arg in &args.args {
                            if let GenericArgument::AssocType(assoc) = arg {
                                if assoc.ident == "Item" {
                                    return Some(&assoc.ty);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}
