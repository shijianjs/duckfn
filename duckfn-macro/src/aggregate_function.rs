//! `#[duck_aggregate_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_aggregate_function]`.

use crate::common::{
    DuckDocArgs, DuckDocArgsProvider, FnArgWrapper, ItemFnWrapper, handle_duck_function,
    null_handling_override,
};
use crate::macro_utils::{TokenStream2Result, extract_generic_arg_type};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::__private::TokenStream2;
use syn::{Ident, ItemFn, ReturnType, Type, Visibility};

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

    /// `#[duck_aggregate_function(auto_collect = true)]`
    ///
    /// 「先收集整列、再一次性计算」模式，默认 `false`。开启后函数不再是逐行回调，而是聚合的
    /// **收尾函数**：参数用 `Vec<T>` 声明要收集的列、用 `DuckFirst<T>` 声明逐行不变的标量常量，
    /// 返回值即聚合结果（`-> T` / `-> Option<T>` / `-> DuckOptionResult<T>`）。宏自动生成收集状态
    /// 与 `combine`，在 finalize 时把收集好的 `Vec<T>` 与解析一次的标量交给本函数，省掉手写状态
    /// 结构体、`DuckAggregateState`、逐行 `push` 那一整套样板。空值传播与原写法一致：`Vec<T>` 里
    /// 的 `T` 非 `Option` 时该行任一非可空列为 NULL 就整行丢弃（想收集可空值写 `Vec<Option<T>>`）。
    pub(crate) auto_collect: Option<bool>,

    /// `#[duck_aggregate_function(overloads_name = "my_overloads")]`
    ///
    /// 指定重载函数集的名称：设置后不注册自身的函数名，只把本签名作为重载挂到该函数集上
    /// （同名重载由 `duckfn::register_all_aggregate_overload` 分组注册）。
    pub(crate) overloads_name: Option<String>,

    /// 文档参数：`description` / `comment` / `example`（`examples`）。
    ///
    /// Documentation arguments: `description` / `comment` / `example` (`examples`).
    #[darling(flatten)]
    pub(crate) doc: DuckDocArgs,
}

/// 让公共代码拿到 `#[duck_aggregate_function]` 的文档参数。
///
/// Hands `#[duck_aggregate_function]`'s documentation arguments to the shared code.
impl DuckDocArgsProvider for DuckAggregateFunctionArgs {
    fn duck_doc(&self) -> DuckDocArgs {
        self.doc.clone()
    }
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
        if self.auto_collect() {
            return self.build_auto_collect_aggregate_function();
        }
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

