//! `#[duck_cast_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_cast_function]`.

use crate::common::{ItemFnWrapper, handle_duck_function};
use crate::macro_utils::TokenStream2Result;
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::__private::TokenStream2;
use syn::Type;

/// `#[duck_cast_function(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_cast_function(...)]` accepts. Each attribute macro declares only the keys
/// it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckCastFunctionArgs {
    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成 builder，交给
    /// `#[duck_custom_register]` 手动注册。
    pub(crate) auto_register: Option<bool>,

    /// `#[duck_cast_function(implicit_cost = 100)]`
    /// 隐式转换代价：设置后 DuckDB 可能自动插入该 cast，值越小优先级越高
    ///
    /// `#[duck_cast_function(implicit_cost = 100)]`. Implicit-cast cost: once set, DuckDB may
    /// insert this cast automatically, and a smaller value means higher priority.
    pub(crate) implicit_cost: Option<i64>,
}

/// `#[duck_cast_function]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_cast_function]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(attr, item, |wrapper: ItemFnWrapper<DuckCastFunctionArgs>| {
        wrapper.build_cast_function()
    })
}

impl ItemFnWrapper<DuckCastFunctionArgs> {
    /// `#[duck_cast_function]`：把 `fn(源值) -> 目标值` 变成一个 DuckDB cast 回调。
    ///
    /// 源类型来自唯一参数的 Rust 类型，目标类型来自返回类型（与 `duck_scalar_function`
    /// 的返回形式一致），注册成 `CAST(源 AS 目标)`。
    ///
    /// `#[duck_cast_function]`: turns `fn(source) -> target` into a DuckDB cast callback. The
    /// source type comes from the single parameter's Rust type and the target type from the return
    /// type (using the same return shapes as `duck_scalar_function`), registered as
    /// `CAST(source AS target)`.
    pub(crate) fn build_cast_function(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let item_fn = &self.item_fn;
        let input_type = self.cast_input_type()?;
        let (_, output_type) = self.scalar_return_type()?;
        let return_clause = self.build_scalar_return_clause()?;
        let implicit_cost = self.implicit_cost_override();
        let function_register = self.cast_function_register()?;

        // 入参的可空性由参数类型自己决定，判据是 `DuckValueType::from_null`：
        // 写成 `T` 时 `from_null()` 给不出值 → NULL 直接短路成 NULL（函数体不执行）；
        // 写成 `Option<T>` 时 `from_null()` 给出 `None` → NULL 以 `None` 进入函数体，
        // 语义由函数自己决定。
        //
        // Input nullability is decided by the parameter type through
        // `DuckValueType::from_null`: with `T` it yields no value, so NULL short-circuits to NULL
        // (the body never runs); with `Option<T>` it yields `None`, so NULL reaches the body.
        let body = quote! {
            let value = match value {
                Some(value) => value,
                None => match <Self::Input as duckfn::DuckValueType>::from_null() {
                    Some(value) => value,
                    None => return Ok(None),
                },
            };
            let result = #name(value);
            #return_clause
        };

        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;

                pub struct CastFunctionImpl;

                impl duckfn::CastFunctionAdapter for CastFunctionImpl{
                    const NAME: &'static str = stringify!(#name);
                    type Input = #input_type;
                    type Output = #output_type;

                    #implicit_cost

                    fn apply_with_null(
                        value: Option<Self::Input>,
                    ) -> duckfn::DuckOptionResult<Self::Output> {
                        #body
                    }
                }

                pub fn cast_function_builder() -> quack_rs::prelude::CastFunctionBuilder {
                    use duckfn::CastFunctionAdapter;
                    CastFunctionImpl::cast_function_builder()
                }

                /// 手动注册入口：在 `#[duck_custom_register]` 里调用。
                ///
                /// Manual registration entry point: call it inside `#[duck_custom_register]`.
                pub fn cast_function_register(
                    c: &quack_rs::prelude::Connection,
                ) -> duckfn::DuckResult<()> {
                    use duckfn::CastFunctionAdapter;
                    unsafe { CastFunctionImpl::register(c) }
                }

                #function_register
            }
        })
    }

    /// 生成 cast 函数的自动注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the automatic registration for a cast function; produces nothing when
    /// `auto_register = false`.
    fn cast_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.inventory_submit(quote! {
            use duckfn::CastFunctionAdapter;
            unsafe { CastFunctionImpl::register(c) }
        })
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// 唯一参数就是「源类型的值」：`Option<T>` 表示允许把 NULL 带进函数体。
    ///
    /// 返回声明原样的源类型（`Option<T>` 不再剥掉 —— 它自己实现了 `DuckValueType`，
    /// 可空性由 `DuckValueType::from_null` 承载）；参数个数不为 1 时报编译错误。
    ///
    /// The single parameter is the source value; `Option<T>` allows NULL to reach the body.
    /// Returns the source type exactly as declared (`Option<T>` is no longer stripped — it
    /// implements `DuckValueType` itself, with nullability carried by `DuckValueType::from_null`).
    /// Reports a compile error unless exactly one parameter is present.
    fn cast_input_type(&self) -> syn::Result<Type> {
        let args = self.args();
        if args.len() != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                CAST_SIGNATURE_HINT,
            ));
        }
        Ok(args[0].resolve_type()?.clone())
    }

    /// 生成 `implicit_cost()` 覆盖；未设置 `implicit_cost` 时输出空内容。
    ///
    /// Emits an `implicit_cost()` override; produces nothing when `implicit_cost` is unset.
    fn implicit_cost_override(&self) -> TokenStream2 {
        match self.args.implicit_cost {
            Some(cost) => quote! {
                fn implicit_cost() -> Option<i64> {
                    Some(#cost)
                }
            },
            None => quote! {},
        }
    }
}

const CAST_SIGNATURE_HINT: &str = r#"Only like
    `fn my_cast(from: SourceType) -> TargetType`: the single argument is the source value, the return type is the target type;
    `fn my_cast(from: Option<SourceType>) -> TargetType`: NULL is passed in as None;
is supported"#;
