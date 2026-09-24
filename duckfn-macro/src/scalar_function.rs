//! `#[duck_scalar_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_scalar_function]`.

use crate::common::{
    DuckDocArgs, DuckDocArgsProvider, DuckScalarResult, FnArgWrapper, ItemFnWrapper,
    handle_duck_function, null_handling_override,
};
use crate::macro_utils::{TokenStream2Result, extract_generic_arg_type};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::__private::TokenStream2;
use syn::{ReturnType, Type};

/// `#[duck_scalar_function(...)]` 支持的全部参数。
///
/// 每个属性宏只声明自己认识、自己会用到的键；不在这里的键会直接变成编译错误。
///
/// Every argument `#[duck_scalar_function(...)]` accepts. Each attribute macro declares only the
/// keys it recognises and actually uses; any other key becomes a compile error.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckScalarFunctionArgs {
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

    /// `#[duck_scalar_function(volatile = true)]`
    ///
    /// 是否把标量函数标记为 volatile，默认 `false`。开启后注册期会调用
    /// `duckdb_scalar_function_set_volatile`：DuckDB 不缓存、不复用相同参数的调用结果，每一行都
    /// 重新求值（`random()` 这类函数需要它）。需要 duckfn 打开 `duckdb-1-5` feature，且不能与
    /// `overloads_name` 同用。
    pub(crate) volatile: Option<bool>,

    /// `#[duck_scalar_function(varargs = true)]`
    ///
    /// 是否开启可变参数（variadic arguments），默认 `false`。开启后函数签名的最后一个参数必须是
    /// `Vec<T>`，宏把 `T` 的逻辑类型交给 `duckdb_scalar_function_set_varargs`。需要 duckfn 打开
    /// `duckdb-1-5` feature，且不能与 `overloads_name` 同用。
    pub(crate) varargs: Option<bool>,

    /// `#[duck_scalar_function(overloads_name = "my_overloads")]`
    ///
    /// 指定重载函数集的名称：设置后不注册自身的函数名，只把本签名作为重载挂到该函数集上
    /// （同名重载由 `duckfn::register_all_scalar_overload` 分组注册）。
    pub(crate) overloads_name: Option<String>,

    /// `#[duck_scalar_function(batch = true)]`
    ///
    /// 是否开启批量模式，默认 `false`。开启后函数签名变成「整批进、整批出」：唯一参数是
    /// `Vec<MyRow>`（只收到非空行，空行由适配层按原位回填 NULL）或 `Vec<Option<MyRow>>`
    /// （空行以 `None` 交给函数自己处理），返回 `Vec<T>` / `Vec<Option<T>>` /
    /// `DuckOptionResult<Vec<T>>`（`Ok(None)` 表示整批 NULL）；`MyRow` 由用户
    /// `#[derive(DuckStruct)]` 定义，直接充当参数类型，宏不再生成 `DuckArgsImpl` 结构体。
    ///
    /// 与 `varargs = true` 互斥（可变参数没有稳定的行结构）。
    pub(crate) batch: Option<bool>,

    /// 文档参数：`description` / `comment` / `example`（`examples`）。
    ///
    /// Documentation arguments: `description` / `comment` / `example` (`examples`).
    #[darling(flatten)]
    pub(crate) doc: DuckDocArgs,
}

/// 让公共代码拿到 `#[duck_scalar_function]` 的文档参数。
///
/// Hands `#[duck_scalar_function]`'s documentation arguments to the shared code.
impl DuckDocArgsProvider for DuckScalarFunctionArgs {
    fn duck_doc(&self) -> DuckDocArgs {
        self.doc.clone()
    }
}

/// `#[duck_scalar_function]` 的入口：解析自己的参数后生成代码。
///
/// The `#[duck_scalar_function]` entry point: parses its own arguments and generates the code.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(attr, item, |wrapper: ItemFnWrapper<DuckScalarFunctionArgs>| {
        wrapper.build_scalar_function()
    })
}

