//! 属性宏的参数解析与公共调度逻辑。
//!
//! Argument parsing and common dispatch logic for the attribute macros.

use proc_macro::TokenStream;
use darling::FromMeta;
use syn::{parse_macro_input, ItemFn};
use crate::duck_function::ItemFnWrapper;
use crate::macro_utils::{handle_token_stream2_result, TokenStream2Result};

/// 所有 `#[duck_*]` 属性宏的公共入口。
///
/// 流程：把被标注的函数解析成 [`ItemFn`]、把属性参数解析成 [`DuckFunctionMacroArgs`]，组装
/// [`ItemFnWrapper`] 后交给 `run` 做各宏特有的代码生成；解析错误会直接变成编译错误。
///
/// Common entry point of every `#[duck_*]` attribute macro. It parses the annotated function
/// into an [`ItemFn`] and the attribute arguments into [`DuckFunctionMacroArgs`], assembles an
/// [`ItemFnWrapper`] and hands it to `run` for macro-specific code generation; parse errors
/// become compile errors directly.
pub fn handle_duck_function(
    _attr: TokenStream,
    item: TokenStream,
    run: fn(ItemFnWrapper) -> TokenStream2Result,
) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let duck_args: DuckFunctionMacroArgs = match syn::parse(_attr.clone()) {
        Ok(v) => v,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };

    let wrapper = ItemFnWrapper {
        item_fn:input,
        attr: _attr.into(),
        duck_args,
    };
    let result = run(wrapper);
    handle_token_stream2_result(result)
}

/// `#[duck(...)]` / `#[duck_*(...)]` 里可用的全部参数。
///
/// 这是 `#[duck(...)]` 参数的**唯一配置来源**：属性宏直接用它解析函数上的属性；
/// `#[derive(DuckStruct)]` 则通过 `#[darling(flatten)]` 复用同一个结构体，解析被
/// 写穿到 `DuckArgsImpl` 上的 `#[duck(...)]`，因此新增参数只需在这里写一次。
///
/// All arguments accepted by `#[duck(...)]` / `#[duck_*(...)]`. This is the **single source of
/// truth** for `#[duck(...)]` arguments: the attribute macros parse the function attributes with
/// it directly, while `#[derive(DuckStruct)]` reuses the very same struct through
/// `#[darling(flatten)]` to parse the `#[duck(...)]` written through onto `DuckArgsImpl`, so a new
/// argument only has to be declared once.
#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckFunctionMacroArgs {
    /// 表函数的命名参数从哪个开始
    ///
    /// The field name from which table-function named parameters start.
    pub named_param_from: Option<String>,

    /// Whether to auto register the function
    /// - Default to true
    ///
    /// 是否自动注册，默认 `true`；设为 `false` 时只生成 builder，交给
    /// `#[duck_custom_register]` 手动注册。
    ///
    /// Whether to auto-register the function; defaults to `true`. When set to `false` only the
    /// builders are generated and registration is left to `#[duck_custom_register]`.
    pub auto_register: Option<bool>,

    /// SpecialNullHandling
    ///
    /// 是否开启 DuckDB 的 `SpecialNullHandling`（NULL 行也进入回调）。
    ///
    /// Whether to enable DuckDB's `SpecialNullHandling` (NULL rows also reach the callback).
    pub special_null_handling: Option<bool>,

    /// `#[duck_cast_function(implicit_cost = 100)]`
    /// 隐式转换代价：设置后 DuckDB 可能自动插入该 cast，值越小优先级越高
    ///
    /// `#[duck_cast_function(implicit_cost = 100)]`. Implicit-cast cost: once set, DuckDB may
    /// insert this cast automatically, and a smaller value means higher priority.
    pub implicit_cost: Option<i64>,

    /// `#[duck_scalar_function(overloads_name = "my_overloads")]` /
    /// `#[duck_aggregate_function(overloads_name = "my_overloads")]`
    /// 指定重载函数的名称
    /// - 重载函数不注册自身的函数名，只注册重载
    /// - 同名（overloads_name 相同）的多个签名会被合并成一个函数集，
    ///   每个重载保留自己的返回类型
    ///
    /// `#[duck_scalar_function(overloads_name = "my_overloads")]` /
    /// `#[duck_aggregate_function(overloads_name = "my_overloads")]`: sets the name of the
    /// overload set. The function is then not registered under its own name but as an overload.
    /// Several signatures sharing the same `overloads_name` are merged into one function set,
    /// each overload keeping its own return type.
    pub overloads_name: Option<String>,
}
