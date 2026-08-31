use easy_duckdb_extension_macro::{duck_scalar_function, DuckStruct};

#[derive(Clone, Debug, DuckStruct)]
pub struct DuckStructDemo1 {
    pub count: i64,
    pub data: Vec<i64>,
    pub age: Option<i32>,
    pub nest_data: Option<Vec<Vec<i64>>>,
}

#[duck_scalar_function]
fn error_scalar_demo(input: i64,input2: i64) -> easy_duckdb_extension::DuckOptionResult<i64> {
    Ok(Some(input * 2))
}
