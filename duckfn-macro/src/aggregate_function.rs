//! `#[duck_aggregate_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_aggregate_function]`.

use crate::common::{FnArgWrapper, ItemFnWrapper, handle_duck_function, null_handling_override};
use crate::macro_utils::TokenStream2Result;
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::ReturnType;

/// `#[duck_aggregate_function(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_aggregate_function(...)]` accepts. Each attribute macro declares only the
/// keys it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckAggregateFunctionArgs {
    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成 builder，交给
    /// `#[duck_custom_register]` 手动注册。
    pub(crate) auto_register: Option<bool>,

    /// SpecialNullHandling
    ///
    /// 是否开启 DuckDB 的 `SpecialNullHandling`（NULL 行也进入回调），默认 `false`。
    pub(crate) special_null_handling: Option<bool>,

    /// `#[duck_aggregate_function(overloads_name = "my_overloads")]`
    ///
    /// 指定重载函数集的名称：设置后不注册自身的函数名，只把本签名作为重载挂到该函数集上
    /// （同名重载由 `duckfn::register_all_aggregate_overload` 分组注册）。
    pub(crate) overloads_name: Option<String>,
}

/// `#[duck_aggregate_function]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_aggregate_function]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(
        attr,
        item,
        |wrapper: ItemFnWrapper<DuckAggregateFunctionArgs>| wrapper.build_aggregate_function(),
    )
}

impl ItemFnWrapper<DuckAggregateFunctionArgs> {
    /// 生成聚合函数：同名模块 + `AggregateFunctionImpl` + 自动注册（可按参数关闭）。
    ///
    /// Generates the aggregate function: a same-named module, `AggregateFunctionImpl` and
    /// automatic registration (which can be disabled by arguments).
    pub(crate) fn build_aggregate_function(&self) -> TokenStream2Result {
        let sql_name = self.sql_name(self.overloads_name());
        let duck_function_impl = self.build_aggregate_function_impl()?;
        self.common_build(&self.args(), None, &sql_name, duck_function_impl)
    }

    /// 生成聚合函数实现体：状态结构体 + `AggregateFunctionImpl` + 各种 builder + 自动注册。
    ///
    /// 参数里的 `&mut XxxState` 会被识别为聚合状态，其余参数作为每行输入。
    ///
    /// Generates the aggregate implementation: the state struct, `AggregateFunctionImpl`, the
    /// builders and the automatic registration. The `&mut XxxState` parameter is recognised as
    /// the aggregate state while the remaining parameters are the per-row inputs.
    fn build_aggregate_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let get_data = self.args_to_code(|x| x.build_get_data())?;
        let agg_state_arg = self.agg_state_arg()?;
        let agg_state_type = agg_state_arg.resolve_state_type()?;
        let agg_row_return = self.build_agg_row_return()?;
        let function_register = self.aggregate_function_register()?;
        let null_handling = null_handling_override(self.special_null_handling());

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

                #null_handling

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
            
            pub fn aggregate_overload_builder(builder: quack_rs::aggregate::builder::OverloadBuilder) -> quack_rs::aggregate::builder::OverloadBuilder {
                use duckfn::AggregateFunctionAdapter;
                AggregateFunctionImpl::aggregate_overload_builder(builder)
            }

            /// 创建可挂进 DuckfnAggregateFunctionSetBuilder 的重载句柄，
            /// 返回类型由本签名的 Output 决定，因此函数集内各重载可有不同返回类型
            ///
            /// Creates an overload handle attachable to `DuckfnAggregateFunctionSetBuilder`.
            /// The return type comes from this signature's `Output`, so overloads in one set may
            /// have different return types.
            pub fn aggregate_function_guard() -> duckfn::AggregateFunctionGuard {
                use duckfn::AggregateFunctionAdapter;
                AggregateFunctionImpl::aggregate_function_guard()
            }

            #function_register
        })
    }

    /// 生成聚合函数的注册代码：`auto_register = false` 时空输出；设置 `overloads_name`
    /// 时提交重载项，否则注册为独立聚合函数。
    ///
    /// Emits the aggregate registration: nothing when `auto_register = false`, an overload item
    /// when `overloads_name` is set, otherwise a standalone aggregate registration.
    fn aggregate_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        // overloads_name：不注册自身函数名，改为提交重载项
        if let Some(set_name) = self.overloads_name() {
            return self.aggregate_overload_submit(set_name);
        }
        self.common_inventory_submit(quote! {
            let builder = aggregate_function_builder();
            unsafe { c.register_aggregate(builder)}
        })
    }

    /// `overloads_name = "xxx"`：提交 `DuckAggregateOverloadItem`，
    /// 由 `duckfn::register_all_aggregate_overload` 按名字分组注册成函数集。
    ///
    /// `overloads_name = "xxx"`: submits a `DuckAggregateOverloadItem`; `duckfn::register_all_aggregate_overload`
    /// groups items by name and registers them as one function set.
    fn aggregate_overload_submit(&self, set_name: &str) -> TokenStream2Result {
        Ok(quote! {
            duckfn::inventory_submit! {
                duckfn::DuckAggregateOverloadItem{
                    name: #set_name,
                    register_fn:|name: &std::ffi::CString| -> duckfn::AggregateFunctionGuard {
                        use duckfn::AggregateFunctionAdapter;
                        AggregateFunctionImpl::create_aggregate_function_guard(name)
                    }
                }
            }
        })
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// `#[duck_aggregate_function(special_null_handling = true)]`
    ///
    /// 是否开启 SpecialNullHandling，默认 `false`。
    ///
    /// Whether to enable `SpecialNullHandling`; defaults to `false`.
    fn special_null_handling(&self) -> bool {
        self.args.special_null_handling.unwrap_or(false)
    }

    /// `#[duck_aggregate_function(overloads_name = "xxx")]`
    ///
    /// 设置后不再注册自身的函数名，而是把本签名作为重载挂到 `xxx` 这个函数集上。
    /// 仍然受 `auto_register` 控制：`auto_register = false` 时完全不提交。
    ///
    /// Once set, the function is not registered under its own name; this signature becomes an
    /// overload of the `xxx` function set. It is still subject to `auto_register`: with
    /// `auto_register = false` nothing is submitted at all.
    fn overloads_name(&self) -> Option<&str> {
        self.args.overloads_name.as_deref()
    }

    /// 找出聚合状态参数（形如 `&mut XxxState` 的可变引用参数）；找不到时报编译错误。
    ///
    /// Finds the aggregate-state parameter (a mutable reference such as `&mut XxxState`), and
    /// reports a compile error when none is present.
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

    /// 聚合函数无返回值（`-> ()`）时补上 `; Ok(())`，有返回值时输出空内容。
    ///
    /// When the aggregate function returns `()` the generated call needs a trailing `; Ok(())`;
    /// otherwise nothing is emitted.
    fn build_agg_row_return(&self) -> TokenStream2Result {
        if let ReturnType::Default = self.item_fn.sig.output {
            return Ok(quote! {
                ;
                Ok(())
            });
        }
        Ok(quote! {})
    }
}
