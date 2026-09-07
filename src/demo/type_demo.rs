use duckfn::DuckStruct;

#[derive(Clone, Default,  Debug, /*DuckStruct*/)]
// #[duck(named_param_from = "data")]
pub struct DuckStructDemo1 {
    pub count: i64,
    pub data: Vec<i64>,
    pub age: Option<i32>,
    pub nest_data: Option<Vec<Vec<i64>>>,
}

