//! `#[duck_scalar_function]` / `#[duck_aggregate_function]` / `#[duck_table_function]` /
//! `#[duck_cast_function]` / `#[duck_sql_macro]` / `#[duck_replacement_scan]` /
//! `#[duck_custom_register]` 的代码生成实现。
//!
//! Code generation behind `#[duck_scalar_function]`, `#[duck_aggregate_function]`,
//! `#[duck_table_function]`, `#[duck_cast_function]`, `#[duck_sql_macro]`,
//! `#[duck_replacement_scan]` and `#[duck_custom_register]`.

use crate::attr_args::DuckArgs;
use crate::macro_utils::{
    TokenStream2Result, extract_generic_arg_type, extract_option, iterator_item_type,
};
use quote::quote;
use syn::__private::TokenStream2;
use syn::{GenericArgument, ItemFn, PathArguments, ReturnType, Type};

/// 被标注函数的包装：原始函数 + 原始属性参数 + 解析后的 [`DuckArgs`]。
///
/// 各属性宏的方法都基于它生成代码，最终产出的模块名与函数名相同。
///
/// Wrapper around the annotated function: the original item, the raw attribute tokens and the
/// parsed [`DuckArgs`]. All attribute-macro methods generate code from it, and the resulting
/// module is named after the function.
pub struct ItemFnWrapper {
    /// 被 `#[duck_*]` 标注的原始函数（会原样输出到生成代码里）。
    ///
    /// The original function annotated with `#[duck_*]` (emitted verbatim in the generated code).
    pub item_fn: ItemFn,
    /// 属性宏收到的原始 token，会原样转写到 `#[duck(...)]` 上。
    ///
    /// The raw attribute tokens, written through to `#[duck(...)]` unchanged.
    pub attr: TokenStream2,
    /// 解析后的属性参数。
    ///
    /// The parsed attribute arguments.
    pub duck_args: DuckArgs,
}

// 预留的空 impl，方便后续把 `ItemFnWrapper` 的构造逻辑单独放在这里。
//
// Empty impl reserved for future constructors of `ItemFnWrapper`.
impl ItemFnWrapper {

}

impl ItemFnWrapper {

    /// 生成标量函数：同名模块 + `ScalarFunctionImpl` + 自动注册（可按参数关闭）。
    ///
    /// Generates the scalar function: a same-named module, `ScalarFunctionImpl` and automatic
    /// registration (which can be disabled by arguments).
    pub fn build_scalar_function(&self) -> TokenStream2Result {
        self.common_build(self.build_scalar_function_impl()?)
    }

    /// 生成聚合函数：同名模块 + `AggregateFunctionImpl` + 自动注册（可按参数关闭）。
    ///
    /// Generates the aggregate function: a same-named module, `AggregateFunctionImpl` and
    /// automatic registration (which can be disabled by arguments).
    pub(crate) fn build_aggregate_function(&self) -> TokenStream2Result {
        self.common_build(self.build_aggregate_function_impl()?)
    }

    /// 生成表函数：同名模块 + `TableFunctionImpl` + 自动注册（可按参数关闭）。
    ///
    /// Generates the table function: a same-named module, `TableFunctionImpl` and automatic
    /// registration (which can be disabled by arguments).
    pub(crate) fn build_table_function(&self) -> TokenStream2Result {
        self.common_build(self.build_table_function_impl()?)
    }

