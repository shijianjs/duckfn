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

    /// `#[duck_table_function(dynamic_columns = true)]`：输出列在 bind 阶段动态确定。
    ///
    /// 开启后函数不再返回行迭代器，而是返回「schema + 行迭代器」的
    /// `duckfn::DuckDynamicTable`（或 `DuckResult<DuckDynamicTable>`）：列名与列类型可以来自
    /// 文件头、字典表、远端 schema 等外部元数据。默认 `false`，保持原有的静态列行为。
    ///
    /// `#[duck_table_function(dynamic_columns = true)]`: the output columns are decided dynamically
    /// during bind. With this on, the function no longer returns a row iterator but a
    /// `duckfn::DuckDynamicTable` (or `DuckResult<DuckDynamicTable>`) carrying "schema + row
    /// iterator": the column names and types may come from external metadata such as a file header,
    /// a dictionary table or a remote schema. Defaults to `false`, keeping the static-column
    /// behaviour.
    pub dynamic_columns: Option<bool>,

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

    /// `#[duck_scalar_function(volatile = true)]`
    ///
    /// 是否把标量函数标记为 volatile，默认 `false`。
    ///
    /// 开启后注册期会调用 `duckdb_scalar_function_set_volatile`：DuckDB 不缓存、不复用相同参数的
    /// 调用结果，每一行都重新求值（`random()` 这类函数需要它）；不开启时 DuckDB 可能把常量参数
    /// 的调用折叠成只执行一次。需要 duckfn 打开 `duckdb-1-5` feature（DuckDB 1.5.0+ 的 C API），
    /// 未开启时该开关被忽略。仅对标量函数有效，且不能和 `overloads_name` 同用
    /// （quack-rs 的 `ScalarOverloadBuilder` 没有暴露该开关）。
    ///
    /// `#[duck_scalar_function(volatile = true)]`: whether to mark the scalar function volatile;
    /// defaults to `false`. Once enabled, registration calls
    /// `duckdb_scalar_function_set_volatile`, so DuckDB neither caches nor reuses the result of a
    /// call with the same arguments — each row is re-evaluated, which is what functions like
    /// `random()` need; without it DuckDB may fold constant-argument calls into a single
    /// execution. Requires duckfn's `duckdb-1-5` feature (the DuckDB 1.5.0+ C API); the switch is
    /// ignored otherwise. It only applies to scalar functions and cannot be combined with
    /// `overloads_name` (quack-rs' `ScalarOverloadBuilder` does not expose it).
    pub volatile: Option<bool>,

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

    /// `#[duck(create_type = ...)]`（`#[derive(DuckEnum)]` / `#[derive(DuckStruct)]`）：
    /// 加载期如何处理这个命名类型，默认 `false`（什么都不做）。取值见 [`CreateTypeMode`]：
    ///
    /// - `true`：执行 `CREATE TYPE IF NOT EXISTS <sql_name> AS <类型>;`；
    /// - `"print"`：把同一条 DDL 收进队列，**不**建类型；等全部注册项跑完，扩展入口点把这一批
    ///   （`"print"` 的全部类型）一次性打印到 stderr。
    ///
    /// 语句是幂等的（`LOAD` 多次也不会报错），且不会覆盖已存在的同名类型；执行路径与 SQL 宏相同
    /// （`duckdb_query`）。STRUCT 的字段类型由 DuckDB 自己的逻辑类型渲染成 SQL，因此自定义字段
    /// 类型（含手写的 `DuckValueType`）同样适用。
    ///
    /// `#[duck(create_type = ...)]` (on `#[derive(DuckEnum)]` / `#[derive(DuckStruct)]`): how the
    /// named type is handled at load time; defaults to `false` (nothing happens). See
    /// [`CreateTypeMode`]: `true` runs `CREATE TYPE IF NOT EXISTS <sql_name> AS <type>;` while
    /// `"print"` queues that same DDL without creating the type — the entry point prints the whole
    /// batch (every print-mode type) in one block once every registration has run. The statement is
    /// idempotent (loading the extension twice is fine) and leaves a pre-existing type untouched; it
    /// goes through the SQL-macro execution path (`duckdb_query`). A STRUCT's field types are
    /// rendered from DuckDB's own logical types, so custom field types — hand-written
    /// `DuckValueType` implementations included — work too.
    pub create_type: Option<CreateTypeMode>,
}

/// `#[duck(create_type = ...)]` 的取值。
///
/// 「打印」模式（`create_type = "print"`）只把宏会执行的 DDL 收进队列、**不**进 catalog；等全部
/// 注册项跑完，入口点把这一批一次性打到 stderr —— 可以先看看渲染出来的 SQL 长什么样、再决定要不要
/// 真的建类型，也可以把语句抄走自己执行。
///
/// The values `#[duck(create_type = ...)]` accepts. The print mode
/// (`create_type = "print"`) queues the very DDL the macro would run without touching the catalog,
/// and the entry point prints the collected batch in one block once every registration has run — so
/// the rendered statements can be inspected and copied before committing to them.
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
