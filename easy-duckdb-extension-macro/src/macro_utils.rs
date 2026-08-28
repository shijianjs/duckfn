use syn::__private::TokenStream2;
use syn::{GenericArgument, Path, PathArguments, Type, TypePath};

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