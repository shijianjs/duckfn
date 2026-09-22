//! `#[duck_*]` 属性宏的公共脚手架。
//!
//! 每个属性宏的实现都在自己的文件里（`scalar_function.rs`、`aggregate_function.rs`、
//! `table_function.rs`、……），本文件只保留它们共用的部分：
//!
//! - [`handle_duck_function`]：统一的入口调度 —— 解析被标注函数与各自参数，再交给该宏的生成函数；
//! - [`ItemFnWrapper`]：被标注函数的包装，各宏通过 `impl ItemFnWrapper<自己的参数>` 扩展；
//! - [`FnArgWrapper`]：单个函数参数的包装；
//! - [`CreateTypeMode`]：两个 derive 宏共用的 `create_type` 取值；
//! - 共用的返回值解析（标量返回类型）与 `DuckArgsImpl` 生成。
//!
//! Shared scaffolding for the `#[duck_*]` attribute macros. Every macro's implementation lives in
//! its own file (`scalar_function.rs`, `aggregate_function.rs`, `table_function.rs`, ...) and this
//! module keeps only what they share: the common dispatch [`handle_duck_function`], the function
//! wrapper [`ItemFnWrapper`] (extended by each macro through `impl ItemFnWrapper<its own args>`),
//! the argument wrapper [`FnArgWrapper`], the [`CreateTypeMode`] value shared by the two derives,
//! plus the shared return-type parsing and `DuckArgsImpl` generation.

use crate::macro_utils::{
    TokenStream2Result, extract_generic_arg_type, handle_token_stream2_result,
};
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::__private::TokenStream2;
use syn::parse::Parse;
use syn::{ItemFn, ReturnType, Type, parse_macro_input};

/// 所有 `#[duck_*]` 属性宏的公共入口。
///
/// 流程：把被标注的函数解析成 [`ItemFn`]、把属性参数解析成各宏自己的 `A`（泛型参数），组装
/// [`ItemFnWrapper`] 后交给 `run` 做该宏特有的代码生成；解析错误会直接变成编译错误。
///
/// 参数类型 `A` 由调用方指定（每个属性宏一个，只声明自己需要的键），因此这里不再有一个「放之四海
/// 而皆准」的参数结构体。
///
/// Common entry point of every `#[duck_*]` attribute macro. It parses the annotated function into
/// an [`ItemFn`] and the attribute arguments into the macro's own `A`, assembles an
/// [`ItemFnWrapper`] and hands it to `run` for macro-specific code generation; parse errors become
/// compile errors directly. The argument type `A` is chosen by the caller (one per attribute macro,
/// declaring only the keys it actually needs), so there is no longer a single one-size-fits-all
/// argument struct.
pub(crate) fn handle_duck_function<A, F>(attr: TokenStream, item: TokenStream, run: F) -> TokenStream
where
    A: Parse,
    F: FnOnce(ItemFnWrapper<A>) -> TokenStream2Result,
{
    let input = parse_macro_input!(item as ItemFn);
    let args: A = match syn::parse(attr) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error().into(),
    };

    let wrapper = ItemFnWrapper {
        item_fn: input,
        args,
    };
    handle_token_stream2_result(run(wrapper))
}

/// 被标注函数的包装：原始函数 + 该宏各自解析出的参数 `A`。
///
/// 各属性宏在各自的文件里写 `impl ItemFnWrapper<自己的参数>`，只实现自己需要的生成逻辑。
///
/// Wrapper around the annotated function: the original item plus the arguments `A` parsed by that
/// macro. Every attribute macro adds `impl ItemFnWrapper<its own args>` in its own file and
/// implements only the generation it needs.
pub(crate) struct ItemFnWrapper<A> {
    /// 被 `#[duck_*]` 标注的原始函数（会原样输出到生成代码里）。
    ///
    /// The original function annotated with `#[duck_*]` (emitted verbatim in the generated code).
    pub(crate) item_fn: ItemFn,
    /// 该宏自己解析出的属性参数。
    ///
    /// The attribute arguments parsed by this macro.
    pub(crate) args: A,
}

impl<A> ItemFnWrapper<A> {
    /// 被标注函数的标识符（同时用作生成模块的名字）。
    ///
    /// The annotated function's identifier (also used as the generated module name).
    pub(crate) fn name(&self) -> &syn::Ident {
        &self.item_fn.sig.ident
    }

