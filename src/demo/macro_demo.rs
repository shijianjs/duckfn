use duckfn::{duck_aggregate_function, duck_scalar_function, duck_table_function, DuckStruct};

#[derive(Clone, Default,  Debug, DuckStruct)]
#[duck(named_param_from = "data")]
pub struct DuckStructDemo1 {
    pub count: i64,
    pub data: Vec<i64>,
    pub age: Option<i32>,
    pub nest_data: Option<Vec<Vec<i64>>>,
}

#[duck_scalar_function]
fn error_scalar_demo(input: i64,input2: i64) -> duckfn::DuckOptionResult<i64> {
    Ok(Some(input * 2))
}

#[duck_aggregate_function]
fn word_count_w(input: Option<String>, arg2: i64,  state: &mut WcAggState)-> duckfn::DuckResult<()>  {
    todo!()
}

#[derive(Default, Debug, Clone)]
struct WcAggState {
    count: i64,
}
impl duckfn::DuckAggregateState for WcAggState {
    type Output = i64;

    fn combine(&mut self, other: &Self) -> duckfn::DuckResult<()> {
        todo!()
    }

    fn result(&self) -> duckfn::DuckOptionResult<i64> {
        todo!()
    }
}

#[duck_table_function]
fn table_fun_demo(start: i64) -> impl Iterator<Item =CountDownOutput> {
    (0..start).rev()
        .map(|x| CountDownOutput { n: x })
}
#[derive(Default, Debug, Clone, duckfn::DuckStruct)]
pub struct CountDownOutput {
    n: i64,
}