    /// 生成 `#[duck_custom_register]`：原函数 + 一条 inventory 提交，把函数本身当作注册回调。
    ///
    /// Generates `#[duck_custom_register]`: the original function plus an inventory submission
    /// that uses the function itself as the registration callback.
    pub(crate) fn build_custom_register(&self) -> TokenStream2Result {
        let name = self.name();
        let item_fn = &self.item_fn;

        Ok(quote! {
            #item_fn
            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn: #name
                }
            }
        })
    }

    /// 生成 `#[duck_sql_macro]`：原函数 + 一条 inventory 提交，初始化时注册/执行返回的 SQL。
    ///
    /// 按返回类型分四种收尾方式：`SqlMacro` / `DuckResult<SqlMacro>` 直接交给
    /// `register_sql_macro`；`String` / `&'static str` 及其 `DuckResult` 变体则通过
    /// `register_sql_macro_str` 作为 SQL 文本执行。
    ///
    /// Generates `#[duck_sql_macro]`: the original function plus an inventory submission that
    /// registers or executes the returned SQL during initialisation. Four epilogues are chosen by
    /// return type: `SqlMacro` / `DuckResult<SqlMacro>` go to `register_sql_macro`, while
    /// `String` / `&'static str` and their `DuckResult` variants are executed as SQL text through
    /// `register_sql_macro_str`.
    pub(crate) fn build_sql_macro(&self) -> TokenStream2Result {
        let name = self.name();
        let item_fn = &self.item_fn;

        let register_body = match self.sql_macro_return_type()? {
            DuckSqlMacroResult::SqlMacro => quote! {
                let builder: quack_rs::prelude::SqlMacro = #name();
                use quack_rs::prelude::Registrar;
                unsafe { c.register_sql_macro(builder) }
            },
            DuckSqlMacroResult::ResultSqlMacro => quote! {
                let builder: duckfn::DuckResult<quack_rs::prelude::SqlMacro> = #name();
                use quack_rs::prelude::Registrar;
                unsafe { c.register_sql_macro(builder?) }
            },
            // 直接返回 SQL 字符串，经 duckfn::register_sql_macro_str 执行
            DuckSqlMacroResult::Str => quote! {
                let sql = #name();
                duckfn::register_sql_macro_str(c, &sql)
            },
            DuckSqlMacroResult::ResultStr => quote! {
                let sql = #name();
                duckfn::register_sql_macro_str(c, &sql?)
            },
        };

        Ok(quote! {
            #item_fn
            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn: |c| {
                        #register_body
                    }
                }
            }
        })
    }

    /// `#[duck_replacement_scan]`：把 `fn(path: &str) -> Option<String> / DuckOptionResult<String>`
    /// 变成一个 replacement scan 回调。
    ///
    /// 生成的同名模块里导出 `ReplacementScanImpl`（实现 `duckfn::ReplacementScanAdapter`）
    /// 和 `replacement_scan_register`，供 `#[duck_custom_register]` 手动注册使用。
    ///
    /// `#[duck_replacement_scan]`: turns
    /// `fn(path: &str) -> Option<String> / DuckOptionResult<String>` into a replacement-scan
    /// callback. The generated same-named module exports `ReplacementScanImpl` (implementing
    /// `duckfn::ReplacementScanAdapter`) and `replacement_scan_register` for manual registration
    /// via `#[duck_custom_register]`.
    pub(crate) fn build_replacement_scan(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let item_fn = &self.item_fn;
        let path_expr = self.replacement_scan_path_expr()?;
        let return_clause = self.build_replacement_scan_return_clause()?;
        let function_register = self.replacement_scan_register()?;

        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;

                pub struct ReplacementScanImpl;

                impl duckfn::ReplacementScanAdapter for ReplacementScanImpl{
                    const NAME: &'static str = stringify!(#name);

                    fn handle_path(path: &str) -> duckfn::DuckOptionResult<String> {
                        let result = #name(#path_expr);
                        #return_clause
                    }
                }

                /// 手动注册入口：在 `#[duck_custom_register]` 里调用。
                ///
                /// Manual registration entry point: call it inside `#[duck_custom_register]`.
                pub fn replacement_scan_register(
                    c: &quack_rs::prelude::Connection,
                ) -> duckfn::DuckResult<()> {
                    use duckfn::ReplacementScanAdapter;
                    ReplacementScanImpl::register(c)
                }

                #function_register
            }
        })
    }

    /// 生成 replacement scan 的自动注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the automatic registration for a replacement scan; produces nothing when
    /// `auto_register = false`.
    fn replacement_scan_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.inventory_submit(quote! {
            use duckfn::ReplacementScanAdapter;
            ReplacementScanImpl::register(c)
        })
    }

    /// 回调函数的唯一参数就是 DuckDB 传进来的表名/路径。
    ///
    /// 入参是引用（`&str`）时直接透传，是 `String` 时复制一份。
    ///
    /// The callback's single parameter is the table name/path passed in by DuckDB. A reference
    /// (`&str`) is forwarded as is, a `String` is copied.
    fn replacement_scan_path_expr(&self) -> TokenStream2Result {
        let args = self.args();
        if args.len() != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                REPLACEMENT_SCAN_SIGNATURE_HINT,
            ));
        }
        match args[0].resolve_type()? {
            // `path: &str`：直接透传
            Type::Reference(_) => Ok(quote! { path }),
            // `path: String`：复制一份
            _ => Ok(quote! { path.to_string() }),
        }
    }

    /// 按「可空性 + 字符串种类」四种组合生成 replacement scan 的返回收尾代码，
    /// 统一把结果收敛成 `DuckOptionResult<String>`。
    ///
    /// Generates the return epilogue of a replacement scan for the four combinations of
    /// nullability and string kind, always normalising the result into
    /// `DuckOptionResult<String>`.
    fn build_replacement_scan_return_clause(&self) -> TokenStream2Result {
        let (result_type, str_kind) = self.replacement_scan_return_type()?;
        Ok(match (result_type, str_kind) {
            (DuckReplacementScanResult::Option, DuckStrKind::Owned) => quote! { Ok(result) },
            (DuckReplacementScanResult::Option, DuckStrKind::Borrowed) => {
                quote! { Ok(result.map(::std::string::ToString::to_string)) }
            }
            (DuckReplacementScanResult::OptionResult, DuckStrKind::Owned) => quote! { result },
            (DuckReplacementScanResult::OptionResult, DuckStrKind::Borrowed) => {
                quote! { Ok(result?.map(::std::string::ToString::to_string)) }
            }
        })
    }

    /// 解析 replacement scan 的返回类型，得到「可空性 + 字符串种类」。
    ///
    /// 只接受 `Option<..>` 与 `DuckOptionResult<..>` 两种外层形式，内层必须是
    /// `String` / `&str`；否则报编译错误。
    ///
    /// Parses the replacement-scan return type into "nullability + string kind". Only
    /// `Option<..>` and `DuckOptionResult<..>` are accepted as the outer form and the inner type
    /// must be `String` / `&str`; otherwise a compile error is reported.
    fn replacement_scan_return_type(
        &self,
    ) -> syn::Result<(DuckReplacementScanResult, DuckStrKind)> {
        let ReturnType::Type(_, ty) = &self.item_fn.sig.output else {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.output,
                REPLACEMENT_SCAN_RETURN_TYPE_HINT,
            ));
        };

        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                let result_type = match segment.ident.to_string().as_str() {
                    "Option" => Some(DuckReplacementScanResult::Option),
                    "DuckOptionResult" => Some(DuckReplacementScanResult::OptionResult),
                    _ => None,
                };
                if let Some(result_type) = result_type {
                    let inner = extract_generic_arg_type(segment).ok_or_else(|| {
                        syn::Error::new_spanned(ty, REPLACEMENT_SCAN_RETURN_TYPE_HINT)
                    })?;
                    let str_kind = Self::replacement_scan_str_kind(inner).ok_or_else(|| {
                        syn::Error::new_spanned(ty, REPLACEMENT_SCAN_RETURN_TYPE_HINT)
                    })?;
                    return Ok((result_type, str_kind));
                }
            }
        }

        Err(syn::Error::new_spanned(
            &self.item_fn.sig.output,
            REPLACEMENT_SCAN_RETURN_TYPE_HINT,
        ))
    }

    /// 识别 `String` / `&'static str`（可空与不可空都用同一套收尾）。
    ///
    /// Recognises `String` / `&'static str` (both nullable and non-nullable share the same
    /// epilogue).
    fn replacement_scan_str_kind(ty: &Type) -> Option<DuckStrKind> {
        match ty {
            Type::Path(type_path) => type_path
                .path
                .segments
                .last()
                .filter(|s| s.ident == "String")
                .map(|_| DuckStrKind::Owned),
            Type::Reference(type_ref) => match &*type_ref.elem {
                Type::Path(type_path)
                    if type_path
                        .path
                        .segments
                        .last()
                        .is_some_and(|s| s.ident == "str") =>
                {
                    Some(DuckStrKind::Borrowed)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// `#[duck_cast_function]`：把 `fn(源值) -> 目标值` 变成一个 DuckDB cast 回调。
    ///
    /// 源类型来自唯一参数的 Rust 类型，目标类型来自返回类型（与 `duck_scalar_function`
    /// 的返回形式一致），注册成 `CAST(源 AS 目标)`：
    ///
    /// ```ignore
    /// #[duck_cast_function]
    /// fn my_cast(s: String) -> DuckOptionResult<i32> { s.parse().map_err(...) }
    /// ```
    ///
    /// `#[duck_cast_function]`: turns `fn(source) -> target` into a DuckDB cast callback. The
    /// source type comes from the single parameter's Rust type and the target type from the return
    /// type (using the same return shapes as `duck_scalar_function`), registered as
    /// `CAST(source AS target)`.
    pub(crate) fn build_cast_function(&self) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let item_fn = &self.item_fn;
        let (input_type, input_is_option) = self.cast_input_type()?;
        let (_, output_type) = self.scalar_return_type()?;
        let return_clause = self.build_scalar_return_clause()?;
        let implicit_cost = self.implicit_cost_override();
        let function_register = self.cast_function_register()?;

        // 入参写成 T 时 NULL 直接短路成 NULL（函数体不执行）；
        // 写成 Option<T> 时 NULL 以 None 进入函数体，语义由函数自己决定。
        let body = if input_is_option {
            quote! {
                let result = #name(value);
                #return_clause
            }
        } else {
            quote! {
                let Some(value) = value else {
                    return Ok(None);
                };
                let result = #name(value);
                #return_clause
            }
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

    /// 唯一参数就是「源类型的值」，`Option<T>` 表示允许把 NULL 带进函数体。
    ///
    /// 返回 `(源类型, 是否 Option)`；参数个数不为 1 时报编译错误。
    ///
    /// The single parameter is the source value; `Option<T>` allows NULL to reach the body.
    /// Returns `(source type, is_option)` and reports a compile error unless exactly one
    /// parameter is present.
    fn cast_input_type(&self) -> syn::Result<(Type, bool)> {
        let args = self.args();
        if args.len() != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                CAST_SIGNATURE_HINT,
            ));
        }
        let ty = args[0].resolve_type()?;
        if let Some(inner) = extract_option(ty) {
            return Ok((inner.clone(), true));
        }
        Ok((ty.clone(), false))
    }

    /// 生成 `implicit_cost()` 覆盖；未设置 `implicit_cost` 时输出空内容。
    ///
    /// Emits an `implicit_cost()` override; produces nothing when `implicit_cost` is unset.
    fn implicit_cost_override(&self) -> TokenStream2 {
        match self.duck_args.implicit_cost {
            Some(cost) => quote! {
                fn implicit_cost() -> Option<i64> {
                    Some(#cost)
                }
            },
            None => quote! {},
        }
    }

    /// 把 `duck_function_impl` 包进与函数同名的模块，并先插入参数结构体 `DuckArgsImpl`。
    ///
    /// Wraps `duck_function_impl` in a module named after the function, prepending the argument
    /// struct `DuckArgsImpl`.
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

    /// 由函数参数生成 `#[derive(duckfn::DuckStruct)]` 的参数结构体 `DuckArgsImpl`，
    /// 并把原始属性原样转写上去。
    ///
    /// Generates the `#[derive(duckfn::DuckStruct)]` argument struct `DuckArgsImpl` from the
    /// function parameters, writing the original attributes through unchanged.
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
    /// 生成标量函数实现体：`ScalarFunctionImpl` + 各种 builder 导出 + 自动注册。
    ///
    /// Generates the scalar implementation: `ScalarFunctionImpl`, the exported builders and the
    /// automatic registration.
    fn build_scalar_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let (_, return_type) = self.scalar_return_type()?;
        let return_clause = self.build_scalar_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;
        let function_register = self.scalar_function_register()?;
        let null_handling = self.null_handling_override();

        Ok(quote! {

            pub struct ScalarFunctionImpl;

            impl duckfn::ScalarFunctionAdapter for ScalarFunctionImpl{
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #return_type;

                #null_handling

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

            #function_register
        })
    }

    /// 生成标量函数的注册代码：`auto_register = false` 时空输出；设置 `overloads_name`
    /// 时提交重载项，否则注册为独立函数。
    ///
    /// Emits the scalar registration: nothing when `auto_register = false`, an overload item when
    /// `overloads_name` is set, otherwise a standalone function registration.
    fn scalar_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        // overloads_name：不注册自身函数名，改为提交重载项
        if let Some(set_name) = self.overloads_name() {
            return self.scalar_overload_submit(set_name);
        }
        self.common_inventory_submit(quote! {
            let builder = scalar_function_builder();
            unsafe { c.register_scalar(builder)}
        })
    }

    /// `overloads_name = "xxx"`：提交 `DuckScalarOverloadItem`，
    /// 由 `duckfn::register_all_scalar_overload` 按名字分组、用
    /// `ScalarFunctionSetBuilder` 注册成一个函数集（每个重载自带返回类型）。
    ///
    /// `overloads_name = "xxx"`: submits a `DuckScalarOverloadItem`; `duckfn::register_all_scalar_overload`
    /// then groups items by name and registers them as one function set through
    /// `ScalarFunctionSetBuilder` (each overload carries its own return type).
    fn scalar_overload_submit(&self, set_name: &str) -> TokenStream2Result {
        Ok(quote! {
            duckfn::inventory_submit! {
                duckfn::DuckScalarOverloadItem{
                    name: #set_name,
                    register_fn:|| {
                        use duckfn::ScalarFunctionAdapter;
                        ScalarFunctionImpl::scalar_overload_builder()
                    }
                }
            }
        })
    }

    /// 提交一条注册项，并预先引入 `Registrar` trait（注册方法需要它）。
    ///
    /// Submits one registration entry, pre-importing the `Registrar` trait required by the
    /// registration methods.
    fn common_inventory_submit(&self, content: TokenStream2) -> TokenStream2Result {
        self.inventory_submit(quote! {
            use quack_rs::prelude::Registrar;
            #content
        })
    }

    /// 提交一个 `DuckFunctionItem`，注册闭包体即 `content`（需自行引入 trait）。
    ///
    /// Submits one `DuckFunctionItem` whose registration closure body is `content` (the caller
    /// imports any required trait itself).
    fn inventory_submit(&self, content: TokenStream2) -> TokenStream2Result {
        Ok(quote! {
            duckfn::inventory_submit! {
                duckfn::DuckFunctionItem{
                    register_fn:|c|{
                        #content
                    }
                }
            }
        })
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
        let null_handling = self.null_handling_override();

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

    /// 生成表函数实现体：`TableFunctionImpl`（返回行迭代器）+ builder 导出 + 自动注册。
    ///
    /// Generates the table-function implementation: `TableFunctionImpl` (returning the row
    /// iterator), the builder export and the automatic registration.
    fn build_table_function_impl(&self) -> TokenStream2Result {
        let name = self.name();

        let (_, return_type) = self.table_return_type()?;
        let return_clause = self.build_table_return_clause()?;
        let get_data = self.args_to_code(|x| x.build_get_data())?;
        let function_register = self.table_function_register()?;

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

            #function_register

        })
    }

    /// 生成表函数的注册代码；`auto_register = false` 时输出空内容。
    ///
    /// Emits the table-function registration; produces nothing when `auto_register = false`.
    fn table_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.common_inventory_submit(quote! {
            let builder = table_function_builder()?;
            unsafe { c.register_table(builder)}
        })
    }

    /// 被标注函数的标识符（同时用作生成模块的名字）。
    ///
    /// The annotated function's identifier (also used as the generated module name).
    fn name(&self) -> &syn::Ident {
        &self.item_fn.sig.ident
    }

    /// 被标注函数的可见性，会原样应用到生成的模块上。
    ///
    /// The annotated function's visibility, applied verbatim to the generated module.
    fn visibility(&self) -> &syn::Visibility {
        &self.item_fn.vis
    }

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.duck_args.auto_register.unwrap_or(true)
    }

    /// `#[duck_scalar_function(overloads_name = "xxx")]` /
    /// `#[duck_aggregate_function(overloads_name = "xxx")]`
    ///
    /// 设置后不再注册自身的函数名，而是把本签名作为重载挂到 `xxx` 这个函数集上
    /// （同名重载由 `duckfn::register_all_*_overload` 分组注册）。
    /// 仍然受 `auto_register` 控制：`auto_register = false` 时完全不提交。
    ///
    /// `#[duck_scalar_function(overloads_name = "xxx")]` /
    /// `#[duck_aggregate_function(overloads_name = "xxx")]`. Once set, the function is not
    /// registered under its own name; this signature becomes an overload of the `xxx` function set
    /// (overloads sharing the name are grouped by `duckfn::register_all_*_overload`). It is still
    /// subject to `auto_register`: with `auto_register = false` nothing is submitted at all.
    fn overloads_name(&self) -> Option<&str> {
        self.duck_args.overloads_name.as_deref()
    }

    /// `#[duck_scalar_function(special_null_handling = true)]` /
    /// `#[duck_aggregate_function(special_null_handling = true)]`
    ///
    /// 是否开启 SpecialNullHandling，默认 `false`。
    ///
    /// `#[duck_scalar_function(special_null_handling = true)]` /
    /// `#[duck_aggregate_function(special_null_handling = true)]`. Whether to enable
    /// `SpecialNullHandling`; defaults to `false`.
    fn special_null_handling(&self) -> bool {
        self.duck_args.special_null_handling.unwrap_or(false)
    }

    /// 生成 `null_handling()` 覆盖：只有显式开启时才覆盖适配层默认值。
    ///
    /// 适配层的默认实现返回 `DefaultNullHandling`；开启后改成
    /// `SpecialNullHandling`，quack-rs 注册时会调用
    /// `duckdb_{scalar,aggregate}_function_set_special_handling`，
    /// 于是 NULL 行会进入回调（标量函数体是否真的看到 NULL，仍由参数是否
    /// 写成 `Option<T>` 决定）。
    ///
    /// Emits a `null_handling()` override, and only when explicitly enabled. The adapter default
    /// is `DefaultNullHandling`; once enabled it becomes `SpecialNullHandling`, so quack-rs calls
    /// `duckdb_{scalar,aggregate}_function_set_special_handling` and NULL rows reach the callback
    /// (whether the scalar body actually sees NULL still depends on whether the parameter is
    /// written as `Option<T>`).
    fn null_handling_override(&self) -> TokenStream2 {
        if !self.special_null_handling() {
            return quote! {};
        }
        quote! {
            fn null_handling() -> quack_rs::prelude::NullHandling {
                quack_rs::prelude::NullHandling::SpecialNullHandling
            }
        }
    }

    /// 收集函数的所有参数（含 `&mut State`）。
    ///
    /// Collects all parameters of the function (including `&mut State`).
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

    /// 对每个参数调用 `x` 生成代码，并收集成列表；任一参数出错则整体失败。
    ///
    /// Calls `x` for every parameter to generate code and collects the results; a failure on any
    /// parameter fails the whole list.
    fn args_to_code(
        &self,
        x: fn(&FnArgWrapper) -> TokenStream2Result,
    ) -> syn::Result<Vec<TokenStream2>> {
        self.args().iter().map(x).collect::<syn::Result<Vec<_>>>()
    }

    /// 解析标量函数的返回类型，得到「外层形式 + 内部类型」。
    ///
    /// 支持 `T`（Plain）、`Option<T>`、`DuckOptionResult<T>`；其他形式报编译错误。
    ///
    /// Parses a scalar return type into "outer form + inner type". `T` (plain), `Option<T>` and
    /// `DuckOptionResult<T>` are supported; anything else is a compile error.
    fn scalar_return_type(&self) -> syn::Result<(DuckScalarResult, &Type)> {
        if let ReturnType::Type(_, ty) = &self.item_fn.sig.output {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    let result_type = match segment.ident.to_string().as_str() {
                        "Option" => {
                            extract_generic_arg_type(segment)
                                .map(|inner| (DuckScalarResult::Option, inner))
                        }
                        "DuckOptionResult" => {
                            extract_generic_arg_type(segment)
                                .map(|inner| (DuckScalarResult::DuckOptionResult, inner))
                        }
                        _ => None,
                    };

                    if let Some((result_type, inner)) = result_type {
                        return Ok((result_type, inner));
                    }
                }

                return Ok((DuckScalarResult::Plain, &**ty));
            }
        }

        Err(syn::Error::new_spanned(
            self.item_fn.sig.output.to_owned(),
            "Only like `-> f64` `-> Option<f64>` `-> DuckOptionResult<f64>` is supported",
        ))
    }

    /// 解析表函数的返回类型，得到「外层形式 + 行类型」。
    ///
    /// 支持 `impl Iterator<Item = T>`、`DuckResult<impl Iterator<Item = T>>`、
    /// `DuckFullIteratorResult<T>`；其他形式报编译错误。
    ///
    /// Parses a table-function return type into "outer form + row type". `impl Iterator<Item = T>`,
    /// `DuckResult<impl Iterator<Item = T>>` and `DuckFullIteratorResult<T>` are supported;
    /// anything else is a compile error.
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
                            if let Some(GenericArgument::Type(Type::ImplTrait(impl_trait))) =
                                args.args.first()
                            {
                                if let Some(item) = iterator_item_type(impl_trait) {
                                    return Ok((DuckTableResult::ResultIterator, item));
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

    /// 被标注函数的原始返回类型。
    ///
    /// The annotated function's raw return type.
    fn return_type(&self) -> &ReturnType {
        &self.item_fn.sig.output
    }

    /// 按标量返回类型生成收尾代码，统一收敛成 `DuckOptionResult<Output>`。
    ///
    /// Generates the epilogue for a scalar return type, normalising it into
    /// `DuckOptionResult<Output>`.
    fn build_scalar_return_clause(&self) -> TokenStream2Result {
        let (result_type, _) = self.scalar_return_type()?;
        match result_type {
            DuckScalarResult::Plain => Ok(quote! { Ok(Some(result)) }),
            DuckScalarResult::Option => Ok(quote! { Ok(result) }),
            DuckScalarResult::DuckOptionResult => Ok(quote! { result }),
        }
    }

    /// 按表函数返回类型生成收尾代码，统一收敛成 `DuckFullIteratorResult<Output>`。
    ///
    /// Generates the epilogue for a table-function return type, normalising it into
    /// `DuckFullIteratorResult<Output>`.
    fn build_table_return_clause(&self) -> TokenStream2Result {
        let (result_type, _) = self.table_return_type()?;
        match result_type {
            DuckTableResult::Full => Ok(quote! { result }),
            DuckTableResult::ResultIterator => Ok(quote! { Ok(Box::new(result?.map(|x| Ok(Some(x))))) }),
            DuckTableResult::SimpleIterator => Ok(quote! { Ok(Box::new(result.map(|x| Ok(Some(x))))) }),
        }
    }

    /// 解析 `#[duck_sql_macro]` 的返回类型。
    ///
    /// Recognises `SqlMacro` / `DuckResult<SqlMacro>` / `String`（或 `&'static str`）/
    /// `DuckResult<...>`；其他形式报编译错误。
    ///
    /// Parses the return type of `#[duck_sql_macro]`. `SqlMacro` / `DuckResult<SqlMacro>` /
    /// `String` (or `&'static str`) / `DuckResult<...>` are recognised; anything else is a compile
    /// error.
    fn sql_macro_return_type(&self) -> syn::Result<DuckSqlMacroResult> {
        let ReturnType::Type(_, ty) = &self.item_fn.sig.output else {
            return Err(syn::Error::new_spanned(
                self.item_fn.sig.output.to_owned(),
                SQL_MACRO_RETURN_TYPE_HINT,
            ));
        };

        if Self::is_str_type(ty) {
            return Ok(DuckSqlMacroResult::Str);
        }

        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "SqlMacro" {
                    return Ok(DuckSqlMacroResult::SqlMacro);
                }
                if segment.ident == "DuckResult" {
                    if let Some(inner) = extract_generic_arg_type(segment) {
                        if Self::is_str_type(inner) {
                            return Ok(DuckSqlMacroResult::ResultStr);
                        }
                        if Self::is_sql_macro_type(inner) {
                            return Ok(DuckSqlMacroResult::ResultSqlMacro);
                        }
                    }
                }
            }
        }

        Err(syn::Error::new_spanned(
            self.item_fn.sig.output.to_owned(),
            SQL_MACRO_RETURN_TYPE_HINT,
        ))
    }

    /// 判断类型是否为字符串（`String` / `str` / `&str`，自动穿透引用）。
    ///
    /// Whether the type is a string (`String` / `str` / `&str`; references are followed).
    fn is_str_type(ty: &Type) -> bool {
        match ty {
            Type::Path(type_path) => type_path
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "String" || s.ident == "str"),
            Type::Reference(type_ref) => Self::is_str_type(&type_ref.elem),
            _ => false,
        }
    }

    /// 判断类型是否为 `SqlMacro`。
    ///
    /// Whether the type is `SqlMacro`.
    fn is_sql_macro_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "SqlMacro")
        )
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
        if let ReturnType::Default=self.item_fn.sig.output {
            return Ok(quote! {
                ;
                Ok(())
            });
        }
        Ok(quote! {})
    }
}

