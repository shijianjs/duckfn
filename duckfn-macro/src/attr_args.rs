use proc_macro::TokenStream;
use darling::FromMeta;
use syn::{parse_macro_input, ItemFn};
use crate::duck_function::ItemFnWrapper;
use crate::macro_utils::{handle_token_stream2_result, TokenStream2Result};

pub fn handle_duck_function(
    _attr: TokenStream,
    item: TokenStream,
    run: fn(ItemFnWrapper) -> TokenStream2Result,
) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let duck_args: DuckArgs = match syn::parse(_attr.clone()) {
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

#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
pub(crate) struct DuckArgs {
    /// 表函数的命名参数从哪个开始
    #[allow(dead_code)]
    pub named_param_from: Option<String>,

    /// Whether to auto register the function
    /// - Default to true
    pub auto_register: Option<bool>,
    /// SpecialNullHandling
    pub special_null_handling: Option<bool>,
    /// `#[duck_cast_function(implicit_cost = 100)]`
    /// 隐式转换代价：设置后 DuckDB 可能自动插入该 cast，值越小优先级越高
    pub implicit_cost: Option<i64>,

    /// `#[duck_cast_function(overloads_name = "my_overloads")]`
    /// 指定重载函数的名称
    /// - 重载函数不注册自身的函数名，只注册重载
    pub overloads_name: Option<String>,
}
