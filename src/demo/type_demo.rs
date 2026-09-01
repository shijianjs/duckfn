


/* #[duck_agg] */
fn word_count_w(input: Option<String>, arg2: i64, /* #[state]*/  state: &mut WcAggState)-> easy_duckdb_extension::DuckResult<()>  {
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

mod word_count_w{
    use super::*;

    #[derive(easy_duckdb_extension_macro::DuckStruct, Clone,)]
    pub struct DuckArgsImpl{
        input: Option<String>,
        arg2: i64,
    }


    #[derive(Default, Debug, Clone)]
    struct AggregateFunctionImpl {
        state: WcAggState,
    }

    impl quack_rs::prelude::AggregateState for AggregateFunctionImpl {}


    impl easy_duckdb_extension::AggregateFunctionAdapter for AggregateFunctionImpl {
        const NAME: &'static str = "word_count_w";
        type Args = DuckArgsImpl;
        type Output = <WcAggState as easy_duckdb_extension::DuckAggregateState>::Output;

        // #[duckdb_aggregate_function]
        fn handle_row(&mut self, args: Self::Args) -> easy_duckdb_extension::DuckResult<()> {
            word_count_w(args.input, args.arg2, &mut self.state)
        }

        fn combine(&mut self, other: &Self) -> easy_duckdb_extension::DuckResult<()> {
            use easy_duckdb_extension::{DuckAggregateState};
            self.state.combine(&other.state)
        }

        fn result(&self) -> easy_duckdb_extension::DuckOptionResult<Self::Output> {
            use easy_duckdb_extension::{DuckAggregateState};
            self.state.result()
        }
    }

}