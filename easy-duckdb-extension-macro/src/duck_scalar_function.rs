use crate::macro_utils::{TokenStream2Result, require_generic_arg_type};
use quote::quote;
use syn::__private::TokenStream2;
use syn::{ItemFn, ReturnType, Type};
use syn_match::path_match;

pub(crate) fn duck_scalar_function(item_fn: ItemFn) -> TokenStream2Result {
    ItemFnWrapper { item_fn }.build_all()
}

struct ItemFnWrapper {
    item_fn: ItemFn,
}
impl ItemFnWrapper {
    fn build_all(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let duck_args = self.build_duck_args()?;
        let scalar_function_impl = self.build_scalar_function_impl()?;
        let item_fn = &self.item_fn;
        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;
                use easy_duckdb_extension::ScalarFunctionAdapter;

                #duck_args

                #scalar_function_impl
            }
        })
    }

    fn build_duck_args(&self) -> TokenStream2Result {
        let fields = self.args_to_code(|x| x.build_duck_args_field())?;
        Ok(quote! {
            #[derive(easy_duckdb_extension_macro::DuckStruct, Clone)]
            pub struct DuckArgsImpl{
                #(#fields)*
            }
        })
    }
    //ScalarFunctionImpl
    fn build_scalar_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let (_, return_type) = self.resolve_return_type()?;
        let return_clause = self.build_return_clause()?;
        let get_data = self.args_to_code(|x| { x.build_get_data()})?;

        Ok(quote! {
            pub struct ScalarFunctionImpl;

            impl easy_duckdb_extension::ScalarFunctionAdapter for ScalarFunctionImpl{
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #return_type;

                fn apply(args: Self::Args) -> easy_duckdb_extension::DuckOptionResult<Self::Output> {
                    let result = #name(
                        #(#get_data),*
                    );
                    #return_clause
                }
            }
            pub fn scalar_function_builder() -> quack_rs::prelude::ScalarFunctionBuilder {
                ScalarFunctionImpl::scalar_function_builder()
            }
            pub fn scalar_overload_builder() -> quack_rs::prelude::ScalarOverloadBuilder {
                ScalarFunctionImpl::scalar_overload_builder()
            }
        })
    }

    fn name(&self) -> &syn::Ident {
        &self.item_fn.sig.ident
    }

    fn visibility(&self) -> &syn::Visibility {
        &self.item_fn.vis
    }

    fn args(&self) -> Vec<FnArgWrapper> {
        self.item_fn
            .sig
            .inputs
            .iter()
            .map(|x| FnArgWrapper {
                fn_arg: x.to_owned(),
            })
            .collect()
    }

    fn args_to_code(
        &self,
        x: fn(&FnArgWrapper) -> TokenStream2Result,
    ) -> syn::Result<Vec<TokenStream2>> {
        self.args()
            .iter()
            .map(x)
            .into_iter()
            .collect::<syn::Result<Vec<_>>>()
    }

    fn resolve_return_type(&self) -> syn::Result<(DuckScalarResult, &Type)> {
        if let ReturnType::Type(_, ty) = &self.item_fn.sig.output {
            if let Type::Path(type_path) = &**ty {
                let path = &type_path.path;
                let inner_opt = path_match!(path,
                    Option<$inner> => Some((DuckScalarResult::Option,inner))
                    easy_duckdb_extension?::DuckOptionResult<$inner> => Some((DuckScalarResult::DuckOptionResult,inner))
                    _=> None
                );
                return match inner_opt {
                    None => Ok((DuckScalarResult::Plain, &**ty)),
                    Some((outter, inner)) => Ok((outter, require_generic_arg_type(inner)?)),
                };
            };
        };
        Err(syn::Error::new_spanned(
            self.item_fn.sig.output.to_owned(),
            "Only like `-> f64` `-> Option<f64>` `-> DuckOptionResult<f64>` is supported",
        ))
    }

    fn build_return_clause(&self) -> TokenStream2Result {
        let (result, return_type) = self.resolve_return_type()?;
        match result {
            DuckScalarResult::Plain => Ok(quote! { Ok(Some(result)) }),
            DuckScalarResult::Option =>Ok(quote! { Ok(result) }),
            DuckScalarResult::DuckOptionResult => Ok(quote! { result }),
        }
    }
}

struct FnArgWrapper {
    fn_arg: syn::FnArg,
}
impl FnArgWrapper {
    fn name(&self) -> syn::Result<&syn::Ident> {
        if let syn::FnArg::Typed(pat) = &self.fn_arg {
            if let syn::Pat::Ident(ref ident) = *pat.pat {
                return Ok(&ident.ident);
            }
        }
        Err(syn::Error::new_spanned(
            self.fn_arg.to_owned(),
            "Only like `foo: f64` is supported",
        ))
    }

    fn resolve_type(&self) -> syn::Result<&syn::Type> {
        if let syn::FnArg::Typed(pat) = &self.fn_arg {
            return Ok(&*pat.ty);
        }
        Err(syn::Error::new_spanned(
            self.fn_arg.to_owned(),
            "Only like `foo: f64` is supported",
        ))
    }
    fn build_duck_args_field(&self) -> TokenStream2Result {
        let name = self.name()?;
        let ty = self.resolve_type()?;
        Ok(quote! {
            pub #name: #ty,
        })
    }
    fn build_get_data(&self) -> TokenStream2Result {
        let name = self.name()?;
        Ok(quote! {
            args.#name
        })
    }
}

#[derive(Clone, Copy)]
enum DuckScalarResult {
    Plain,
    Option,
    DuckOptionResult,
}
