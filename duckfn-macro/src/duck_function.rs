use crate::macro_utils::{iterator_item_type, require_generic_arg_type, TokenStream2Result};
use quote::quote;
use syn::__private::TokenStream2;
use syn::{GenericArgument, ItemFn, PathArguments, ReturnType, Type, TypeImplTrait, TypeParamBound};
use syn_match::path_match;

pub struct ItemFnWrapper {
    pub item_fn: ItemFn,
    pub attr:TokenStream2
}


impl ItemFnWrapper {

    pub fn build_scalar_function(&self) -> TokenStream2Result {
        self.common_build(self.build_scalar_function_impl()?)
    }
    pub(crate) fn build_aggregate_function(&self) -> TokenStream2Result {
        self.common_build(self.build_aggregate_function_impl()?)
    }
    pub(crate) fn build_table_function(&self) -> TokenStream2Result {
        self.common_build(self.build_table_function_impl()?)
    }

    fn common_build(&self, duck_function_impl: TokenStream2) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let duck_args = self.build_duck_args()?;
        let item_fn = &self.item_fn;
        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;

                #duck_args

                #duck_function_impl
            }
        })
    }

    fn build_duck_args(&self) -> TokenStream2Result {
        let fields = self.args_to_code(|x| x.build_duck_args_field())?;
        let attr = &self.attr;

        Ok(quote! {

            #[derive(duckfn::DuckStruct, Debug, Clone, Default)]
            #[duck(#attr)]
            pub struct DuckArgsImpl{
                #(#fields)*
            }
        })
    }
    //ScalarFunctionImpl
    fn build_scalar_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let (_, return_type) = self.scalar_return_type()?;
        let return_clause = self.build_scalar_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;

        Ok(quote! {

            pub struct ScalarFunctionImpl;

            impl duckfn::ScalarFunctionAdapter for ScalarFunctionImpl{
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #return_type;

                fn apply(args: Self::Args) -> duckfn::DuckOptionResult<Self::Output> {
                    let result = #name(
                        #(#get_data),*
                    );
                    #return_clause
                }
            }
            pub fn scalar_function_builder() -> quack_rs::prelude::ScalarFunctionBuilder {
                use duckfn::ScalarFunctionAdapter;
                ScalarFunctionImpl::scalar_function_builder()
            }
            pub fn scalar_overload_builder() -> quack_rs::prelude::ScalarOverloadBuilder {
                use duckfn::ScalarFunctionAdapter;
                ScalarFunctionImpl::scalar_overload_builder()
            }

            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn:|c|{
                        use quack_rs::prelude::Registrar;
                        let builder = scalar_function_builder();
                        unsafe { c.register_scalar(builder)}
                    }
                }
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
            #[derive(Default, Debug, Clone)]
            struct AggregateFunctionImpl {
                state: #agg_state_type,
            }

            impl quack_rs::prelude::AggregateState for AggregateFunctionImpl {}

            impl duckfn::AggregateFunctionAdapter for AggregateFunctionImpl {
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = <#agg_state_type as duckfn::DuckAggregateState>::Output;

                // #[duckdb_aggregate_function]
                fn handle_row(&mut self, args: Self::Args) -> duckfn::DuckResult<()> {
                    #name(
                        #(#get_data),*
                    )
                    #agg_row_return
                }

                fn combine(&mut self, other: &Self) -> duckfn::DuckResult<()> {
                    use duckfn::{DuckAggregateState};
                    self.state.combine(&other.state)
                }

                fn result(&self) -> duckfn::DuckOptionResult<Self::Output> {
                    use duckfn::{DuckAggregateState};
                    self.state.result()
                }
            }


            pub fn aggregate_function_builder() -> quack_rs::prelude::AggregateFunctionBuilder {
                use duckfn::AggregateFunctionAdapter;
                AggregateFunctionImpl::aggregate_function_builder()
            }

            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn:|c|{
                        use quack_rs::prelude::Registrar;
                        let builder = aggregate_function_builder();
                        unsafe { c.register_aggregate(builder)}
                    }
                }
            }
        })
    }

    fn build_table_function_impl(&self) -> TokenStream2Result {
        let name = self.name();

        let (_, return_type) = self.table_return_type()?;
        let return_clause = self.build_table_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;

        Ok(quote! {
            use duckfn::TableFunctionAdapter;

            pub struct TableFunctionImpl;

            impl duckfn::TableFunctionAdapter for TableFunctionImpl {
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #return_type;

                fn init_data_iterator(
                    args: Self::Args,
                ) -> duckfn::DuckFullIteratorResult<Self::Output> {
                    let result = #name(
                        #(#get_data),*
                    );
                    #return_clause
                }
            }

            pub fn table_function_builder() -> duckfn::DuckResult<quack_rs::prelude::TableFunctionBuilder> {
                TableFunctionImpl::table_function_builder()
            }

            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn:|c| {
                        use quack_rs::prelude::Registrar;
                        let builder = table_function_builder()?;
                        unsafe { c.register_table(builder)}
                    }
                }
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
                    duckfn?::DuckOptionResult<$inner> => Some((DuckScalarResult::DuckOptionResult,inner))
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
    // fn table_return_type(&self) -> syn::Result<(DuckTableResult, &Type)> {
    //     if let ReturnType::Type(_, ty) = self.return_type() {
    //         if let Type::Path(type_path) = &**ty {
    //             // let path = &type_path.path;
    //             // let inner_opt = path_match!(path,
    //             //     duckfn?::DuckFullIteratorResult<$inner> => Some((DuckTableResult::Full,inner))
    //             //     duckfn?::DuckResult<impl Iterator<Item = SomeDuckStruct>> => Some((DuckTableResult::ResultIterator,inner))
    //             //     impl Iterator<Item = <$inner>> => Some((DuckTableResult::SimpleIterator,inner))
    //             //     _=> None
    //             // );
    //             // return match inner_opt {
    //             //     None => Ok((DuckScalarResult::Plain, &**ty)),
    //             //     Some((outter, inner)) => Ok((outter, require_generic_arg_type(inner)?)),
    //             // };
    //         };
    //     };
    //     Err(syn::Error::new_spanned(
    //         self.item_fn.sig.output.to_owned(),
    //         "Only like
    //             `-> impl Iterator<Item = SomeDuckStruct>`: simple;
    //             `-> DuckResult<impl Iterator<Item = SomeDuckStruct>>`: handle input err;
    //             `-> DuckIteratorResult<SomeDuckStruct>: handle input err and output row err;
    //         is supported",
    //     ))
    // }
    fn table_return_type(&self) -> syn::Result<(DuckTableResult, &Type)> {
        if let ReturnType::Type(_, ty) = self.return_type() {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "DuckFullIteratorResult" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                                return Ok((DuckTableResult::Full, inner));
                            }
                        }
                    }

                    if segment.ident == "DuckResult" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(GenericArgument::Type(inner)) = args.args.first() {
                                if let Type::ImplTrait(impl_trait) = inner {
                                    if let Some(item) = iterator_item_type(impl_trait) {
                                        return Ok((DuckTableResult::ResultIterator, item));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Type::ImplTrait(impl_trait) = &**ty {
                if let Some(item) = iterator_item_type(impl_trait) {
                    return Ok((DuckTableResult::SimpleIterator, item));
                }
            }
        }

        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            "Only like
                `-> impl Iterator<Item = SomeDuckStruct>`: simple;
                `-> DuckResult<impl Iterator<Item = SomeDuckStruct>>`: handle input err;
                `-> DuckFullIteratorResult<SomeDuckStruct>`: handle input err and output row err;
            is supported",
        ))
    }

    fn return_type(&self) -> &ReturnType {
        &self.item_fn.sig.output
    }

    fn build_scalar_return_clause(&self) -> TokenStream2Result {
        let (result_type, _) = self.scalar_return_type()?;
        match result_type {
            DuckScalarResult::Plain => Ok(quote! { Ok(Some(result)) }),
            DuckScalarResult::Option => Ok(quote! { Ok(result) }),
            DuckScalarResult::DuckOptionResult => Ok(quote! { result }),
        }
    }
    fn build_table_return_clause(&self) -> TokenStream2Result {
        let (result_type, _) = self.table_return_type()?;
        match result_type {
            DuckTableResult::Full => Ok(quote! { result }),
            DuckTableResult::ResultIterator => Ok(quote! { Ok(Box::new(result?.map(|x| Ok(Some(x))))) }),
            DuckTableResult::SimpleIterator => Ok(quote! { Ok(Box::new(result.map(|x| Ok(Some(x))))) }),
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

#[derive(Clone, Copy)]
enum DuckTableResult {
    Full,
    ResultIterator,
    SimpleIterator,
}

