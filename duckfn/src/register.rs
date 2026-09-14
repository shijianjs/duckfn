use crate::{AggregateFunctionGuard, DuckResult, DuckfnAggregateFunctionSetBuilder};
use itertools::Itertools;
use quack_rs::connection::Connection;
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
    Ok(())
}

pub struct DuckAggregateOverloadItem {
    pub name: String,
    pub register_fn: fn(name: &CString) -> AggregateFunctionGuard,
}

inventory::collect!(DuckAggregateOverloadItem);

pub fn register_all_aggregate_overload(connection: &Connection) -> DuckResult<()> {
    let map: HashMap<String, Vec<&DuckAggregateOverloadItem>> =
        inventory::iter::<DuckAggregateOverloadItem>()
            .into_iter()
            .into_grouping_map_by(|item| item.name.clone())
            .collect();
    for (name, items) in map {
        let c_name = CString::new(name.clone()).expect("function name must not contain null bytes");
        let map1: Vec<AggregateFunctionGuard> = items
            .iter()
            .map(|item| (item.register_fn)(&c_name))
            .collect();
        let builder = DuckfnAggregateFunctionSetBuilder::new(&name, map1);
        unsafe { builder.register(connection.as_raw_connection()) }?;
    }
    Ok(())
}