/// 函数单个参数的包装：提供参数名、参数类型、是否聚合状态等解析。
///
/// Wrapper around one function parameter: resolves the parameter name, its type and whether it is
/// the aggregate state.
struct FnArgWrapper {
    /// 被包装的函数参数。
    ///
    /// The wrapped function parameter.
    fn_arg: syn::FnArg,
}

impl FnArgWrapper {
    /// 取参数名；只支持 `foo: f64` 这种「标识符 + 类型」形式。
    ///
    /// Returns the parameter name; only the `foo: f64` (identifier + type) form is supported.
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

    /// 取参数类型。
    ///
    /// Returns the parameter type.
    fn resolve_type(&self) -> syn::Result<&syn::Type> {
        if let syn::FnArg::Typed(pat) = &self.fn_arg {
            return Ok(&*pat.ty);
        }
        Err(syn::Error::new_spanned(
            self.fn_arg.to_owned(),
            "Only like `foo: f64` is supported",
        ))
    }

    /// 取聚合状态类型：参数必须是可变引用（`&mut XxxState`），返回其中的 `XxxState`。
    ///
    /// Returns the aggregate-state type: the parameter must be a mutable reference
    /// (`&mut XxxState`) and the inner `XxxState` is returned.
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

    /// 判断该参数是否为聚合状态参数：是可变引用即为真。
    ///
    /// Whether this parameter is the aggregate state: true for a mutable reference.
    fn is_agg_state(&self) -> bool {
        if let Ok(syn::Type::Reference(type_ref)) = self.resolve_type() {
            return type_ref.mutability.is_some();
        }
        false
    }

