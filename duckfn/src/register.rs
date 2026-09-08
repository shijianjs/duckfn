use crate::DuckResult;
use quack_rs::connection::Connection;

pub type DuckRegisterFn = fn(connection: &Connection) -> DuckResult<()>;

pub struct DuckFunctionItem{
    pub register_fn: DuckRegisterFn
}
inventory::collect!(DuckFunctionItem);

pub fn register_all_duckfn(connection: &Connection) ->DuckResult<()>{
    for item in inventory::iter::<DuckFunctionItem>(){
        (item.register_fn)(connection)?;
    }
    Ok(())
}