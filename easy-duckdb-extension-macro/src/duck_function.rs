use crate::macro_utils::{TokenStream2Result, require_generic_arg_type};
use quote::quote;
use syn::__private::TokenStream2;
use syn::{ItemFn, ReturnType, Type};
use syn_match::path_match;

pub struct ItemFnWrapper {
    item_fn: ItemFn,
}

impl ItemFnWrapper {

}

impl ItemFnWrapper {
    pub fn new(item_fn: ItemFn) -> Self {
        ItemFnWrapper { item_fn }
    }

    pub fn build_scalar_function(&self) -> TokenStream2Result {
        self.common_build(self.build_scalar_function_impl()?)
    }
    pub(crate) fn build_aggregate_function(&self) -> TokenStream2Result {
        self.common_build(self.build_aggregate_function_impl()?)
    }
    pub(crate) fn build_table_function(&self) -> TokenStream2Result {
        todo!()
    }

    fn common_build(&self, scalar_function_impl: TokenStream2) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let duck_args = self.build_duck_args()?;
        let item_fn = &self.item_fn;
        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;

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
        let (_, return_type) = self.scalar_return_type()?;
        let return_clause = self.build_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;

        Ok(quote! {
            use easy_duckdb_extension::ScalarFunctionAdapter;

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
    fn build_aggregate_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let get_data = self.args_to_code(|x| x.build_get_data())?;
        let agg_state_arg = self.agg_state_arg()?;
        let agg_state_type = agg_state_arg.resolve_state_type()?;
        let agg_row_return = self.build_agg_row_return()?;

        Ok(quote! {
            use easy_duckdb_extension::AggregateFunctionAdapter;
            use easy_duckdb_extension::DuckAggregateState;

            #[derive(Default, Debug, Clone)]
            struct AggregateFunctionImpl {
                state: #agg_state_type,
            }

            impl quack_rs::prelude::AggregateState for AggregateFunctionImpl {}

            impl easy_duckdb_extension::AggregateFunctionAdapter for AggregateFunctionImpl {
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = <#agg_state_type as easy_duckdb_extension::DuckAggregateState>::Output;

                // #[duckdb_aggregate_function]
                fn handle_row(&mut self, args: Self::Args) -> easy_duckdb_extension::DuckResult<()> {
                    #name(
                        #(#get_data),*
                    )
                    #agg_row_return
                }

                fn combine(&mut self, other: &Self) -> easy_duckdb_extension::DuckResult<()> {
                    use easy_duckdb_extension::{DuckAggregateState};
                    self.state.combine(&other.state)
                }

                fn result(&self) -> easy_duckdb_extension::DuckOptionResult<Self::Output> {
                    use easy_duckdb_extension::{DuckAggregateState};
                    self.state.result()
                }
            }


            pub fn aggregate_function_builder() -> quack_rs::prelude::AggregateFunctionBuilder {
                AggregateFunctionImpl::aggregate_function_builder()
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
        self.args().iter().map(x).collect::<syn::Result<Vec<_>>>()
    }

    fn scalar_return_type(&self) -> syn::Result<(DuckScalarResult, &Type)> {
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
        let (result_type, _) = self.scalar_return_type()?;
        match result_type {
            DuckScalarResult::Plain => Ok(quote! { Ok(Some(result)) }),
            DuckScalarResult::Option => Ok(quote! { Ok(result) }),
            DuckScalarResult::DuckOptionResult => Ok(quote! { result }),
        }
    }

    fn agg_state_arg(&self) -> syn::Result<FnArgWrapper> {
        for x in self.args() {
            if x.is_agg_state() {
                return Ok(x);
            }
        }
        Err(syn::Error::new_spanned(
            self.item_fn.sig.output.to_owned(),
            "Aggregate state type not found",
        ))
    }

    fn build_agg_row_return(&self) -> TokenStream2Result {
        if let ReturnType::Default=self.item_fn.sig.output {
            return Ok(quote! {
                ;
                Ok(())
            });
        }
        Ok(quote! {})
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

    fn resolve_state_type(&self) -> syn::Result<&syn::Type> {
        let ty = self.resolve_type()?;

        let syn::Type::Reference(type_ref) = ty else {
            return Err(syn::Error::new_spanned(
                ty,
                "State must be like `&mut WcAggState`",
            ));
        };

        if type_ref.mutability.is_none() {
            return Err(syn::Error::new_spanned(
                ty,
                "State must be mutable: `&mut WcAggState`",
            ));
        }

        Ok(&type_ref.elem)
    }

    fn is_agg_state(&self) -> bool {
        if let Ok(ty) = self.resolve_type(){
            if let syn::Type::Reference(type_ref) = ty{
                return type_ref.mutability.is_some();
            }
        }
        false
    }
    fn build_duck_args_field(&self) -> TokenStream2Result {
        let name = self.name()?;
        let ty = self.resolve_type()?;
        if self.is_agg_state() {
            return Ok(quote! {});
        }
        Ok(quote! {
            pub #name: #ty,
        })
    }
    fn build_get_data(&self) -> TokenStream2Result {
        let name = self.name()?;
        if self.is_agg_state() {
            return Ok(quote! {
                 &mut self.state
            });
        }
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