    /// 被标注函数的可见性，会原样应用到生成的模块上。
    ///
    /// The annotated function's visibility, applied verbatim to the generated module.
    pub(crate) fn visibility(&self) -> &syn::Visibility {
        &self.item_fn.vis
    }

    /// 被标注函数的原始返回类型。
    ///
    /// The annotated function's raw return type.
    pub(crate) fn return_type(&self) -> &ReturnType {
        &self.item_fn.sig.output
    }

    /// 该签名注册进 DuckDB 时真正使用的 SQL 名字。
    ///
    /// 设了 `overloads_name` 时是函数集名（此时本签名不注册自己的函数名），否则就是函数名。
    /// 生成的 `SQL_NAME` 常量与它一致。
    ///
    /// The SQL name this signature is actually registered under: the function-set name when
    /// `overloads_name` is set (the signature is then not registered under its own name), the
    /// function name otherwise. The generated `SQL_NAME` constant holds the same value.
    pub(crate) fn sql_name(&self, overloads_name: Option<&str>) -> String {
        match overloads_name {
            Some(name) => name.to_string(),
            None => self.name().to_string(),
        }
    }

    /// 收集函数的所有参数（含 `&mut State`）。
    ///
    /// Collects all parameters of the function (including `&mut State`).
    pub(crate) fn args(&self) -> Vec<FnArgWrapper> {
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
    pub(crate) fn args_to_code(
        &self,
        x: fn(&FnArgWrapper) -> TokenStream2Result,
    ) -> syn::Result<Vec<TokenStream2>> {
        self.args().iter().map(x).collect::<syn::Result<Vec<_>>>()
    }

    /// 提交一个 `DuckFunctionItem`，注册闭包体即 `content`（需自行引入 trait）。
    ///
    /// Submits one `DuckFunctionItem` whose registration closure body is `content` (the caller
    /// imports any required trait itself).
    pub(crate) fn inventory_submit(&self, content: TokenStream2) -> TokenStream2Result {
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

    /// 提交一条注册项，并预先引入 `Registrar` trait（注册方法需要它）。
    ///
    /// Submits one registration entry, pre-importing the `Registrar` trait required by the
    /// registration methods.
    pub(crate) fn common_inventory_submit(&self, content: TokenStream2) -> TokenStream2Result {
        self.inventory_submit(quote! {
            use quack_rs::prelude::Registrar;
            #content
        })
    }

    /// 把 `duck_function_impl` 包进与函数同名的模块，并先插入参数结构体 `DuckArgsImpl` 与
    /// SQL 注册名常量 `SQL_NAME`。
    ///
    /// `fields` 是参与参数结构体的参数（标量函数的可变参数集合不入内），`named_param_from` 会
    /// 转写成 `#[duck(named_param_from = "...")]` —— 各宏只把 derive 宏真正需要的键透传过去。
    /// `sql_name` 是该函数注册进 DuckDB 时真正用的名字，由各宏给出（设了 `overloads_name` 时是
    /// 函数集名，否则是函数名）。
    ///
    /// Wraps `duck_function_impl` in a module named after the function, prepending the argument
    /// struct `DuckArgsImpl` and the `SQL_NAME` constant holding the SQL registration name.
    /// `fields` are the parameters taking part in it (a scalar function's variadic collection is
    /// left out) and `named_param_from` is written as `#[duck(named_param_from = "...")]` — each
    /// macro forwards only the keys the derive macro actually needs. `sql_name` is the name the
    /// function is really registered under, supplied by each macro (the function-set name when
    /// `overloads_name` is set, the function name otherwise).
    pub(crate) fn common_build(
        &self,
        fields: &[FnArgWrapper],
        named_param_from: Option<&str>,
        sql_name: &str,
        duck_function_impl: TokenStream2,
    ) -> TokenStream2Result {
        let name = self.name();
        let vis = self.visibility();
        let duck_args = build_duck_args(fields, named_param_from)?;
        let item_fn = &self.item_fn;
        Ok(quote! {
            #item_fn

            #vis mod #name{
                use super::*;

                /// 本函数注册到 DuckDB 时使用的 SQL 名字。
                ///
                /// 与 `NAME` 的区别：`NAME` 是 Rust 函数名（只用于标识回调），当签名通过
                /// `overloads_name = "..."` 挂到函数集上时，`NAME` 与真正的 SQL 名字并不相同。
                /// 错误信息前缀、日志、以及需要在别处引用这个函数名时，读这里。
                ///
                /// The SQL name this function is registered under. Unlike `NAME`, which is the Rust
                /// function name (only used to identify the callback), this is the name SQL actually
                /// uses — and it differs from `NAME` when the signature is attached to a function
                /// set through `overloads_name = "..."`. Read this for error prefixes, logging, or
                /// whenever the function name is needed elsewhere.
                pub const SQL_NAME: &str = #sql_name;

                #duck_args

                #duck_function_impl
            }
        })
    }

    /// 取返回类型 `DuckResult<T>` 里的 `T`；外层不是 `DuckResult` 时返回 `None`。
    ///
    /// Returns the `T` of a `DuckResult<T>` return type, or `None` when the outer type is something
    /// else.
    pub(crate) fn duck_result_inner(&self) -> Option<&Type> {
        if let ReturnType::Type(_, ty) = &self.item_fn.sig.output {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "DuckResult" {
                        return extract_generic_arg_type(segment);
                    }
                }
            }
        }
        None
    }

    /// 解析标量函数的返回类型，得到「外层形式 + `Output` 类型」。
    ///
    /// - `T`（Plain，含定长数组 `[T; N]`）：`Output = T`；
    /// - `Option<T>`：`Output = Option<T>` —— 可空性由类型表达，`None` 就是 SQL NULL；
    /// - `DuckOptionResult<T>`：`Output = T` —— `DuckOptionResult` 不是值类型，只表示「可失败」。
    ///
    /// 其他形式报编译错误。`#[duck_scalar_function]` 与 `#[duck_cast_function]` 共用。
    ///
    /// Parses a scalar return type into "outer form + `Output` type": `T` (plain, including the
    /// fixed-size array `[T; N]`) keeps `T`, `Option<T>` keeps `Option<T>` (nullability expressed
    /// by the type) and `DuckOptionResult<T>` takes the inner `T` (`DuckOptionResult` is not a
    /// value type, it only means "fallible"). Anything else is a compile error. Shared by
    /// `#[duck_scalar_function]` and `#[duck_cast_function]`.
    pub(crate) fn scalar_return_type(&self) -> syn::Result<(DuckScalarResult, &Type)> {
        if let ReturnType::Type(_, ty) = &self.item_fn.sig.output {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    let result_type = match segment.ident.to_string().as_str() {
                        "Option" => Some(DuckScalarResult::Option),
                        "DuckOptionResult" => Some(DuckScalarResult::DuckOptionResult),
                        _ => None,
                    };

                    if let Some(result_type) = result_type {
                        let inner = extract_generic_arg_type(segment).ok_or_else(|| {
                            syn::Error::new_spanned(
                                ty,
                                "Only like `-> f64` `-> Option<f64>` `-> DuckOptionResult<f64>` is supported",
                            )
                        })?;
                        // `Option<T>` 自己就是值类型，`Output` 保持 `Option<T>`；
                        // `DuckOptionResult<T>` 只表示可失败，`Output` 取内层 `T`。
                        //
                        // `Option<T>` is a value type of its own, so `Output` stays `Option<T>`;
                        // `DuckOptionResult<T>` only means "fallible", so `Output` is the inner `T`.
                        let output = if matches!(result_type, DuckScalarResult::Option) {
                            &**ty
                        } else {
                            inner
                        };
                        return Ok((result_type, output));
                    }
                }
            }

            // 定长数组 `[T; N]` 不是一个 `Type::Path`（`DuckArray<T, N>` 才是它的别名），
            // 但同样是普通值类型，因此和 `Type::Path` 一样按 `Plain` 处理。
            //
            // A fixed-size array `[T; N]` is not a `Type::Path` (that is what the `DuckArray<T, N>`
            // alias is for) but is an ordinary value type all the same, so it is treated as
            // `Plain` just like `Type::Path`.
            if matches!(&**ty, Type::Path(_) | Type::Array(_)) {
                return Ok((DuckScalarResult::Plain, &**ty));
            }
        }

        Err(syn::Error::new_spanned(
            self.item_fn.sig.output.to_owned(),
            "Only like `-> f64` `-> Option<f64>` `-> DuckOptionResult<f64>` is supported",
        ))
    }

    /// 按标量返回类型生成收尾代码，统一收敛成 `DuckOptionResult<Output>`。
    ///
    /// `-> T` 与 `-> Option<T>` 的 `Output` 就是声明类型本身，因此都是 `Ok(Some(result))`
    /// —— `Option<T>` 的 `None` 会在写向量时变成 SQL NULL；`-> DuckOptionResult<T>` 直接
    /// 把结果（可失败、可为 NULL）交给适配层。
    ///
    /// Generates the epilogue for a scalar return type, normalising it into
    /// `DuckOptionResult<Output>`. For `-> T` and `-> Option<T>` the `Output` is the declared type
    /// itself, hence `Ok(Some(result))` in both cases — `Option<T>`'s `None` turns into SQL NULL
    /// while writing the vector. `-> DuckOptionResult<T>` hands the (fallible, nullable) result
    /// straight to the adapter.
    pub(crate) fn build_scalar_return_clause(&self) -> TokenStream2Result {
        let (result_type, _) = self.scalar_return_type()?;
        match result_type {
            DuckScalarResult::Plain | DuckScalarResult::Option => Ok(quote! { Ok(Some(result)) }),
            DuckScalarResult::DuckOptionResult => Ok(quote! { result }),
        }
    }
}

