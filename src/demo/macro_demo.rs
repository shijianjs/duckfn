use easy_duckdb_extension_macro::{duck_aggregate_function, duck_scalar_function, DuckStruct};

#[derive(Clone, Debug, DuckStruct)]
#[duck(named_param_from = "data")]
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

#[duck_aggregate_function]
fn word_count_w(input: Option<String>, arg2: i64,  state: &mut WcAggState)-> easy_duckdb_extension::DuckResult<()>  {
    todo!()
}

#[derive(Default, Debug, Clone)]
struct WcAggState {
    count: i64,
}
impl easy_duckdb_extension::DuckAggregateState for WcAggState {
    type Output = i64;

    fn combine(&mut self, other: &Self) -> easy_duckdb_extension::DuckResult<()> {
        todo!()
    }

    fn result(&self) -> easy_duckdb_extension::DuckOptionResult<i64> {
        todo!()
    }
}
