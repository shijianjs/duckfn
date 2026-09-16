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

/// 给泛型加上`::`，例如`Vec<T>` -> `Vec::<T>`
///
/// 在生成 `Vec::<T>::create_reader_from_vector(...)` 这类「泛型类型调用关联函数」的代码时，
/// 需要 `::<T>` 形式才能通过解析。
///
/// Adds `::` to a generic, e.g. `Vec<T>` -> `Vec::<T>`. Code such as
/// `Vec::<T>::create_reader_from_vector(...)` that calls an associated function on a generic
/// type needs this turbofish form to parse.
pub fn add_colon2_token(ty: &mut syn::Type) {
    let syn::Type::Path(type_path) = ty else {
        return;
    };

    let Some(segment) = type_path.path.segments.last_mut() else {
        return;
    };

    let syn::PathArguments::AngleBracketed(args) = &mut segment.arguments else {
        return;
    };

    args.colon2_token = Some(Default::default());
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