/// 由函数参数生成 `#[derive(duckfn::DuckStruct)]` 的参数结构体 `DuckArgsImpl`。
///
/// ``named_param_from`` 会转写成 `#[duck(named_param_from = "...")]`；除此之外**不再**把属性宏
/// 收到的参数原样透传给 derive 宏 —— 每个宏只声明自己需要的键，derive 宏也只解析自己认识的键。
///
/// Generates the `#[derive(duckfn::DuckStruct)]` argument struct `DuckArgsImpl` from the function
/// parameters. `named_param_from` becomes `#[duck(named_param_from = "...")]`; apart from that the
/// attribute macro's arguments are **no longer** written through to the derive macro — every macro
/// declares only the keys it needs and the derive parses only the keys it knows.
fn build_duck_args(fields: &[FnArgWrapper], named_param_from: Option<&str>) -> TokenStream2Result {
    let fields = fields
        .iter()
        .map(|x| x.build_duck_args_field())
        .collect::<syn::Result<Vec<_>>>()?;
    let duck_attr = match named_param_from {
        Some(field) => quote! { #[duck(named_param_from = #field)] },
        None => quote! {},
    };

    Ok(quote! {
        #[derive(duckfn::DuckStruct, Debug, Clone, Default)]
        #duck_attr
        pub struct DuckArgsImpl{
            #(#fields)*
        }
    })
}

/// 生成 `null_handling()` 覆盖：只有显式开启时才覆盖适配层默认值。
///
/// 适配层的默认实现返回 `DefaultNullHandling`；开启后改成 `SpecialNullHandling`，quack-rs 注册时
/// 会调用 `duckdb_{scalar,aggregate}_function_set_special_handling`，于是 NULL 行会进入回调
/// （标量函数体是否真的看到 NULL，仍由参数是否写成 `Option<T>` 决定）。
///
/// Emits a `null_handling()` override, and only when explicitly enabled. The adapter default is
/// `DefaultNullHandling`; once enabled it becomes `SpecialNullHandling`, so quack-rs calls
/// `duckdb_{scalar,aggregate}_function_set_special_handling` and NULL rows reach the callback
/// (whether the scalar body actually sees NULL still depends on whether the parameter is written
/// as `Option<T>`).
pub(crate) fn null_handling_override(enabled: bool) -> TokenStream2 {
    if !enabled {
        return quote! {};
    }
    quote! {
        fn null_handling() -> quack_rs::prelude::NullHandling {
            quack_rs::prelude::NullHandling::SpecialNullHandling
        }
    }
}

/// 标量函数返回类型的外层形式（`#[duck_scalar_function]` / `#[duck_cast_function]` 共用）。
///
/// The outer form of a scalar-function return type (shared by `#[duck_scalar_function]` and
/// `#[duck_cast_function]`).
#[derive(Clone, Copy)]
pub(crate) enum DuckScalarResult {
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

/// `#[duck(create_type = ...)]` 的取值（`#[derive(DuckStruct)]` / `#[derive(DuckEnum)]` 共用）。
///
/// 「打印」模式（`create_type = "print"`）只把宏会执行的 DDL 收进队列、**不**进 catalog；等全部
/// 注册项跑完，入口点把这一批一次性打到 stderr —— 可以先看看渲染出来的 SQL 长什么样、再决定要不要
/// 真的建类型，也可以把语句抄走自己执行。
///
/// The values `#[duck(create_type = ...)]` accepts (shared by `#[derive(DuckStruct)]` and
/// `#[derive(DuckEnum)]`). The print mode (`create_type = "print"`) queues the very DDL the macro
/// would run without touching the catalog, and the entry point prints the collected batch in one
/// block once every registration has run — so the rendered statements can be inspected and copied
/// before committing to them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CreateTypeMode {
    /// `create_type = false`（默认）：不建类型、不打印。
    ///
    /// `create_type = false` (the default): neither create nor print.
    #[default]
    Off,
    /// `create_type = true`：加载期执行 `CREATE TYPE IF NOT EXISTS ...`。
    ///
    /// `create_type = true`: run `CREATE TYPE IF NOT EXISTS ...` at load time.
    Create,
    /// `create_type = "print"`：把 `CREATE TYPE IF NOT EXISTS ...` 收进队列，由入口点统一打印；不建类型。
    ///
    /// `create_type = "print"`: queue `CREATE TYPE IF NOT EXISTS ...` for the entry point to print
    /// as one block; the type is not created.
    Print,
}

impl FromMeta for CreateTypeMode {
    /// `create_type = true` / `create_type = false`。
    ///
    /// `create_type = true` / `create_type = false`.
    fn from_bool(value: bool) -> darling::Result<Self> {
        Ok(if value {
            CreateTypeMode::Create
        } else {
            CreateTypeMode::Off
        })
    }

    /// `create_type = "print"`。
    ///
    /// `create_type = "print"`.
    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(match value {
            "print" => CreateTypeMode::Print,
            other => return Err(darling::Error::unknown_value(other)),
        })
    }
}

