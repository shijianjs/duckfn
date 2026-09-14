//! `duck_sql_macro_files!` 的实现：编译期内联若干 `.sql` 文件并注册。
//!
//! Implementation of `duck_sql_macro_files!`: inlines several `.sql` files at compile time and
//! registers them.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{LitStr, Token};

/// `duck_sql_macro_files!("a.sql", "b.sql", ...)` 的实现。
///
/// 输入是一串逗号分隔的字符串字面量（文件路径），支持尾随逗号，至少一个。
/// 生成一次 inventory 注册：依次对每个文件执行
/// `duckfn::register_sql_macro_str(c, include_str!(文件路径))`。
///
/// 路径交给 `include_str!` 解析，因此：
///   - 相对「调用本宏的 .rs 文件」定位（`Span::call_site()` 指向调用处）；
///   - 编译期内联进二进制，扩展包运行时不依赖 .sql 文件；
///   - 文件缺失 / 路径写错会在编译期直接报错。
///
/// Implementation of `duck_sql_macro_files!("a.sql", "b.sql", ...)`. The input is a
/// comma-separated list of string literals (file paths) with an optional trailing comma and at
/// least one entry. It emits a single inventory registration that runs
/// `duckfn::register_sql_macro_str(c, include_str!(path))` for every file in order. Paths are
/// resolved by `include_str!`, so they are relative to the `.rs` file that invokes the macro
/// (`Span::call_site()` points at the call site), the files are inlined into the binary at
/// compile time (so the packaged extension does not need the `.sql` files at runtime), and a
/// missing file or a wrong path fails at compile time.
pub fn duck_sql_macro_files(input: TokenStream) -> TokenStream {
    let parser = Punctuated::<LitStr, Token![,]>::parse_terminated;
    let files = match parser.parse(input) {
        Ok(files) => files,
        Err(err) => return err.to_compile_error().into(),
    };

    if files.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            "duck_sql_macro_files! requires at least one SQL file path, e.g. \
             duck_sql_macro_files!(\"sql/a.sql\", \"sql/b.sql\")",
        )
        .to_compile_error()
        .into();
    }

    let registers = files.iter().map(|file| {
        quote! {
            duckfn::register_sql_macro_str(c, include_str!(#file))?;
        }
    });

    quote! {
        duckfn::inventory_submit! {
            duckfn::DuckFunctionItem {
                register_fn: |c| {
                    #(#registers)*
                    Ok(())
                }
            }
        }
    }
    .into()
}
