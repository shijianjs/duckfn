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

use crate::common::CreateTypeMode;
use crate::macro_utils::{TokenStream2Result, to_snake_case};
use darling::{FromDeriveInput, FromMeta};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, Variant};

/// `#[derive(DuckEnum)]` 在枚举上 `#[duck(...)]` 可用的全部参数。
///
/// 参数只声明本 derive 自己需要、自己认识的键；属性宏那边各有各的参数结构体，不再共用，也不再把
/// 自己的参数原样透传过来。变体级只支持 `#[duck(rename = "...")]`（在 derive 里就地解析）。
///
/// Every argument `#[duck(...)]` accepts on `#[derive(DuckEnum)]`. It declares only the keys this
/// derive needs and knows; the attribute macros each own their own argument struct, no longer share
/// one and no longer forward their arguments verbatim. At variant level only
/// `#[duck(rename = "...")]` is supported (parsed in the derive itself).
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(duck))]
struct DuckEnumDeriveArgs {
    /// `#[duck(rename_all = "snake_case")]`：变体名 → SQL 字典标签的命名规则，默认原样使用。
    ///
    /// `#[duck(rename_all = "snake_case")]`: how variant names map onto SQL dictionary labels;
    /// by default the variant name is used verbatim.
    rename_all: Option<RenameRule>,

    /// `#[duck(sql_name = "priority")]`：SQL 侧的类型名，默认用类型名的小写蛇形。
    ///
    /// `#[duck(sql_name = "priority")]`: the SQL-side type name; defaults to the type name in
    /// lowercase snake_case.
    sql_name: Option<String>,

    /// `#[duck(create_type = ...)]`：加载期如何处理这个命名类型，默认 `false`。
    ///
    /// `#[duck(create_type = ...)]`: how the named type is handled at load time; defaults to
    /// `false`.
    create_type: Option<CreateTypeMode>,
}

/// 变体名 → SQL 字典标签的命名规则（`#[duck(rename_all = "...")]`）。
///
/// `#[derive(DuckEnum)]` 用它把 Rust 变体名（`HttpError` 这种 PascalCase）映射成 SQL 侧的
/// ENUM 标签；单个变体可以用 `#[duck(rename = "...")]` 覆盖。
///
/// The naming rule mapping a variant name onto its SQL dictionary label. `#[derive(DuckEnum)]`
/// uses it to turn a PascalCase variant name into the SQL-side ENUM label, and a single variant
/// can override it with `#[duck(rename = "...")]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum RenameRule {
    /// 原样使用变体名（默认）。/ The variant name as written (the default).
    #[default]
    Verbatim,
    /// `Red` -> `red`；选 `"lowercase"`。
    ///
    /// `Red` -> `red`; written as `"lowercase"`.
    Lower,
    /// `Red` -> `RED`；选 `"UPPERCASE"`。
    ///
    /// `Red` -> `RED`; written as `"UPPERCASE"`.
    Upper,
    /// `HttpError` -> `http_error`；选 `"snake_case"`。
    ///
    /// `HttpError` -> `http_error`; written as `"snake_case"`.
    Snake,
    /// `HttpError` -> `HTTP_ERROR`；选 `"SCREAMING_SNAKE_CASE"`。
    ///
    /// `HttpError` -> `HTTP_ERROR`; written as `"SCREAMING_SNAKE_CASE"`.
    ScreamingSnake,
    /// `HttpError` -> `httpError`（只把首字母小写）；选 `"camelCase"`。
    ///
    /// `HttpError` -> `httpError` (only the first letter is lowered); written as `"camelCase"`.
    Camel,
    /// 保持 PascalCase；选 `"PascalCase"`。
    ///
    /// Keeps PascalCase; written as `"PascalCase"`.
    Pascal,
    /// `HttpError` -> `http-error`；选 `"kebab-case"`。
    ///
    /// `HttpError` -> `http-error`; written as `"kebab-case"`.
    Kebab,
    /// `HttpError` -> `HTTP-ERROR`；选 `"SCREAMING-KEBAB-CASE"`。
    ///
    /// `HttpError` -> `HTTP-ERROR`; written as `"SCREAMING-KEBAB-CASE"`.
    ScreamingKebab,
}