/// 函数单个参数的包装：提供参数名、参数类型、是否聚合状态等解析。
///
/// Wrapper around one function parameter: resolves the parameter name, its type and whether it is
/// the aggregate state.
#[derive(Clone)]
pub(crate) struct FnArgWrapper {
    /// 被包装的函数参数。
    ///
    /// The wrapped function parameter.
    fn_arg: syn::FnArg,
}

impl FnArgWrapper {
    /// 取参数名；只支持 `foo: f64` 这种「标识符 + 类型」形式。
    ///
    /// Returns the parameter name; only the `foo: f64` (identifier + type) form is supported.
    pub(crate) fn name(&self) -> syn::Result<&syn::Ident> {
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
    pub(crate) fn resolve_type(&self) -> syn::Result<&syn::Type> {
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
    pub(crate) fn resolve_state_type(&self) -> syn::Result<&syn::Type> {
        let ty = self.resolve_type()?;

        let syn::Type::Reference(type_ref) = ty else {
            return Err(syn::Error::new_spanned(
                ty,
                "The aggregate state parameter must be a mutable reference like `&mut MyState`",
            ));
        };

        if type_ref.mutability.is_none() {
            return Err(syn::Error::new_spanned(
                ty,
                "The aggregate state parameter must be mutable: `&mut MyState`",
            ));
        }

        Ok(&type_ref.elem)
    }

    /// 判断该参数是否为聚合状态参数：是可变引用即为真。
    ///
    /// Whether this parameter is the aggregate state: true for a mutable reference.
    pub(crate) fn is_agg_state(&self) -> bool {
        if let Ok(syn::Type::Reference(type_ref)) = self.resolve_type() {
            return type_ref.mutability.is_some();
        }
        false
    }

    /// 生成 `DuckArgsImpl` 里的字段声明（聚合状态参数不生成字段）；参数名非法时报错。
    ///
    /// Generates the field declaration inside `DuckArgsImpl` (the aggregate-state parameter has no
    /// field); a compile error is reported for an invalid parameter name.
    pub(crate) fn build_duck_args_field(&self) -> TokenStream2Result {
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
    pub(crate) fn build_get_data(&self) -> TokenStream2Result {
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