impl ItemFnWrapper<DuckScalarFunctionArgs> {
    /// 生成标量函数：同名模块 + `ScalarFunctionImpl` + 自动注册（可按参数关闭）。
    ///
    /// Generates the scalar function: a same-named module, `ScalarFunctionImpl` and automatic
    /// registration (which can be disabled by arguments).
    pub(crate) fn build_scalar_function(&self) -> TokenStream2Result {
        if self.batch() {
            return self.build_batch_scalar_function();
        }
        // `varargs = true` 时最后一个参数是可变参数集合，不属于 `DuckArgsImpl`。
        //
        // With `varargs = true` the last parameter is the variadic collection and does not belong
        // to `DuckArgsImpl`.
        let (fixed_args, _) = self.split_varargs()?;
        let sql_name = self.sql_name(self.overloads_name());
        let duck_function_impl = self.build_scalar_function_impl()?;
        self.common_build(&fixed_args, None, &sql_name, duck_function_impl)
    }

    /// `#[duck_scalar_function(batch = true)]`
    ///
    /// 是否开启批量模式，默认 `false`。
    ///
    /// Whether batch mode is enabled; defaults to `false`.
    fn batch(&self) -> bool {
        self.args.batch.unwrap_or(false)
    }

    /// 生成标量函数实现体：`ScalarFunctionImpl` + 各种 builder 导出 + 自动注册。
    ///
    /// Generates the scalar implementation: `ScalarFunctionImpl`, the exported builders and the
    /// automatic registration.
    fn build_scalar_function_impl(&self) -> TokenStream2Result {
        let name = self.name();
        let (_, return_type) = self.scalar_return_type()?;
        let return_clause = self.build_scalar_return_clause()?;
        // `varargs = true` 时最后一个参数是可变参数集合，只有其余参数从 `DuckArgsImpl` 取值。
        //
        // With `varargs = true` the last parameter is the variadic collection; only the others are
        // read from `DuckArgsImpl`.
        let (fixed_args, varargs_element) = self.split_varargs()?;
        let get_data = fixed_args
            .iter()
            .map(|x| x.build_get_data())
            .collect::<syn::Result<Vec<_>>>()?;
        let varargs_methods =
            self.build_scalar_varargs_methods(&varargs_element, &get_data, &return_clause)?;
        // 可变参数函数走 `apply_varargs`，`apply` 只保留一个占位实现，避免生成缺少可变参数的调用。
        //
        // A variadic function goes through `apply_varargs`; `apply` keeps a placeholder body so
        // that no call missing the variadic argument is generated.
        let apply_body = if varargs_element.is_some() {
            quote! {
                fn apply(_args: Self::Args) -> duckfn::DuckOptionResult<Self::Output> {
                    unreachable!(
                        "`apply` is not used by variadic scalar functions: the adapter calls \
                         `apply_varargs` instead"
                    )
                }
            }
        } else {
            quote! {
                fn apply(args: Self::Args) -> duckfn::DuckOptionResult<Self::Output> {
                    let result = #name(
                        #(#get_data),*
                    );
                    #return_clause
                }
            }
        };
        let function_register = self.scalar_function_register()?;
        let null_handling = null_handling_override(self.special_null_handling());
        let volatile_override = self.volatile_override()?;

