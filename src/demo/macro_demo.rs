use easy_duckdb_extension_macro::DuckStruct;

#[derive(Clone, Debug, DuckStruct)]
pub struct DuckStructDemo1 {
    pub count: i64,
    pub data: Vec<i64>,
    pub age: Option<i32>,
    pub nest_data: Option<Vec<Vec<i64>>>,
}
