//! `#[duck_scalar_function]` 的代码生成实现。
//!
//! Code generation behind `#[duck_scalar_function]`.

use crate::common::{
    DuckDocArgs, DuckDocArgsProvider, FnArgWrapper, ItemFnWrapper, handle_duck_function,
    null_handling_override,
};
use crate::macro_utils::{TokenStream2Result, extract_generic_arg_type};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::__private::TokenStream2;
use syn::Type;

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
        // `varargs = true` 时最后一个参数是可变参数集合，不属于 `DuckArgsImpl`。
        //
        // With `varargs = true` the last parameter is the variadic collection and does not belong
        // to `DuckArgsImpl`.
        let (fixed_args, _) = self.split_varargs()?;
        let sql_name = self.sql_name(self.overloads_name());
        let duck_function_impl = self.build_scalar_function_impl()?;
        self.common_build(&fixed_args, None, &sql_name, duck_function_impl)
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

/// 取 `Vec<T>` 里的 `T`；类型不是 `Vec<...>`（或没有类型参数）时返回 `None`。
///
/// Returns the `T` of a `Vec<T>`; `None` when the type is not a `Vec<...>` or carries no type
/// argument.
fn vec_element_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Vec" {
        return None;
    }
    extract_generic_arg_type(segment)
}