        Ok(quote! {

            pub struct ScalarFunctionImpl;

            impl duckfn::ScalarFunctionAdapter for ScalarFunctionImpl{
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #return_type;

                #null_handling

                #volatile_override

                #varargs_methods

                #apply_body
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

    /// 生成批量标量函数：`#[duck_scalar_function(batch = true)]`。
    ///
    /// 批量模式的签名与逐行版本完全不同 —— 唯一的参数就是「一整批行」，返回值就是「一整批结果」：
    ///
    /// ```ignore
    /// fn f(rows: Vec<Option<MyRow>>) -> DuckOptionResult<Vec<Option<String>>>
    /// fn f(rows: Vec<MyRow>) -> Vec<String>
    /// ```
    ///
    /// `MyRow` 是实现 `DuckColumns` 的行结构体（通常 `#[derive(DuckStruct)]`），直接充当参数类型，
    /// 因此宏**不再生成** `DuckArgsImpl` 结构体，只把它定义成 `MyRow` 的别名，`type Args` 的写法
    /// 与逐行版本保持一致。`apply` 只留一个 `unreachable!` 占位（与 varargs 同样的做法），真正被
    /// 调用的是适配层的 `apply_batch`。
    ///
    /// Generates a batch scalar function (`#[duck_scalar_function(batch = true)]`). The signature is
    /// a different one: the single parameter is the whole batch of rows and the return value is the
    /// whole batch of results. `MyRow` is a `DuckColumns` row struct (usually
    /// `#[derive(DuckStruct)]`) and *is* the argument type, so no `DuckArgsImpl` struct is generated
    /// — it becomes merely an alias for `MyRow`, keeping `type Args` spelled the same way as the
    /// per-row flavour. `apply` is a `unreachable!` placeholder (exactly as with varargs) and the
    /// adapter calls `apply_batch` instead.
    fn build_batch_scalar_function(&self) -> TokenStream2Result {
        if self.varargs() {
            return Err(syn::Error::new_spanned(
                self.name(),
                "`batch = true` cannot be combined with `varargs = true`: variadic arguments have \
                 no stable row structure, so there is no batch to hand over. Drop one of the two.",
            ));
        }
        let name = self.name();
        let input = self.batch_input()?;
        let output = self.batch_return()?;
        let row_type = &input.row_type;
        let element = &output.element;
        let body = self.build_batch_apply_body(&input, &output);
        let null_handling = null_handling_override(self.special_null_handling());
        let volatile_override = self.volatile_override()?;
        let function_register = self.scalar_function_register()?;
        let sql_name = self.sql_name(self.overloads_name());
        // 批量模式的参数类型是用户自己的行结构体，这里只留一个别名；`type Args = DuckArgsImpl`
        // 于是仍指向用户的 `MyRow`，生成代码的其余部分不用区分两种模式。
        //
        // In batch mode the argument type is the user's own row struct; this alias keeps
        // `type Args = DuckArgsImpl` pointing at `MyRow`, so the rest of the generated code needs no
        // special case.
        let duck_args = quote! {
            /// 批量模式下参数类型就是用户自己的行结构体，这里只留一个别名。
            ///
            /// In batch mode the argument type is the user's own row struct; this alias is all that
            /// is generated for it.
            pub type DuckArgsImpl = #row_type;
        };
        let duck_function_impl = quote! {
            pub struct ScalarFunctionImpl;

            impl duckfn::ScalarFunctionAdapter for ScalarFunctionImpl {
                const NAME: &'static str = stringify!(#name);
                type Args = DuckArgsImpl;
                type Output = #element;

                #null_handling

                #volatile_override

                fn apply(_args: Self::Args) -> duckfn::DuckOptionResult<Self::Output> {
                    unreachable!(
                        "`apply` is not used by batch scalar functions: the adapter calls \
                         `apply_batch` instead"
                    )
                }

                fn apply_batch(
                    rows: Vec<Option<Self::Args>>,
                    _extra: Option<&duckfn::DuckExtraInfo>,
                ) -> duckfn::DuckOptionResult<Vec<Option<Self::Output>>> {
                    #body
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
        };
        self.common_build_with_duck_args(duck_args, &sql_name, duck_function_impl)
    }

    /// 生成批量模式 `apply_batch` 的方法体。
    ///
    /// 两种入参形态共用「调用用户函数 → 校验行数 → 归一到 `Vec<Option<Output>>`」这条主干，
    /// 区别只在开头：`Vec<Option<MyRow>>` 原样把行交出去；`Vec<MyRow>` 先把空行剔出来、记下
    /// 下标，算完再按原位把 `None` 填回去（空值短路传播）。
    ///
    /// Generates the body of `apply_batch` in batch mode. Both input shapes share the spine — call
    /// the user function, validate the row count, normalise into `Vec<Option<Output>>` — and differ
    /// only in the prologue: `Vec<Option<MyRow>>` hands the rows over as they are, while
    /// `Vec<MyRow>` pulls the NULL rows out (recording their indexes) and splices `None` back in at
    /// the original positions once the results are in.
    fn build_batch_apply_body(&self, input: &BatchInput, output: &BatchReturn) -> TokenStream2 {
        let function = self.name();
        let row_type = &input.row_type;
        let element = &output.element;
        let call_arg = if input.nullable_rows {
            quote! { rows }
        } else {
            quote! { __duckfn_non_null_rows }
        };
        let call = quote! { #function(#call_arg) };
        // `-> DuckOptionResult<Vec<_>>`：`Err` 直接往上抛，`Ok(None)` 表示整批输出 NULL。
        //
        // `-> DuckOptionResult<Vec<_>>`: an `Err` propagates and `Ok(None)` means the whole batch is
        // NULL.
        let read_values = match output.outer {
            DuckScalarResult::DuckOptionResult => quote! {
                let __duckfn_values = match #call? {
                    Some(values) => values,
                    None => return Ok(None),
                };
            },
            DuckScalarResult::Plain | DuckScalarResult::Option => quote! {
                let __duckfn_values = #call;
            },
        };
        // 行数必须对齐：拿回多少行就要还回多少行。
        //
        // The row count must line up: as many rows out as were handed in.
        let expected_desc = if input.nullable_rows {
            "input rows"
        } else {
            "non-NULL input rows"
        };
        let length_error = format!(
            "{{}}: batch function returned {{}} rows, but received {{}} {expected_desc}"
        );
        let length_check = quote! {
            if __duckfn_values.len() != __duckfn_expected {
                return Err(duckfn::duck_error(format!(
                    #length_error,
                    SQL_NAME,
                    __duckfn_values.len(),
                    __duckfn_expected
                )));
            }
        };
        // 外层 `Option` 表示「这一行有结果」，逐行包一层 `Some`；行本身是 NULL 由外层 `None`
        // 表达（过滤形态回填时用）。元素类型自己可空（`Option<T>`）时多包一层也无妨：
        // `Option<Option<T>>` 与 `Option<T>` 等价，`Some(None)` 照样写 NULL。
        //
        // The outer `Option` means "this row has a result", so every row is wrapped in `Some`; a
        // NULL row is expressed by an outer `None` (used when splicing NULL rows back). An extra
        // layer is harmless when the element type is nullable itself: `Option<Option<T>>` is
        // equivalent to `Option<T>`, and `Some(None)` still writes NULL.
        let results = quote! {
            let __duckfn_results: Vec<Option<#element>> =
                __duckfn_values.into_iter().map(Some).collect();
        };
        if input.nullable_rows {
            quote! {
                let __duckfn_expected = rows.len();
                #read_values
                #length_check
                #results
                Ok(Some(__duckfn_results))
            }
        } else {
            quote! {
                let mut __duckfn_null_indexes: Vec<usize> = Vec::new();
                let mut __duckfn_non_null_rows: Vec<#row_type> = Vec::with_capacity(rows.len());
                for (__duckfn_index, __duckfn_row) in rows.into_iter().enumerate() {
                    match __duckfn_row {
                        Some(__duckfn_row) => __duckfn_non_null_rows.push(__duckfn_row),
                        None => __duckfn_null_indexes.push(__duckfn_index),
                    }
                }
                let __duckfn_expected = __duckfn_non_null_rows.len();
                #read_values
                #length_check
                #results
                let __duckfn_total = __duckfn_expected + __duckfn_null_indexes.len();
                let mut __duckfn_merged: Vec<Option<#element>> =
                    Vec::with_capacity(__duckfn_total);
                let mut __duckfn_values = __duckfn_results.into_iter();
                let mut __duckfn_null_indexes = __duckfn_null_indexes.into_iter().peekable();
                for __duckfn_index in 0..__duckfn_total {
                    if __duckfn_null_indexes.peek() == Some(&__duckfn_index) {
                        __duckfn_null_indexes.next();
                        __duckfn_merged.push(None);
                    } else {
                        __duckfn_merged.push(__duckfn_values.next().expect(
                            "the batch result count was checked against the non-NULL row count",
                        ));
                    }
                }
                Ok(Some(__duckfn_merged))
            }
        }
    }

    /// 解析批量模式的入参：必须**恰好一个**参数，且形如 `Vec<MyRow>` 或 `Vec<Option<MyRow>>`。
    ///
    /// 入参位置因此不需要额外配置：签名里只有一个参数，位置天然确定。
    ///
    /// Parses the batch-mode parameter: exactly one parameter, shaped `Vec<MyRow>` or
    /// `Vec<Option<MyRow>>`. Its position therefore needs no extra configuration — there is only
    /// one parameter, so the position is unambiguous.
    fn batch_input(&self) -> syn::Result<BatchInput> {
        let args = self.args();
        if args.len() != 1 {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                BATCH_SIGNATURE_HINT,
            ));
        }
        let ty = args[0].resolve_type()?;
        let element = vec_element_type(ty)
            .ok_or_else(|| syn::Error::new_spanned(ty, BATCH_SIGNATURE_HINT))?;
        let (row_type, nullable_rows) = match option_element_type(element) {
            Some(row_type) => (row_type.clone(), true),
            None => (element.clone(), false),
        };
        Ok(BatchInput {
            row_type,
            nullable_rows,
        })
    }

    /// 解析批量模式的返回类型，得到「外层形式 + 每一行的结果类型」。
    ///
    /// - `Vec<R>` / `Vec<Option<R>>`：外层 `Plain`；
    /// - `DuckOptionResult<Vec<R>>` / `DuckOptionResult<Vec<Option<R>>>`：外层 `DuckOptionResult`，
    ///   此时 `Ok(None)` 表示整批输出 NULL；
    ///
    /// `R` 就是 `Self::Output`（`Option` 的可空性由类型本身承载，与逐行版本一致）。
    ///
    /// Parses the batch-mode return type into "outer form + per-row result type": `Vec<R>` and
    /// `Vec<Option<R>>` are `Plain`, `DuckOptionResult<Vec<R>>` and
    /// `DuckOptionResult<Vec<Option<R>>>` are `DuckOptionResult` (where `Ok(None)` nulls the whole
    /// batch). `R` is `Self::Output` (nullability lives in the type itself, exactly as in the
    /// per-row flavour).
    fn batch_return(&self) -> syn::Result<BatchReturn> {
        let ReturnType::Type(_, ty) = self.return_type() else {
            return Err(syn::Error::new_spanned(
                self.return_type(),
                BATCH_SIGNATURE_HINT,
            ));
        };
        let (outer, vec_type) = match &**ty {
            Type::Path(type_path) => match type_path.path.segments.last() {
                Some(segment) if segment.ident == "DuckOptionResult" => (
                    DuckScalarResult::DuckOptionResult,
                    extract_generic_arg_type(segment).ok_or_else(|| {
                        syn::Error::new_spanned(ty, BATCH_SIGNATURE_HINT)
                    })?,
                ),
                _ => (DuckScalarResult::Plain, &**ty),
            },
            _ => (DuckScalarResult::Plain, &**ty),
        };
        let element = vec_element_type(vec_type)
            .ok_or_else(|| syn::Error::new_spanned(vec_type, BATCH_SIGNATURE_HINT))?;
        Ok(BatchReturn {
            outer,
            element: element.clone(),
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

    /// 是否自动注册，默认 `true`。
    ///
    /// Whether to auto-register; defaults to `true`.
    fn auto_register(&self) -> bool {
        self.args.auto_register.unwrap_or(true)
    }

    /// `#[duck_scalar_function(special_null_handling = true)]`
    ///
    /// 是否开启 SpecialNullHandling，默认 `false`。
    ///
    /// Whether to enable `SpecialNullHandling`; defaults to `false`.
    fn special_null_handling(&self) -> bool {
        self.args.special_null_handling.unwrap_or(false)
    }

    /// `#[duck_scalar_function(overloads_name = "xxx")]`
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

    /// `#[duck_scalar_function(volatile = true)]`
    ///
    /// 是否把标量函数标记为 volatile，默认 `false`。
    ///
    /// `#[duck_scalar_function(volatile = true)]`: whether to mark the scalar function volatile;
    /// defaults to `false`.
    fn volatile(&self) -> bool {
        self.args.volatile.unwrap_or(false)
    }

    /// 生成 `volatile()` 覆盖：只有显式开启时才覆盖适配层默认值。
    ///
    /// `volatile = true` 与 `overloads_name` 互斥：quack-rs 的 `ScalarOverloadBuilder` 没有暴露
    /// volatile 开关，同时写上只会让开关静默失效，因此在编译期直接报错。
    ///
    /// Emits a `volatile()` override, and only when explicitly enabled. `volatile = true` and
    /// `overloads_name` are mutually exclusive: quack-rs' `ScalarOverloadBuilder` exposes no
    /// volatile switch, so combining them would silently drop the flag, and is rejected at compile
    /// time instead.
    fn volatile_override(&self) -> TokenStream2Result {
        if !self.volatile() {
            return Ok(quote! {});
        }
        if self.overloads_name().is_some() {
            return Err(syn::Error::new_spanned(
                self.name(),
                "`volatile = true` cannot be combined with `overloads_name`: quack-rs' \
                 `ScalarOverloadBuilder` exposes no volatile switch, so the flag would be \
                 dropped silently. Register the function under its own name instead.",
            ));
        }
        Ok(quote! {
            fn volatile() -> bool {
                true
            }
        })
    }

    /// `#[duck_scalar_function(varargs = true)]`
    ///
    /// 是否开启可变参数（variadic arguments），默认 `false`。
    ///
    /// Whether to enable variadic arguments; defaults to `false`.
    fn varargs(&self) -> bool {
        self.args.varargs.unwrap_or(false)
    }

    /// 把参数拆成「固定参数 + 可变参数元素类型」。
    ///
    /// `varargs = false` 时原样返回全部参数与 `None`；`varargs = true` 时最后一个参数必须是
    /// `Vec<T>`，这里返回除它以外的参数与元素类型 `T`。同时拒绝与 `overloads_name` 组合
    /// （quack-rs 的重载 builder 没有暴露 varargs 开关，组合只会让开关静默失效）。
    ///
    /// Splits the parameters into "fixed arguments + variadic element type". With `varargs =
    /// false` every parameter is returned unchanged together with `None`; with `varargs = true`
    /// the last parameter must be `Vec<T>` and everything before it is returned along with the
    /// element type `T`. Combining the flag with `overloads_name` is rejected (quack-rs' overload
    /// builder exposes no varargs switch, so the combination would silently drop the flag).
    fn split_varargs(&self) -> syn::Result<(Vec<FnArgWrapper>, Option<Type>)> {
        let args = self.args();
        if !self.varargs() {
            return Ok((args, None));
        }
        if self.overloads_name().is_some() {
            return Err(syn::Error::new_spanned(
                self.name(),
                "`varargs = true` cannot be combined with `overloads_name`: quack-rs' \
                 `ScalarOverloadBuilder` exposes no varargs switch, so the flag would be dropped \
                 silently. Register the function under its own name instead.",
            ));
        }
        let Some((last, fixed)) = args.split_last() else {
            return Err(syn::Error::new_spanned(
                &self.item_fn.sig.inputs,
                "`varargs = true` requires at least one parameter, and the last one must be `Vec<T>`",
            ));
        };
        let last_type = last.resolve_type()?;
        let element = vec_element_type(last_type).ok_or_else(|| {
            syn::Error::new_spanned(
                last_type,
                "With `varargs = true` the last parameter must be `Vec<T>`, where `T` is the type \
                 of one variadic argument (use `Vec<Option<U>>` for nullable arguments or \
                 `Vec<Vec<U>>` when each argument is itself a LIST)",
            )
        })?;
        Ok((fixed.to_vec(), Some(element.clone())))
    }

    /// 生成 `varargs = true` 时适配层需要的方法：元素类型、可变列读取器与逐行求值。
    ///
    /// 逐行求值把固定参数读成 `DuckArgsImpl`，再把固定列之后的每一列按元素类型读成一个值，
    /// 组成 `Vec<T>` 传给被标注函数；任一非可空固定参数或元素为 NULL 时整行输出 NULL。
    ///
    /// Generates the adapter methods needed when `varargs = true`: the element type, the reader for
    /// a variadic column and the per-row evaluation. The latter reads the fixed arguments into
    /// `DuckArgsImpl` and every column after them as one element of the element type, feeding the
    /// collected `Vec<T>` to the annotated function; a NULL in any non-nullable fixed argument or
    /// element makes the whole row NULL.
    fn build_scalar_varargs_methods(
        &self,
        varargs_element: &Option<Type>,
        fixed_get_data: &[TokenStream2],
        return_clause: &TokenStream2,
    ) -> TokenStream2Result {
        let Some(element) = varargs_element else {
            return Ok(quote! {});
        };
        let name = self.name();
        Ok(quote! {
            fn varargs_element_type() -> Option<quack_rs::prelude::LogicalType> {
                Some(<#element as duckfn::DuckValueType>::logical_type())
            }

            fn varargs_create_reader(
                chunk: &quack_rs::prelude::DataChunk,
                column_index: usize,
            ) -> duckfn::DuckValueReader {
                <#element as duckfn::DuckValueType>::create_reader(chunk, column_index)
            }

            fn apply_varargs(
                readers: &[duckfn::DuckValueReader],
                row: usize,
                fixed_count: usize,
            ) -> duckfn::DuckOptionResult<Self::Output> {
                use duckfn::DuckValueType;
                let args = match <DuckArgsImpl as duckfn::DuckColumns>::read_columns(
                    &readers[..fixed_count],
                    row,
                ) {
                    Some(args) => args,
                    None => return Ok(None),
                };
                let _ = &args;
                let mut __duckfn_varargs: Vec<#element> =
                    Vec::with_capacity(readers.len() - fixed_count);
                for __duckfn_reader in &readers[fixed_count..] {
                    match <#element as duckfn::DuckValueType>::read_slot(__duckfn_reader, row) {
                        Some(__duckfn_value) => __duckfn_varargs.push(__duckfn_value),
                        None => return Ok(None),
                    }
                }
                let result = #name(#(#fixed_get_data,)* __duckfn_varargs);
                #return_clause
            }
        })
    }
}

/// 批量模式解析出来的入参：行结构体 + 空行是否进入用户代码。
///
/// The batch-mode parameter, once parsed: the row struct plus whether NULL rows reach the user code.
struct BatchInput {
    /// 一行的类型，即 `Vec<E>` 里的 `E`（`Option` 已剥掉）；它直接充当 `Self::Args`。
    ///
    /// The type of one row — the `E` of `Vec<E>` with any `Option` stripped. It becomes
    /// `Self::Args` directly.
    row_type: Type,
    /// 声明成 `Vec<Option<E>>` 时为 `true`：空行以 `None` 交给用户；否则空行先被剔出来、
    /// 结果再按原位回填 NULL。
    ///
    /// `true` when declared as `Vec<Option<E>>` (NULL rows reach the user as `None`); otherwise the
    /// NULL rows are pulled out first and their results spliced back as NULL.
    nullable_rows: bool,
}

/// 批量模式解析出来的返回值：外层形式 + 每一行的结果类型。
///
/// The batch-mode return value, once parsed: the outer form plus the per-row result type.
struct BatchReturn {
    /// `Vec<R>` 为 [`DuckScalarResult::Plain`]，`DuckOptionResult<Vec<R>>` 为
    /// [`DuckScalarResult::DuckOptionResult`]。
    ///
    /// [`DuckScalarResult::Plain`] for `Vec<R>`, [`DuckScalarResult::DuckOptionResult`] for
    /// `DuckOptionResult<Vec<R>>`.
    outer: DuckScalarResult,
    /// 每一行的结果类型，即 `Vec<R>` 里的 `R`（可以是 `Option<T>`）；它就是 `Self::Output`。
    ///
    /// The per-row result type — the `R` of `Vec<R>`, possibly `Option<T>`. It is `Self::Output`.
    element: Type,
}

/// `batch = true` 的函数签名要求（编译错误里直接展示给用户）。
///
/// The signature `batch = true` requires, shown verbatim in the compile error.
const BATCH_SIGNATURE_HINT: &str = r#"Only like
    `fn my_batch(rows: Vec<MyRow>) -> Vec<T>` or
    `fn my_batch(rows: Vec<Option<MyRow>>) -> DuckOptionResult<Vec<Option<T>>>`
is supported with `batch = true`: the single parameter is the whole batch of rows and the return
value is the whole batch of results. `MyRow` is a `DuckColumns` row struct (usually
`#[derive(DuckStruct)]`); `Vec<MyRow>` receives only the non-NULL rows (NULL rows are filled back in
at their original positions), while `Vec<Option<MyRow>>` receives the NULL rows as `None` and is
responsible for them itself. The return type may be `Vec<T>`, `Vec<Option<T>>`,
`DuckOptionResult<Vec<T>>` or `DuckOptionResult<Vec<Option<T>>>`."#;

/// 取 `Vec<T>` 里的 `T`；类型不是 `Vec<...>`（或没有类型参数）时返回 `None`。
///
/// Returns the `T` of a `Vec<T>`; `None` when the type is not a `Vec<...>` or carries no type
/// argument.
fn vec_element_type(ty: &Type) -> Option<&Type> {
    generic_element_type(ty, "Vec")
}

/// 取 `Option<T>` 里的 `T`；类型不是 `Option<...>`（或没有类型参数）时返回 `None`。
///
/// Returns the `T` of an `Option<T>`; `None` when the type is not an `Option<...>` or carries no
/// type argument.
fn option_element_type(ty: &Type) -> Option<&Type> {
    generic_element_type(ty, "Option")
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
