use easy_duckdb_extension::aggregate_function_wrapper::AggregateFunctionAdapter;
use easy_duckdb_extension::value_types::duck_list::DuckList;
use libduckdb_sys::{
    duckdb_aggregate_state, duckdb_data_chunk, duckdb_function_info, duckdb_vector, idx_t,
};
use quack_rs::connection::Connection;
use quack_rs::prelude::{
    AggregateFunctionBuilder, AggregateState, ExtensionError, FfiState, Registrar, TypeId, VectorReader, VectorWriter,
};
use tuple_transpose::TupleTranspose;
use easy_duckdb_extension::{DuckOptionResult, DuckResult};

/// ============= demo wrapper封装版  ============
///

#[derive(Default, Debug, Clone)]
struct WordCountStateWrapper {
    count: i64,
}

impl AggregateState for WordCountStateWrapper {}

impl AggregateFunctionAdapter for WordCountStateWrapper {
    const NAME: &'static str = "word_count_w";
    type Args = (Option<String>,);
    type Output = i64;
    fn handle_row(&mut self, args: Self::Args) -> DuckResult<()>{
        self.count += args.transpose().map(|(t,)| count_words(&t)).unwrap_or(0);
        Ok(())
    }

    fn combine(&mut self, other: &Self) -> DuckResult<()> {
        self.count += other.count;
        Ok(())
    }

    fn result(&self) -> DuckOptionResult<Self::Output> {
        Ok(Some(self.count))
    }
}

#[derive(Default, Debug, Clone)]
struct AggListWrapper{
    li:DuckList<i64>
}
impl AggregateState for AggListWrapper {}
impl AggregateFunctionAdapter for AggListWrapper {
    const NAME: &'static str = "agg_list_w";
    type Args = (Option<i64>,);
    type Output = DuckList<i64>;
    fn handle_row(&mut self, args: Self::Args) -> DuckResult<()>{
        let value = args.0;
        if let Some(12)=value{
            return Err(ExtensionError::new("Value is 12"));
        }
        self.li.value.push(value);
        Ok(())
    }

    fn combine(&mut self, other: &Self) -> DuckResult<()> {
        self.li.value.extend(other.li.value.iter().cloned());
        Ok(())
    }

    fn result(&self) -> DuckOptionResult<Self::Output> {
        Ok(Some(self.li.clone()))
    }
}




/// ============= demo 官方的 ============

#[derive(Default, Debug, Clone)]
struct WordCountState {
    count: i64,
}

impl AggregateState for WordCountState {}

unsafe extern "C" fn wc_state_size(_info: duckdb_function_info) -> idx_t {
    FfiState::<WordCountState>::size_callback(_info)
}

unsafe extern "C" fn wc_state_init(info: duckdb_function_info, state: duckdb_aggregate_state) {
    unsafe { FfiState::<WordCountState>::init_callback(info, state) };
}

unsafe extern "C" fn wc_update(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    states: *mut duckdb_aggregate_state,
) {
    let reader = unsafe { VectorReader::new(input, 0) };
    let row_count = reader.row_count();

    for row in 0..row_count {
        if !unsafe { reader.is_valid(row) } {
            continue; // NULL input → skip (contributes 0 words)
        }
        let s = unsafe { reader.read_str(row) };
        let words = count_words(s);

        let state_ptr = unsafe { *states.add(row) };
        if let Some(st) = unsafe { FfiState::<WordCountState>::with_state_mut(state_ptr) } {
            st.count += words;
        }
    }
}

fn count_words(s: &str) -> i64 {
    s.split_whitespace().count() as i64
}

unsafe extern "C" fn wc_combine(
    _info: duckdb_function_info,
    source: *mut duckdb_aggregate_state,
    target: *mut duckdb_aggregate_state,
    count: idx_t,
) {
    for i in 0..count as usize {
        let src_ptr = unsafe { *source.add(i) };
        let tgt_ptr = unsafe { *target.add(i) };
        let src = unsafe { FfiState::<WordCountState>::with_state(src_ptr) };
        let tgt = unsafe { FfiState::<WordCountState>::with_state_mut(tgt_ptr) };
        if let (Some(s), Some(t)) = (src, tgt) {
            t.count += s.count;
            // If you add fields to WordCountState, combine them here too.
        }
    }
}

unsafe extern "C" fn wc_finalize(
    _info: duckdb_function_info,
    source: *mut duckdb_aggregate_state,
    result: duckdb_vector,
    count: idx_t,
    offset: idx_t,
) {
    let mut writer = unsafe { VectorWriter::new(result) };

    for i in 0..count as usize {
        let state_ptr = unsafe { *source.add(i) };
        match unsafe { FfiState::<WordCountState>::with_state(state_ptr) } {
            Some(st) => unsafe { writer.write_i64(offset as usize + i, st.count) },
            None => unsafe { writer.set_null(offset as usize + i) },
        }
    }
}

unsafe extern "C" fn wc_state_destroy(states: *mut duckdb_aggregate_state, count: idx_t) {
    unsafe { FfiState::<WordCountState>::destroy_callback(states, count) };
}

pub unsafe fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let builders = vec![
        AggregateFunctionBuilder::new("word_count")
            .param(TypeId::Varchar)
            .returns(TypeId::BigInt)
            .state_size(wc_state_size)
            .init(wc_state_init)
            .update(wc_update)
            .combine(wc_combine)
            .finalize(wc_finalize)
            .destructor(wc_state_destroy),
        WordCountStateWrapper::register_builder(),
        AggListWrapper::register_builder(),
    ];
    for builder in builders {
        unsafe { connection.register_aggregate(builder) }?;
    }
    Ok(())
}
