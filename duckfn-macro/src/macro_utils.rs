use proc_macro::TokenStream;
use syn::__private::TokenStream2;
use syn::{GenericArgument, Path, PathArguments, Type, TypeImplTrait, TypeParamBound, TypePath};
use syn_match::path_match;

/// 常用过程宏返回值类型
pub type TokenStream2Result = syn::Result<TokenStream2>;
pub fn handle_token_stream2_result(result: TokenStream2Result) -> TokenStream {
    match result {
        Ok(token) => TokenStream::from(token),
        Err(err) => TokenStream::from(err.to_compile_error()),
    }
}

pub fn extract_option(x: &Type) -> Option<&Type> {
    // extract_option::from_ref(&self.field.ty)
    let Type::Path(type_path) = x else {
        return None;
    };
    let path = &type_path.path;
    path_match!(path,
        Option<$inner> => Some(inner)
        _=> None
    )
    .and_then(|inner| match inner {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

pub fn require_generic_arg_type(x: &GenericArgument) -> syn::Result<&Type> {
    match x {
        GenericArgument::Type(ty) => Ok(ty),
        _ => Err(syn::Error::new_spanned(x, "expected a type")),
    }
}

pub fn get_option_inner(ty: &Type) -> (bool, &Type) {
    get_type_inner(ty, "Option")
}

pub fn get_type_inner<'a>(ty: &'a Type, name: &str) -> (bool, &'a Type) {
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(v) = segments.iter().next() {
            if v.ident == name {
                let t = match &v.arguments {
                    PathArguments::AngleBracketed(a) => match a.args.iter().next() {
                        Some(GenericArgument::Type(t)) => {
                            return (true, t);
                        }
                        _ => {}
                    },
                    (_) => {}
                };
            }
        }
    }
    (false, ty)
}

/// 给泛型加上`::`，例如`Vec<T>` -> `Vec::<T>`
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