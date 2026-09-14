use crate::{AggregateFunctionGuard, DuckResult, DuckfnAggregateFunctionSetBuilder};
use itertools::Itertools;
use quack_rs::connection::Connection;
use quack_rs::prelude::{Registrar, ScalarFunctionSetBuilder, ScalarOverloadBuilder};
use std::collections::HashMap;
use std::ffi::CString;

pub type DuckRegisterFn = fn(connection: &Connection) -> DuckResult<()>;

pub struct DuckFunctionItem {
    pub register_fn: DuckRegisterFn,
}
inventory::collect!(DuckFunctionItem);

pub fn register_all_duckfn(connection: &Connection) -> DuckResult<()> {
    for item in inventory::iter::<DuckFunctionItem>() {
        (item.register_fn)(connection)?;
    }
    register_all_aggregate_overload(connection)?;
    register_all_scalar_overload(connection)?;
    Ok(())
}

/// 聚合函数集重载项：`#[duck_aggregate_function(overloads_name = "xxx")]` 时由宏提交。
///
/// - `name`：函数集名字，同名（overloads_name 相同）的重载会被合并成一个函数集；
/// - `register_fn`：用函数集名字造出本签名的重载句柄，返回类型取各自的 Output，
///   所以同一函数集里可以有不同返回类型。
///
/// `name` 用 `&'static str` 而不是 `String`：`inventory::submit!` 会把值放进
/// `static` 初始化表达式（const 上下文），`String` 在那里无法构造。
pub struct DuckAggregateOverloadItem {
    pub name: &'static str,
    pub register_fn: fn(name: &CString) -> AggregateFunctionGuard,
}

inventory::collect!(DuckAggregateOverloadItem);

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
/// - `name`：函数集名字，同名（overloads_name 相同）的重载会被合并成一个函数集；
/// - `register_fn`：生成本签名的 `ScalarOverloadBuilder`（自带返回类型与参数表）。
///
/// `name` 用 `&'static str` 的原因同上：inventory 的提交需要 const 上下文。
pub struct DuckScalarOverloadItem {
    pub name: &'static str,
    pub register_fn: fn() -> ScalarOverloadBuilder,
}

inventory::collect!(DuckScalarOverloadItem);

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