impl RenameRule {
    /// 把一个变体名按规则转成标签。
    ///
    /// Applies the rule to a variant name.
    #[must_use]
    fn apply(self, variant: &str) -> String {
        match self {
            RenameRule::Verbatim | RenameRule::Pascal => variant.to_owned(),
            RenameRule::Lower => variant.to_lowercase(),
            RenameRule::Upper => variant.to_uppercase(),
            RenameRule::Snake => to_snake_case(variant),
            RenameRule::ScreamingSnake => to_snake_case(variant).to_uppercase(),
            RenameRule::Camel => {
                let mut chars = variant.chars();
                match chars.next() {
                    Some(first) => first.to_lowercase().chain(chars).collect(),
                    None => String::new(),
                }
            }
            RenameRule::Kebab => to_snake_case(variant).replace('_', "-"),
            RenameRule::ScreamingKebab => to_snake_case(variant).to_uppercase().replace('_', "-"),
        }
    }
}

impl FromMeta for RenameRule {
    fn from_string(value: &str) -> darling::Result<Self> {
        Ok(match value {
            "verbatim" => RenameRule::Verbatim,
            "lowercase" => RenameRule::Lower,
            "UPPERCASE" => RenameRule::Upper,
            "snake_case" => RenameRule::Snake,
            "SCREAMING_SNAKE_CASE" => RenameRule::ScreamingSnake,
            "camelCase" => RenameRule::Camel,
            "PascalCase" => RenameRule::Pascal,
            "kebab-case" => RenameRule::Kebab,
            "SCREAMING-KEBAB-CASE" => RenameRule::ScreamingKebab,
            other => return Err(darling::Error::unknown_value(other)),
        })
    }
}

/// `#[derive(DuckEnum)]` 的入口。
///
/// 校验「是 enum、没有泛型、至少一个成员、成员都是单元变体、标签不重复」，然后生成：
///
/// - 一个私有辅助模块：字典 `MEMBERS`、`index` / `from_index` / `from_label`，以及（当
///   `create_type` 不为 `false` 时）加载期处理命名类型的函数 —— `true` 建类型，`"print"` 把 DDL 收进队列；
/// - `DuckValueType` 实现：逻辑类型是带字典的 ENUM，读写只搬运下标，bind 阶段按标签匹配；
/// - 当 `create_type` 不为 `false` 时，把该函数提交给 `inventory`（与其它宏一样的自动注册机制）。
///
/// Entry point of `#[derive(DuckEnum)]`. After validating the shape it generates a private helper
/// module (dictionary plus index/label conversions, and the load-time handling of the named type
/// when `create_type` is not `false` — `true` creates it, `"print"` queues the DDL), the
/// `DuckValueType` implementation and — again when `create_type` is not `false` — the `inventory`
/// submission that every other macro uses to auto-register.
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

    let macro_args = DuckEnumDeriveArgs::from_derive_input(&input)?;
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

    // `create_type` 不为 `false` 时才生成注册函数与 inventory 提交：`true` 建类型，`"print"` 只打印 DDL。
    //
    // The registration function and the inventory submission only exist for a `create_type` other
    // than `false`: `true` creates the type, `"print"` only prints the DDL.
    let create_type = macro_args.create_type.unwrap_or_default();

    let register = match create_type {
        CreateTypeMode::Create => Some(quote! {
            /// 加载期把 ENUM 类型建进 catalog（`CREATE TYPE IF NOT EXISTS ...`，幂等）。
            ///
            /// Creates the ENUM type in the catalog at load time (`CREATE TYPE IF NOT
            /// EXISTS ...`, idempotent).
            pub fn register(connection: &::duckfn::Connection) -> ::duckfn::DuckResult<()> {
                ::duckfn::register_enum_type(connection, #sql_name, MEMBERS)
            }
        }),
        CreateTypeMode::Print => Some(quote! {
            /// 加载期把建类型的 DDL 收进队列（不建类型）；全部注册跑完后由入口点一次性打印。
            ///
            /// Queues the `CREATE TYPE IF NOT EXISTS ...` statement at load time (the type is not
            /// created); the entry point prints the collected batch afterwards.
            pub fn register(_connection: &::duckfn::Connection) -> ::duckfn::DuckResult<()> {
                ::duckfn::queue_enum_type_ddl(#sql_name, MEMBERS)
            }
        }),
        CreateTypeMode::Off => None,
    };

    let submit = register.as_ref().map(|_| {
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
