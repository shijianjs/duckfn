//! `#[derive(DuckEnum)]` 的实现：把「只有单元变体的 Rust enum」映射成 DuckDB `ENUM`。
//!
//! Implementation of `#[derive(DuckEnum)]`: maps a unit-variant-only Rust enum onto a DuckDB
//! `ENUM`.
//!
//! ENUM 的规则非常固定，所以这里生成的是**完整且固定**的代码，而不是像 `#[derive(DuckStruct)]`
//! 那样只生成一个 trait 实现再靠 blanket impl 兜底 —— `Option<T>` 与结构体已经各占了一个
//! `impl<T: …> DuckValueType for T`，再加第三个 blanket impl 会与它们冲突（E0119）。
//!
//! The ENUM rules are fixed, so this generates the whole `DuckValueType` implementation directly
//! instead of leaning on a blanket impl the way `#[derive(DuckStruct)]` does: `Option<T>` and the
//! struct derive already own a blanket `impl<T: …> DuckValueType for T` each, and a third one
//! would conflict with them (E0119).

use crate::attr_args::DuckEnumMacroArgs;
use crate::macro_utils::{TokenStream2Result, to_snake_case};
use darling::FromDeriveInput;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, Variant};

/// `#[duck(...)]` 属性在枚举上的解析结果。
///
/// Parse result of the `#[duck(...)]` attribute on the enum.
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(duck))]
struct DuckEnumDeriveArgs {
    /// 枚举级配置（`rename_all` / `sql_name` / `create_type`）。
    ///
    /// The enum-level configuration (`rename_all` / `sql_name` / `create_type`).
    #[darling(flatten)]
    args: DuckEnumMacroArgs,
}