    /// 生成 `DuckArgsImpl` 里的字段声明（聚合状态参数不生成字段）；参数名非法时报错。
    ///
    /// Generates the field declaration inside `DuckArgsImpl` (the aggregate-state parameter has no
    /// field); a compile error is reported for an invalid parameter name.
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

    /// 生成调用被标注函数时用的实参表达式：聚合状态取 `&mut self.state`，其余取 `args.#name`。
    ///
    /// Generates the argument expression used when calling the annotated function: the aggregate
    /// state becomes `&mut self.state` and the others `args.#name`.
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

/// 标量函数返回类型的外层形式。
///
/// The outer form of a scalar-function return type.
#[derive(Clone, Copy)]
enum DuckScalarResult {
    /// `-> T`：直接返回值，永不为 NULL。
    ///
    /// `-> T`: returns the value directly and is never NULL.
    Plain,
    /// `-> Option<T>`：`None` 表示 SQL NULL。
    ///
    /// `-> Option<T>`: `None` means SQL NULL.
    Option,
    /// `-> DuckOptionResult<T>`：可失败、可为 NULL。
    ///
    /// `-> DuckOptionResult<T>`: fallible and nullable.
    DuckOptionResult,
}

/// SQL 宏返回类型的外层形式。
///
/// The outer form of a SQL-macro return type.
#[derive(Clone, Copy)]
enum DuckSqlMacroResult {
    /// `-> SqlMacro`
    SqlMacro,
    /// `-> DuckResult<SqlMacro>`
    ResultSqlMacro,
    /// `-> String` / `-> &'static str`
    Str,
    /// `-> DuckResult<String>` / `-> DuckResult<&'static str>`
    ResultStr,
}

const SQL_MACRO_RETURN_TYPE_HINT: &str = r#"Only like
    `-> SqlMacro`: plain sql macro;
    `-> DuckResult<SqlMacro>`: handle input err;
    `-> String` / `-> &'static str`: plain sql string, executed directly;
    `-> DuckResult<String>` / `-> DuckResult<&'static str>`;
is supported"#;

/// 表函数返回类型的外层形式。
///
/// The outer form of a table-function return type.
#[derive(Clone, Copy)]
enum DuckTableResult {
    /// `-> DuckFullIteratorResult<T>`：构建与逐行都可能出错。
    ///
    /// `-> DuckFullIteratorResult<T>`: construction and every row may fail.
    Full,
    /// `-> DuckResult<impl Iterator<Item = T>>`：构建可能出错。
    ///
    /// `-> DuckResult<impl Iterator<Item = T>>`: construction may fail.
    ResultIterator,
    /// `-> impl Iterator<Item = T>`：最简单，不出错。
    ///
    /// `-> impl Iterator<Item = T>`: simplest, cannot fail.
    SimpleIterator,
}

/// `#[duck_replacement_scan]` 的返回形式：都带「管不管」的语义，
/// 因为 DuckDB 会对每个未解析的表名调用回调，回调必须能拒绝。
///
/// The return forms of `#[duck_replacement_scan]`, all carrying a "take over or not" meaning,
/// because DuckDB calls the callback for every unresolved table name and the callback must be able
/// to decline.
#[derive(Clone, Copy)]
enum DuckReplacementScanResult {
    /// `-> Option<String>` / `-> Option<&'static str>`
    Option,
    /// `-> DuckOptionResult<String>` / `-> DuckOptionResult<&'static str>`
    OptionResult,
}

/// 返回的表函数名是拥有所有权的 `String` 还是 `&'static str`。
///
/// Whether the returned table-function name is an owned `String` or a `&'static str`.
#[derive(Clone, Copy)]
enum DuckStrKind {
    /// `String`
    Owned,
    /// `&'static str`
    Borrowed,
}

const REPLACEMENT_SCAN_RETURN_TYPE_HINT: &str = r#"Only like
    `-> Option<String>` / `-> Option<&'static str>`: redirect when matched;
    `-> DuckOptionResult<String>` / `-> DuckOptionResult<&'static str>`: redirect when matched, may fail the query;
is supported"#;

const REPLACEMENT_SCAN_SIGNATURE_HINT: &str = r#"Only like
    `fn my_scan(path: &str) -> DuckOptionResult<String>`: takes the unresolved table name (usually a file path) and returns the table function to use;
is supported"#;

const CAST_SIGNATURE_HINT: &str = r#"Only like
    `fn my_cast(from: SourceType) -> TargetType`: the single argument is the source value, the return type is the target type;
    `fn my_cast(from: Option<SourceType>) -> TargetType`: NULL is passed in as None;
is supported"#;
