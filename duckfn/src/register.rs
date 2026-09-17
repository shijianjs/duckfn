//! 函数注册收集器：汇聚过程宏通过 `inventory` 提交的注册项，在扩展初始化时统一注册。
//!
//! Registration collector: gathers the entries submitted by the procedural macros through
//! `inventory` and registers them all when the extension is initialised.

use crate::{AggregateFunctionGuard, DuckResult, DuckfnAggregateFunctionSetBuilder};
use itertools::Itertools;
use quack_rs::connection::Connection;
use quack_rs::prelude::{Registrar, ScalarFunctionSetBuilder, ScalarOverloadBuilder};
use std::collections::HashMap;
use std::ffi::CString;

/// 注册回调的函数指针类型：接收一个 DuckDB 连接，返回可能失败的结果。
///
/// Signature of a registration callback: takes a DuckDB connection and returns a fallible
/// result.
pub type DuckRegisterFn = fn(connection: &Connection) -> DuckResult<()>;

/// 一条待注册项，由 `#[duck_*]` 宏（以及 `duck_sql_macro_files!`）通过
/// `inventory::submit!` 在编译期提交。
///
/// A single registration entry, submitted at compile time by the `#[duck_*]` macros (and by
/// `duck_sql_macro_files!`) through `inventory::submit!`.
pub struct DuckFunctionItem {
    /// 实际执行注册的函数/闭包。
    ///
    /// The function/closure that performs the actual registration.
    pub register_fn: DuckRegisterFn,
}
// 把 `DuckFunctionItem` 登记进 inventory，使 `register_all_duckfn` 能遍历所有提交项。
//
// Collect `DuckFunctionItem`s so that `register_all_duckfn` can iterate over every submission.
inventory::collect!(DuckFunctionItem);

/// 注册本扩展收集到的全部 DuckDB 函数（扩展初始化入口）。
///
/// 顺序为：先按提交顺序执行所有 `DuckFunctionItem`（标量函数、表函数、cast、SQL 宏、
/// replacement scan、自定义注册等），再分组注册聚合函数集重载，最后分组注册标量函数集
/// 重载。全部跑完后统一把 `create_type = "print"` 收集到的 DDL 打印一次（见
/// [`crate::flush_queued_type_ddl`]）—— 入口点在这里，所以一批 `"print"` 类型只有一块提示。
///
/// Registers every DuckDB function collected by this extension (the extension init entry
/// point). It first runs all `DuckFunctionItem`s in submission order (scalar functions,
/// table functions, casts, SQL macros, replacement scans, custom registrations, ...), then
/// registers the aggregate overload sets and finally the scalar overload sets. Once everything has
/// run it flushes the DDL collected by `create_type = "print"` in one go (see
/// [`crate::flush_queued_type_ddl`]) — this is the entry point, so a batch of print-mode types
/// yields a single notice.
///
/// # Errors
///
/// 任一注册步骤失败时立即返回该错误，后续函数不再注册（收集到的 DDL 仍会打印出来）。
///
/// Returns the first error encountered; remaining functions are then left unregistered (the
/// collected DDL is printed either way).
pub fn register_all_duckfn(connection: &Connection) -> DuckResult<()> {
    let result = register_collected_items(connection);
    // 注册失败也把 `create_type = "print"` 的 DDL 打出来：它只是「预览」，与注册成败无关，
    // 出问题时反而更需要看到。
    //
    // Flush the `create_type = "print"` DDL even when registration failed: it is only a preview,
    // independent of the outcome — and all the more useful when something went wrong.
    crate::flush_queued_type_ddl();
    result
}

/// [`register_all_duckfn`] 的实际注册流程；收尾（打印 `create_type = "print"` 的 DDL）由调用方负责。
///
/// The actual registration pass; the caller takes care of the flush afterwards.
fn register_collected_items(connection: &Connection) -> DuckResult<()> {
    for item in inventory::iter::<DuckFunctionItem>() {
        (item.register_fn)(connection)?;
    }
    register_all_aggregate_overload(connection)?;
    register_all_scalar_overload(connection)?;
    Ok(())
}

/// 聚合函数集重载项：`#[duck_aggregate_function(overloads_name = "xxx")]` 时由宏提交。
///
/// Aggregate function-set overload entry, submitted by the macro when
/// `#[duck_aggregate_function(overloads_name = "xxx")]` is used.
///
/// - `name`：函数集名字，同名（overloads_name 相同）的重载会被合并成一个函数集；
/// - `register_fn`：用函数集名字造出本签名的重载句柄，返回类型取各自的 Output，
///   所以同一函数集里可以有不同返回类型。
///
/// - `name`: the function-set name; overloads sharing the same `overloads_name` are merged
///   into one set.
/// - `register_fn`: builds the overload handle for this signature with the set name; each
///   overload keeps its own output type, so one set may contain different return types.
///
/// `name` 用 `&'static str` 而不是 `String`：`inventory::submit!` 会把值放进
/// `static` 初始化表达式（const 上下文），`String` 在那里无法构造。
///
/// `name` is a `&'static str` rather than `String` because `inventory::submit!` stores the
/// value in a `static` initialiser (const context), where `String` cannot be constructed.
pub struct DuckAggregateOverloadItem {
    /// 函数集名字。
    ///
    /// Name of the aggregate function set.
    pub name: &'static str,
    /// 用函数集名字生成该签名对应的重载句柄。
    ///
    /// Builds the overload handle of this signature using the function-set name.
    pub register_fn: fn(name: &CString) -> AggregateFunctionGuard,
}