/// `#[derive(DuckEnum)]` 的入口。
///
/// 校验「是 enum、没有泛型、至少一个成员、成员都是单元变体、标签不重复」，然后生成：
///
/// - 一个私有辅助模块：字典 `MEMBERS`、`index` / `from_index` / `from_label`，以及（当
///   `create_type = true` 时）加载期建类型的注册函数；
/// - `DuckValueType` 实现：逻辑类型是带字典的 ENUM，读写只搬运下标，bind 阶段按标签匹配；
/// - 当 `create_type = true` 时，把注册函数提交给 `inventory`（与其它宏一样的自动注册机制）。
///
/// Entry point of `#[derive(DuckEnum)]`. After validating the shape it generates a private helper
/// module (dictionary plus index/label conversions, and the load-time registration when
/// `create_type = true`), the `DuckValueType` implementation and — again for `create_type` — the
/// `inventory` submission that every other macro uses to auto-register.
///
/// # Errors
///
/// 不是 enum、带泛型、没有成员、成员带数据、标签重复、`#[duck(...)]` 参数非法时返回编译错误。
///
/// Returns a compile error when the input is not an enum, is generic, has no variants, has data
/// variants, has duplicate labels or carries an unsupported `#[duck(...)]` argument.
pub(crate) fn duck_enum_derive(input: DeriveInput) -> TokenStream2Result {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "`#[derive(DuckEnum)]` only supports enums",
        ));
    };
    if let Some(param) = input.generics.params.first() {
        return Err(syn::Error::new(
            param.span(),
            "`#[derive(DuckEnum)]` does not support generics: a DuckDB ENUM has no type parameters",
        ));
    }
    if data.variants.is_empty() {
        return Err(syn::Error::new(
            input.span(),
            "a DuckDB ENUM needs at least one variant",
        ));
    }

    let macro_args = DuckEnumDeriveArgs::from_derive_input(&input)?.args;
    let rename_all = macro_args.rename_all.unwrap_or_default();

    let mut variants: Vec<Ident> = Vec::with_capacity(data.variants.len());
    let mut labels: Vec<String> = Vec::with_capacity(data.variants.len());
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new(
                variant.span(),
                "`#[derive(DuckEnum)]` only supports unit variants: a DuckDB ENUM value carries no data",
            ));
        }
        let label = match variant_rename(variant)? {
            Some(rename) => rename,
            None => rename_all.apply(&variant.ident.to_string()),
        };
        if labels.contains(&label) {
            return Err(syn::Error::new(
                variant.span(),
                format!("duplicate ENUM label `{label}`: every variant needs a distinct label"),
            ));
        }
        variants.push(variant.ident.clone());
        labels.push(label);
    }

    let enum_ident = &input.ident;
    let module_ident = format_ident!("__duck_enum_{}", to_snake_case(&enum_ident.to_string()));
    let sql_name = macro_args
        .sql_name
        .clone()
        .unwrap_or_else(|| to_snake_case(&enum_ident.to_string()));
    let indexes: Vec<u32> = (0..variants.len() as u32).collect();

    // `create_type = true` 时才生成注册函数与 inventory 提交。
    //
    // The registration function and the inventory submission only exist for `create_type = true`.
    let register = macro_args
        .create_type
        .unwrap_or(false)
        .then(|| {
            quote! {
                /// 加载期把 ENUM 类型建进 catalog（`CREATE TYPE IF NOT EXISTS ...`，幂等）。
                ///
                /// Creates the ENUM type in the catalog at load time (`CREATE TYPE IF NOT
                /// EXISTS ...`, idempotent).
                pub fn register(connection: &::duckfn::Connection) -> ::duckfn::DuckResult<()> {
                    ::duckfn::register_enum_type(connection, #sql_name, MEMBERS)
                }
            }
        });

    let submit = macro_args
        .create_type
        .unwrap_or(false)
        .then(|| {
            quote! {
                ::duckfn::inventory_submit! {
                    ::duckfn::DuckFunctionItem {
                        register_fn: #module_ident::register,
                    }
                }
            }
        });

    Ok(quote! {
        #[allow(non_snake_case)]
        mod #module_ident {
            use super::#enum_ident;

            /// SQL 侧的 ENUM 字典：声明顺序就是下标。
            ///
            /// The SQL-side ENUM dictionary: the declaration order *is* the index.
            pub const MEMBERS: &[&str] = &[#(#labels),*];

            /// 变体 → 下标。
            ///
            /// Variant → index.
            pub const fn index(value: &#enum_ident) -> u32 {
                match value {
                    #(#enum_ident::#variants => #indexes,)*
                }
            }

            /// 下标 → 变体；越界返回 `None`（读取侧会把它当成 NULL）。
            ///
            /// Index → variant; an out-of-range index yields `None` (read as NULL).
            pub fn from_index(index: u32) -> Option<#enum_ident> {
                match index {
                    #(#indexes => Some(#enum_ident::#variants),)*
                    _ => None,
                }
            }

            /// 标签 → 变体（bind 阶段的 `duckdb_value` 给的是标签文本）。
            ///
            /// Label → variant (the bind-time `duckdb_value` holds the label text).
            pub fn from_label(label: &str) -> Option<#enum_ident> {
                MEMBERS
                    .iter()
                    .position(|member| *member == label)
                    .and_then(|index| from_index(index as u32))
            }

            #register
        }

        impl ::duckfn::DuckValueType for #enum_ident {
            fn type_id() -> ::duckfn::TypeId {
                ::duckfn::TypeId::Enum
            }

            /// 带字典的 ENUM 逻辑类型；`#[duck(sql_name = ...)]` 只影响 catalog 里建的命名类型，
            /// 这里始终是这份字典本身。
            ///
            /// The ENUM logical type carrying the dictionary; `#[duck(sql_name = ...)]` only names
            /// the catalog type, this is always the dictionary itself.
            fn logical_type() -> ::duckfn::LogicalType {
                ::duckfn::LogicalType::enum_type(#module_ident::MEMBERS)
            }

            fn read_valid(reader: &::duckfn::DuckValueReader, row: usize) -> Option<Self> {
                #module_ident::from_index(::duckfn::read_enum_index(
                    reader.c_duckdb_vector,
                    row,
                    #module_ident::MEMBERS.len(),
                ))
            }

            fn write_valid(writer: &mut ::duckfn::DuckValueWriter, idx: usize, value: &Self) {
                ::duckfn::write_enum_index(
                    writer.c_duckdb_vector,
                    idx,
                    #module_ident::MEMBERS.len(),
                    #module_ident::index(value),
                );
            }

            /// bind 阶段（表函数参数、结构体字段）拿到的是标签文本。
            ///
            /// The bind path (table-function arguments, struct fields) carries the label text.
            fn read_by_duck_value_valid(value: &::duckfn::Value) -> ::duckfn::DuckResult<Self> {
                let label = value.as_str()?;
                #module_ident::from_label(&label).ok_or_else(|| {
                    ::duckfn::duck_error(format!(
                        "ENUM value {:?} is not a member of the `{}` dictionary {:?}",
                        label,
                        #sql_name,
                        #module_ident::MEMBERS,
                    ))
                })
            }

            fn read_by_duck_value_valid_simple(value: &::duckfn::Value) -> Self {
                Self::read_by_duck_value_valid(value).unwrap_or_else(|err| panic!("{}", err.as_str()))
            }
        }

        #submit
    })
}

/// 读取变体上的 `#[duck(rename = "...")]`。
///
/// Reads `#[duck(rename = "...")]` from a variant, rejecting every other key.
fn variant_rename(variant: &Variant) -> syn::Result<Option<String>> {
    let mut rename = None;
    for attr in &variant.attrs {
        if !attr.path().is_ident("duck") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                rename = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                return Ok(());
            }
            Err(meta.error(
                "unsupported `#[duck(...)]` key on an enum variant: only `rename = \"...\"` is supported",
            ))
        })?;
    }
    Ok(rename)
}
