use crate::attr_args::DuckArgs;
use crate::macro_utils::{
    TokenStream2Result, extract_generic_arg_type, extract_option, iterator_item_type,
};
use quote::quote;
use syn::__private::TokenStream2;
use syn::{GenericArgument, ItemFn, PathArguments, ReturnType, Type};

pub struct ItemFnWrapper {
    pub item_fn: ItemFn,
    pub attr: TokenStream2,
    pub duck_args: DuckArgs,
}

impl ItemFnWrapper {

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

    fn common_inventory_submit(&self, content: TokenStream2) -> TokenStream2Result {
        self.inventory_submit(quote! {
            use quack_rs::prelude::Registrar;
            #content
        })
    }

    /// 提交一个 `DuckFunctionItem`，注册闭包体即 `content`（需自行引入 trait）。
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
            pub fn aggregate_function_guard() -> duckfn::AggregateFunctionGuard {
                use duckfn::AggregateFunctionAdapter;
                AggregateFunctionImpl::aggregate_function_guard()
            }

            #function_register
        })
    }

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

    fn table_function_register(&self) -> TokenStream2Result {
        if !self.auto_register() {
            return Ok(quote! {});
        }
        self.common_inventory_submit(quote! {
            let builder = table_function_builder()?;
            unsafe { c.register_table(builder)}
        })
    }

    fn name(&self) -> &syn::Ident {
        &self.item_fn.sig.ident
    }

    fn visibility(&self) -> &syn::Visibility {
        &self.item_fn.vis
    }

    fn auto_register(&self) -> bool {
        self.duck_args.auto_register.unwrap_or(true)
    }

    /// `#[duck_scalar_function(overloads_name = "xxx")]` /
    /// `#[duck_aggregate_function(overloads_name = "xxx")]`
    ///
    /// 设置后不再注册自身的函数名，而是把本签名作为重载挂到 `xxx` 这个函数集上
    /// （同名重载由 `duckfn::register_all_*_overload` 分组注册）。
    /// 仍然受 `auto_register` 控制：`auto_register = false` 时完全不提交。
    fn overloads_name(&self) -> Option<&str> {
        self.duck_args.overloads_name.as_deref()
    }

    /// `#[duck_scalar_function(special_null_handling = true)]` /
    /// `#[duck_aggregate_function(special_null_handling = true)]`
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

    fn is_sql_macro_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "SqlMacro")
        )
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
        if let Ok(syn::Type::Reference(type_ref)) = self.resolve_type() {
            return type_ref.mutability.is_some();
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

#[derive(Clone, Copy)]
enum DuckTableResult {
    Full,
    ResultIterator,
    SimpleIterator,
}

/// `#[duck_replacement_scan]` 的返回形式：都带「管不管」的语义，
/// 因为 DuckDB 会对每个未解析的表名调用回调，回调必须能拒绝。
#[derive(Clone, Copy)]
enum DuckReplacementScanResult {
    /// `-> Option<String>` / `-> Option<&'static str>`
    Option,
    /// `-> DuckOptionResult<String>` / `-> DuckOptionResult<&'static str>`
    OptionResult,
}

/// 返回的表函数名是拥有所有权的 `String` 还是 `&'static str`。
#[derive(Clone, Copy)]
enum DuckStrKind {
    Owned,
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

