//! 属性宏的参数解析与公共调度逻辑。
//!
//! Argument parsing and common dispatch logic for the attribute macros.

use crate::duck_function::ItemFnWrapper;
use crate::macro_utils::{TokenStream2Result, handle_token_stream2_result, to_snake_case};
use darling::{FromDeriveInput, FromMeta};
use proc_macro::TokenStream;
use syn::{ItemFn, parse_macro_input};

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

    /// `#[duck(sql_name = "priority")]`（`#[derive(DuckEnum)]` / `#[derive(DuckStruct)]`）：
    /// SQL 侧的类型名，默认用类型名的小写蛇形（`Priority` -> `priority`）。
    ///
    /// 配合 `create_type` 建类型，也用在报错信息里。
    ///
    /// `#[duck(sql_name = "priority")]` (on `#[derive(DuckEnum)]` / `#[derive(DuckStruct)]`): the
    /// SQL-side type name; defaults to the type name in lowercase snake_case. It is used by
    /// `create_type` and in error messages.
    pub sql_name: Option<String>,

    /// `#[duck(create_type = true)]`（`#[derive(DuckEnum)]` / `#[derive(DuckStruct)]`）：
    /// 加载期执行 `CREATE TYPE IF NOT EXISTS <sql_name> AS <类型>;`，默认 `false`。
    ///
    /// 语句是幂等的（`LOAD` 多次也不会报错），且不会覆盖已存在的同名类型；执行路径与 SQL 宏相同
    /// （`duckdb_query`）。STRUCT 的字段类型由 DuckDB 自己的逻辑类型渲染成 SQL，因此自定义字段
    /// 类型（含手写的 `DuckValueType`）同样适用。
    ///
    /// `#[duck(create_type = true)]` (on `#[derive(DuckEnum)]` / `#[derive(DuckStruct)]`): run
    /// `CREATE TYPE IF NOT EXISTS <sql_name> AS <type>;` at load time; defaults to `false`. The
    /// statement is idempotent (loading the extension twice is fine) and leaves a pre-existing type
    /// untouched; it goes through the SQL-macro execution path (`duckdb_query`). A STRUCT's field
    /// types are rendered from DuckDB's own logical types, so custom field types — hand-written
    /// `DuckValueType` implementations included — work too.
    pub create_type: Option<bool>,
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
pub(crate) enum RenameRule {
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
    pub(crate) fn apply(self, variant: &str) -> String {
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

/// `#[derive(DuckEnum)]` 在枚举上 `#[duck(...)]` 可用的参数。
///
/// 除 `rename_all` 外，其余键（`sql_name` / `create_type`）就是
/// [`DuckFunctionMacroArgs`] 里那两个，通过 `#[darling(flatten)]` 复用。
/// 变体级只支持 `#[duck(rename = "...")]`（在 derive 里就地解析）。
///
/// The `#[duck(...)]` arguments `#[derive(DuckEnum)]` accepts on the enum. Apart from
/// `rename_all` they are the `sql_name` / `create_type` keys of [`DuckFunctionMacroArgs`], reused
/// through `#[darling(flatten)]`. At variant level only `#[duck(rename = "...")]` is supported
/// (parsed in the derive itself).
#[derive(Debug, FromDeriveInput)]
#[darling(attributes(duck))]
pub(crate) struct DuckEnumMacroArgs {
    /// `#[duck(rename_all = "snake_case")]`：变体名 → SQL 字典标签的命名规则，默认原样使用。
    ///
    /// `#[duck(rename_all = "snake_case")]`: how variant names map onto SQL dictionary labels;
    /// by default the variant name is used verbatim.
    pub rename_all: Option<RenameRule>,

    /// `sql_name` / `create_type` 等共用配置。
    ///
    /// The shared configuration (`sql_name`, `create_type`, ...).
    #[darling(flatten)]
    pub args: DuckFunctionMacroArgs,
}