// 把 `DuckAggregateOverloadItem` 登记进 inventory，供分组注册时遍历。
//
// Collect `DuckAggregateOverloadItem`s so that grouped registration can iterate over them.
inventory::collect!(DuckAggregateOverloadItem);

/// 按 `name` 分组注册所有聚合函数集重载。
///
/// 同名（`overloads_name` 相同）的重载会被合并进同一个 `DuckfnAggregateFunctionSetBuilder`，
/// 然后一次性注册到 DuckDB；每个重载保留自己的返回类型。
///
/// Registers every aggregate overload set, grouped by `name`. Overloads sharing the same
/// `overloads_name` are merged into one `DuckfnAggregateFunctionSetBuilder` and registered
/// to DuckDB in one shot, each overload keeping its own return type.
///
/// # Errors
///
/// 注册任一函数集失败时返回对应错误。
///
/// Returns the error of the first function set that fails to register.
pub fn register_all_aggregate_overload(connection: &Connection) -> DuckResult<()> {
    let map: HashMap<&'static str, Vec<&DuckAggregateOverloadItem>> =
        inventory::iter::<DuckAggregateOverloadItem>()
            .into_iter()
            .into_grouping_map_by(|item| item.name)
            .collect();
    for (name, items) in map {
        let c_name = CString::new(name).expect("function name must not contain null bytes");
        let map1: Vec<AggregateFunctionGuard> = items
            .iter()
            .map(|item| (item.register_fn)(&c_name))
            .collect();
        let builder = DuckfnAggregateFunctionSetBuilder::new(name, map1);
        unsafe { builder.register(connection.as_raw_connection()) }?;
    }
    Ok(())
}

/// 标量函数集重载项：`#[duck_scalar_function(overloads_name = "xxx")]` 时由宏提交。
///
/// Scalar function-set overload entry, submitted by the macro when
/// `#[duck_scalar_function(overloads_name = "xxx")]` is used.
///
/// - `name`：函数集名字，同名（overloads_name 相同）的重载会被合并成一个函数集；
/// - `register_fn`：生成本签名的 `ScalarOverloadBuilder`（自带返回类型与参数表）。
///
/// - `name`: the function-set name; overloads sharing the same `overloads_name` are merged
///   into one set.
/// - `register_fn`: produces the `ScalarOverloadBuilder` of this signature (carrying its own
///   return type and parameter list).
///
/// `name` 用 `&'static str` 的原因同上：inventory 的提交需要 const 上下文。
///
/// `name` is a `&'static str` for the same reason as above: inventory submission needs a
/// const context.
pub struct DuckScalarOverloadItem {
    /// 函数集名字。
    ///
    /// Name of the scalar function set.
    pub name: &'static str,
    /// 生成本签名对应的标量函数重载 builder。
    ///
    /// Produces the scalar overload builder of this signature.
    pub register_fn: fn() -> ScalarOverloadBuilder,
}

// 把 `DuckScalarOverloadItem` 登记进 inventory，供分组注册时遍历。
//
// Collect `DuckScalarOverloadItem`s so that grouped registration can iterate over them.
inventory::collect!(DuckScalarOverloadItem);

/// 按 `name` 分组注册所有标量函数集重载。
///
/// 同名（`overloads_name` 相同）的重载会被合并进同一个 `ScalarFunctionSetBuilder`，
/// 再一次性注册到 DuckDB。
///
/// Registers every scalar overload set, grouped by `name`. Overloads sharing the same
/// `overloads_name` are merged into one `ScalarFunctionSetBuilder` and registered together.
///
/// # Errors
///
/// 注册任一函数集失败时返回对应错误。
///
/// Returns the error of the first function set that fails to register.
pub fn register_all_scalar_overload(connection: &Connection) -> DuckResult<()> {
    let map: HashMap<&'static str, Vec<&DuckScalarOverloadItem>> =
        inventory::iter::<DuckScalarOverloadItem>()
            .into_iter()
            .into_grouping_map_by(|item| item.name)
            .collect();
    for (name, items) in map {
        let mut builder = ScalarFunctionSetBuilder::new(name);
        for x in items {
            builder = builder.overload((x.register_fn)());
        }
        unsafe { connection.register_scalar_set(builder) }?;
    }
    Ok(())
}