    /// `#[duck_aggregate_function(auto_collect = true)]`
    ///
    /// 是否开启「先收集后计算」模式，默认 `false`。
    ///
    /// Whether the collect-then-compute mode is enabled; defaults to `false`.
    fn auto_collect(&self) -> bool {
        self.args.auto_collect.unwrap_or(false)
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

    /// 生成「先收集后计算」聚合：宏自动产出收集状态（`Vec<T>` + `DuckLazySlot<T>`）、
    /// `combine` 与 `result`，在 finalize 时调用被标注函数（此时它是收尾函数，不是逐行回调）。
    ///
    /// **实现方式：递归生成普通宏声明**。`auto_collect` 不自己重搭 `AggregateFunctionAdapter` /
    /// builder / guard 那一整套（那是与逐行版重复、且容易各自踩坑的实现），而是把带 `auto_collect`
    /// 的签名**改写成一个不带 `auto_collect` 的 `#[duck_aggregate_function]` 声明**（逐行 `push` /
    /// `resolve` 的行处理器）+ 一个像 `aggregate_summary.rs` 那样的收集状态结构体（`Vec<T>` +
    /// `DuckLazySlot<T>`、`simple_combine` 并入、`result` 调用收尾函数）。改写后的产物交给同一个宏再
    /// 处理一次，于是逐行版所有已测试的逻辑（DuckArgsImpl、空值传播、builder、注册、文档提交）原样复用。
    ///
    /// Generates a collect-then-compute aggregate by RE-EMITTING a plain `#[duck_aggregate_function]`
    /// declaration (a per-row `push`/`resolve` handler) plus a collecting state struct shaped exactly
    /// like `aggregate_summary.rs`. The rewritten output runs through this same macro again, so every
    /// piece of the tested per-row path (DuckArgsImpl, NULL propagation, builders, registration, doc
    /// submission) is reused verbatim rather than duplicated.
    fn build_auto_collect_aggregate_function(&self) -> TokenStream2Result {
        let args = self.classify_auto_collect_args()?;
        if args.is_empty() {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                AUTO_COLLECT_SIGNATURE_HINT,
            ));
        }
        let (_, output) = self.scalar_return_type()?;
        let return_clause = self.build_scalar_return_clause()?;
        let calc_fn = self.build_auto_collect_calc_fn();
        let state_item = self.build_auto_collect_state(&args, output, &return_clause);
        let row_handler = self.build_auto_collect_row_handler(&args);
        Ok(quote! {
            #calc_fn
            #state_item
            #row_handler
        })
    }

    /// 把被标注函数原样留着作为**收尾计算函数**，只重命名（让出 SQL 函数名给行处理器）并取消可见性
    /// （它只在生成的 `result()` 里被同模块私有访问）。签名保持用户写的 `Vec<T>` / `DuckFirst<T>`，
    /// `DuckFirst` 是恒等别名，所以参数实际类型就是收集好的 `Vec<T>` 与解析后的 `T`。
    ///
    /// Keeps the annotated function as the finalize/compute function, only renaming it (so the SQL name
    /// is free for the row handler) and dropping its visibility (it is reached from the generated
    /// `result()` in the same module). The signature keeps the user's `Vec<T>` / `DuckFirst<T>`: because
    /// `DuckFirst` is an identity alias, the parameters really are the collected `Vec<T>` and the
    /// resolved `T`.
    fn build_auto_collect_calc_fn(&self) -> ItemFn {
        let mut func = self.item_fn.clone();
        func.sig.ident = format_ident!("__duckfn_auto_collect_finalize_{}", self.name());
        func.vis = Visibility::Inherited;
        func
    }

    /// 生成收集状态结构体 + `DuckAggregateState`（与 `aggregate_summary.rs` 同形）：每个 `Vec<T>`
    /// 参数是一个收集字段、每个 `DuckFirst<T>` 是一个 `DuckLazySlot`；`result()` 调用收尾函数。
    ///
    /// Builds the collecting state struct + its `DuckAggregateState` (the same shape as
    /// `aggregate_summary.rs`): one field per `Vec<T>` parameter, one `DuckLazySlot` per `DuckFirst<T>`,
    /// and a `result()` that calls the finalize function.
    fn build_auto_collect_state(
        &self,
        args: &[AutoCollectArg],
        output: &Type,
        return_clause: &TokenStream2,
    ) -> TokenStream2 {
        let name = self.name();
        let state_ident = format_ident!("__duckfn_auto_collect_state_{}", name);
        let calc_ident = format_ident!("__duckfn_auto_collect_finalize_{}", name);
        let fields = args.iter().map(|a| match a {
            AutoCollectArg::Collect { name, row_ty } => quote! {
                #name: Vec<#row_ty>,
            },
            AutoCollectArg::Lazy {
                name, value_ty, ..
            } => {
                let slot = slot_ident(name);
                quote! {
                    #slot: ::duckfn::DuckLazySlot<#value_ty>,
                }
            }
        });
        let combine_ops = args.iter().map(|a| match a {
            AutoCollectArg::Collect { name, .. } => quote! {
                self.#name.extend(other.#name.iter().cloned());
            },
            AutoCollectArg::Lazy { name, .. } => {
                let slot = slot_ident(name);
                quote! {
                    self.#slot.combine(&other.#slot);
                }
            }
        });
        let finalize_args = args.iter().map(|a| match a {
            AutoCollectArg::Collect { name, .. } => quote! {
                self.#name.clone()
            },
            AutoCollectArg::Lazy {
                name,
                optional: true,
                ..
            } => {
                let slot = slot_ident(name);
                quote! {
                    self.#slot.get().map(|__duckfn_value| (*__duckfn_value).clone())
                }
            }
            AutoCollectArg::Lazy {
                name,
                optional: false,
                ..
            } => {
                let slot = slot_ident(name);
                quote! {
                    match self.#slot.get() {
                        Some(__duckfn_value) => (*__duckfn_value).clone(),
                        None => return Ok(None),
                    }
                }
            }
        });
        quote! {
            #[allow(non_camel_case_types)]
            #[derive(Default, Debug, Clone)]
            struct #state_ident {
                #(#fields)*
            }

            #[allow(non_camel_case_types)]
            impl ::duckfn::DuckAggregateState for #state_ident {
                type Output = #output;

                fn simple_combine(&mut self, other: &Self) {
                    #(#combine_ops)*
                }

                fn result(&self) -> ::duckfn::DuckOptionResult<#output> {
                    let result = #calc_ident(
                        #(#finalize_args),*
                    );
                    #return_clause
                }
            }
        }
    }

    /// 生成行处理器：`#[duck_aggregate_function(<转发参数>) ] fn <原名>(逐行参数..., state: &mut
    /// <状态>) -> DuckResult<()>`，函数体只做 `push` / `resolve`。这个属性会被同一个宏再处理一次，
    /// 于是逐行版逻辑全量复用。转发时**去掉 `auto_collect`**（否则无限递归）。
    ///
    /// Builds the row handler: `#[duck_aggregate_function(<forwarded args>)] fn <orig name>(per-row
    /// params..., state: &mut <state>) -> DuckResult<()>` whose body only pushes / resolves. The
    /// attribute is expanded again by this very macro, so the per-row logic is reused wholesale. The
    /// forwarded arguments **drop `auto_collect`** (otherwise it would recurse forever).
    fn build_auto_collect_row_handler(&self, args: &[AutoCollectArg]) -> TokenStream2 {
        let name = self.name();
        let state_ident = format_ident!("__duckfn_auto_collect_state_{}", name);
        let attr = self.build_auto_collect_forwarded_attr();
        let params = args.iter().map(|a| match a {
            AutoCollectArg::Collect { name, row_ty } => quote! {
                #name: #row_ty
            },
            AutoCollectArg::Lazy {
                name,
                value_ty,
                optional: false,
            } => quote! {
                #name: ::duckfn::DuckLazy<#value_ty>
            },
            AutoCollectArg::Lazy {
                name,
                value_ty,
                optional: true,
            } => quote! {
                #name: Option<::duckfn::DuckLazy<#value_ty>>
            },
        });
        let ops = args.iter().map(|a| match a {
            AutoCollectArg::Collect { name, .. } => quote! {
                state.#name.push(#name);
            },
            AutoCollectArg::Lazy {
                name,
                optional: false,
                ..
            } => {
                let slot = slot_ident(name);
                quote! {
                    state.#slot.resolve(&#name)?;
                }
            }
            AutoCollectArg::Lazy {
                name,
                optional: true,
                ..
            } => {
                let slot = slot_ident(name);
                quote! {
                    state.#slot.resolve_optional(#name.as_ref())?;
                }
            }
        });
        quote! {
            #attr
            fn #name(
                #(#params,)*
                state: &mut #state_ident,
            ) -> ::duckfn::DuckResult<()> {
                #(#ops)*
                Ok(())
            }
        }
    }

    /// 重新拼出转发给行处理器的 `#[::duckfn::duck_aggregate_function(...)]`：保留 `auto_register` /
    /// `special_null_handling` / `overloads_name` 与文档参数，唯独去掉 `auto_collect`。
    ///
    /// Rebuilds the forwarded `#[::duckfn::duck_aggregate_function(...)]`: keeps `auto_register` /
    /// `special_null_handling` / `overloads_name` and the documentation arguments, dropping only
    /// `auto_collect`.
    fn build_auto_collect_forwarded_attr(&self) -> TokenStream2 {
        let args = &self.args;
        let mut parts: Vec<TokenStream2> = Vec::new();
        if let Some(value) = args.auto_register {
            parts.push(quote! { auto_register = #value });
        }
        if let Some(value) = args.special_null_handling {
            parts.push(quote! { special_null_handling = #value });
        }
        if let Some(value) = &args.overloads_name {
            parts.push(quote! { overloads_name = #value });
        }
        if let Some(value) = &args.doc.description {
            parts.push(quote! { description = #value });
        }
        if let Some(value) = &args.doc.comment {
            parts.push(quote! { comment = #value });
        }
        if let Some(value) = &args.doc.example {
            parts.push(quote! { example = #value });
        }
        if let Some(lits) = &args.doc.examples {
            parts.push(quote! { examples = [ #(#lits),* ] });
        }
        quote! {
            #[::duckfn::duck_aggregate_function(#(#parts),*)]
        }
    }

    /// 把每个参数归类成「收集列 `Vec<T>`」或「解析一次的标量 `DuckFirst<T>`」；
    /// 出现 `&mut State` 或其它类型时报编译错误。
    ///
    /// Classifies every parameter into a collected column (`Vec<T>`) or a resolve-once scalar
    /// (`DuckFirst<T>`); a `&mut State` or any other type is a compile error.
    fn classify_auto_collect_args(&self) -> syn::Result<Vec<AutoCollectArg>> {
        let mut out = Vec::new();
        for arg in self.args() {
            let name = arg.name()?.clone();
            let ty = arg.resolve_type()?;
            if arg.is_agg_state() {
                return Err(syn::Error::new_spanned(
                    ty,
                    "`auto_collect = true` builds the state itself: the function must not take a \
                     `&mut XxxState` parameter. Declare the columns to collect as `Vec<T>` (and \
                     per-query constants as `DuckFirst<T>`) and return the aggregate result \
                     directly.",
                ));
            }
            if let Some(row_ty) =
                generic_element_type(ty, "Vec").or_else(|| generic_element_type(ty, "DuckList"))
            {
                out.push(AutoCollectArg::Collect {
                    name,
                    row_ty: row_ty.clone(),
                });
                continue;
            }
            if let Some(inner) = generic_element_type(ty, "DuckFirst") {
                // `DuckFirst<Option<X>>` → 可空标量（整列 NULL 得 None）；`DuckFirst<X>` → 非可空
                // （NULL 行整行丢弃）。槽与值类型都用去掉外层 Option 后的 `X`。
                //
                // `DuckFirst<Option<X>>` is a nullable scalar (an all-NULL column yields `None`);
                // `DuckFirst<X>` is non-nullable (a NULL row is dropped). The slot and value type use
                // `X`, i.e. the outer `Option` stripped off.
                let (value_ty, optional) = match generic_element_type(inner, "Option") {
                    Some(x) => (x.clone(), true),
                    None => (inner.clone(), false),
                };
                out.push(AutoCollectArg::Lazy {
                    name,
                    value_ty,
                    optional,
                });
                continue;
            }
            return Err(syn::Error::new_spanned(
                ty,
                AUTO_COLLECT_SIGNATURE_HINT,
            ));
        }
        Ok(out)
    }

}


/// `auto_collect` 归类出来的单个参数。
///
/// One `auto_collect` parameter, once classified.
enum AutoCollectArg {
    /// 要收集的列：`Vec<#row_ty>` 参数 → 逐行读 `#row_ty`、状态收集 `Vec<#row_ty>`。
    ///
    /// A collected column: a `Vec<#row_ty>` parameter reads `#row_ty` per row and stores a
    /// `Vec<#row_ty>` in the state.
    Collect {
        name: Ident,
        row_ty: Type,
    },
    /// 解析一次的标量常量：`DuckFirst<..>` 参数 → 逐行 `DuckLazy`、状态 `DuckLazySlot<#value_ty>`。
    /// `optional` 表示 `DuckFirst<Option<#value_ty>>`（可空，整列 NULL 得 `None`）。
    ///
    /// A resolve-once scalar: a `DuckFirst<..>` parameter reads `DuckLazy` per row and keeps a
    /// `DuckLazySlot<#value_ty>` in the state. `optional` marks `DuckFirst<Option<#value_ty>>`
    /// (nullable: an all-NULL column yields `None`).
    Lazy {
        name: Ident,
        value_ty: Type,
        optional: bool,
    },
}

/// 收集状态里 `DuckFirst` 槽字段的标识符。
///
/// The identifier of a `DuckFirst` slot field inside the generated state.
fn slot_ident(name: &Ident) -> Ident {
    format_ident!("__duckfn_slot_{}", name)
}

/// 取 `Name<T>` 里的 `T`；类型不是 `Name<...>`（或没有类型参数）时返回 `None`。
///
/// Returns the `T` of a `Name<T>`; `None` when the type is not a `Name<...>` or carries no type
/// argument.
fn generic_element_type<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    extract_generic_arg_type(segment)
}

/// `auto_collect = true` 的函数签名要求（编译错误里直接展示给用户）。
///
/// The signature `auto_collect = true` requires, shown verbatim in the compile error.
const AUTO_COLLECT_SIGNATURE_HINT: &str = r#"With `auto_collect = true` the function is the finalize handler, not a row callback. Every parameter must be either:
    - `Vec<T>` (a column to collect across rows; use `Vec<Option<T>>` to keep NULL values), or
    - `DuckFirst<T>` (a per-query constant resolved once; use `DuckFirst<Option<T>>` for a nullable one).
The return value is the aggregate result (`-> T`, `-> Option<T>` or `-> DuckOptionResult<T>`), and there is no `&mut XxxState` parameter.
Example:
    #[duck_aggregate_function(auto_collect = true)]
    fn sr_covariance(x: Vec<f64>, y: Vec<f64>, options: DuckFirst<Option<ComplexOptions>>) -> f64 {
        some_calc(x, y, options)
    }"#;
