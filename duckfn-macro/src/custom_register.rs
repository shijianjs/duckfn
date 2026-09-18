//! `#[duck_custom_register]` 的代码生成实现。
//!
//! Code generation behind `#[duck_custom_register]`.

use crate::common::{ItemFnWrapper, handle_duck_function};
use crate::macro_utils::TokenStream2Result;
use proc_macro::TokenStream;
use quote::quote;

/// `#[duck_custom_register]` 的入口：本宏不接受任何参数。
///
/// The `#[duck_custom_register]` entry point: this macro accepts no arguments.
pub(crate) fn build(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(
        attr,
        item,
        |wrapper: ItemFnWrapper<syn::parse::Nothing>| wrapper.build_custom_register(),
    )
}

impl<A> ItemFnWrapper<A> {
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
}
