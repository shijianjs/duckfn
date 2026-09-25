//! 函数文档元数据的收集。
//!
//! [`DuckFunctionDocItem`] 由 `#[duck_*]` 属性宏在编译期提交（每个会注册出函数的宏都提交一条，
//! 没写文档参数的三个字段为空），[`declared_function_descriptions`] 把它们按函数名收集合并。
//!
//! 这里**不碰 DuckDB**：不加载扩展、不查 catalog、不做差集。导出成 CSV 是命令行工具的事，
//! 见 `duckfn::cli` 与 `cargo run --bin duckfn -- function_descriptions`。
//!
//! 为什么要绕这一圈：DuckDB 的 C 扩展 API 只暴露了 name / varargs / return_type / volatile /
//! special_handling 这些设置，**没有**设置 description 与 example 的接口，所以这类文本没法随
//! 扩展注册进 catalog。社区扩展的文档页由 `duckdb/community-extensions` 的
//! `scripts/generate_md.sh` 生成，它读扩展目录下的 `docs/function_descriptions.csv`，再按
//! `function_name == other.function` LEFT JOIN 覆盖 `functions` / `functions_overloads` 的展示
//! 列 —— 这份 CSV 是唯一的入口。
//!
//! Collection of the function documentation metadata. [`DuckFunctionDocItem`] is submitted at
//! compile time by the `#[duck_*]` attribute macros (every macro that registers a function submits
//! one; the three fields stay empty when no documentation argument was written) and
//! [`declared_function_descriptions`] merges them by function name. Nothing here touches DuckDB: no
//! extension is loaded, the catalog is never queried and no diff is taken. Turning this into a CSV
//! is the command-line tool's job — see `duckfn::cli` and
//! `cargo run --bin duckfn -- function_descriptions`.

use std::collections::BTreeMap;

/// 一条函数的文档元数据，由 `#[duck_*]` 宏通过 `inventory::submit!` 提交。
///
/// 字段用 `&'static str` / `&'static [&'static str]` 而不是 `String` / `Vec<String>`：
/// `inventory::submit!` 把值放进 `static` 初始化表达式（const 上下文），堆类型在那里
/// 构造不出来（与 [`crate::DuckScalarOverloadItem`] 的 `name` 同理）。
///
/// One function's documentation metadata, submitted by the `#[duck_*]` macros through
/// `inventory::submit!`. The fields are `&'static str` / `&'static [&'static str]` rather than
/// `String` / `Vec<String>` because `inventory::submit!` places the value in a `static`
/// initialiser (a const context), where heap types cannot be built (same reason as
/// [`crate::DuckScalarOverloadItem`]'s `name`).
pub struct DuckFunctionDocItem {
    /// 该函数在 DuckDB 里真正注册的名字（`overloads_name` 时是函数集名）。
    ///
    /// The name the function is really registered under (the function-set name when
    /// `overloads_name` is set).
    pub function: &'static str,
    /// 一句话说明。
    ///
    /// One-line summary.
    pub description: Option<&'static str>,
    /// 补充说明。
    ///
    /// Extra remarks.
    pub comment: Option<&'static str>,
    /// 调用示例。
    ///
    /// Usage examples.
    pub examples: &'static [&'static str],
}
// 把 `DuckFunctionDocItem` 登记进 inventory，供 [`declared_function_descriptions`] 遍历。
//
// Collect `DuckFunctionDocItem`s so that [`declared_function_descriptions`] can iterate over them.
inventory::collect!(DuckFunctionDocItem);

/// 合并后的函数文档：同一个函数名的多条提交（重载或函数集的各个签名）在这里合成一条。
///
/// A merged function description: several submissions under one function name (overloads, or the
/// signatures of one function set) collapse into a single entry here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDescription {
    /// 函数名（CSV 的 `function` 列）。
    ///
    /// The function name (the CSV's `function` column).
    pub function: String,
    /// 一句话说明（CSV 的 `description` 列）。
    ///
    /// One-line summary (the CSV's `description` column).
    pub description: Option<String>,
    /// 补充说明（CSV 的 `comment` 列）。
    ///
    /// Extra remarks (the CSV's `comment` column).
    pub comment: Option<String>,
    /// 调用示例；导出时用 `, ` 拼进 CSV 的 `example` 列。
    ///
    /// Usage examples; joined with `", "` into the CSV's `example` column on export.
    pub examples: Vec<String>,
}

impl FunctionDescription {
    /// 是否写了文档：`description` / `comment` / `examples` 任一非空。
    ///
    /// 导出默认只写这些行；`--all` 才把没写的（三个字段全空）也写出来。
    ///
    /// Whether any documentation was written: at least one of `description` / `comment` /
    /// `examples` is non-empty. The default export writes only these rows; `--all` also writes the
    /// ones with all three empty.
    pub fn is_documented(&self) -> bool {
        self.description.is_some() || self.comment.is_some() || !self.examples.is_empty()
    }
}

/// 收集源码里声明的全部函数文档，按函数名排序并合并。
///
/// 合并规则：`description` / `comment` 取第一个非空值，`examples` 拼接后按内容去重。
/// 没写文档参数的函数也在结果里（三个字段为空），调用方自行决定要不要过滤
/// （[`FunctionDescription::is_documented`]）。
///
/// Collects every function documentation entry declared in the source, sorted by function name and
/// merged. Merging takes the first non-empty `description` / `comment` and concatenates `examples`
/// while dropping duplicates. Functions without any documentation argument are included too (with
/// all three fields empty); filtering them out is up to the caller
/// ([`FunctionDescription::is_documented`]).
pub fn declared_function_descriptions() -> Vec<FunctionDescription> {
    let mut merged: BTreeMap<&'static str, FunctionDescription> = BTreeMap::new();
    for item in inventory::iter::<DuckFunctionDocItem>() {
        let entry = merged
            .entry(item.function)
            .or_insert_with(|| FunctionDescription {
                function: item.function.to_string(),
                description: item.description.map(str::to_string),
                comment: item.comment.map(str::to_string),
                examples: Vec::new(),
            });
        if entry.description.is_none() {
            entry.description = item.description.map(str::to_string);
        }
        if entry.comment.is_none() {
            entry.comment = item.comment.map(str::to_string);
        }
        for example in item.examples {
            if !entry.examples.iter().any(|existing| existing == example) {
                entry.examples.push((*example).to_string());
            }
        }
    }
    merged.into_values().collect()
}
