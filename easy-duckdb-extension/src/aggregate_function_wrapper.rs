use crate::DuckResult;
use crate::duck_args_type::DuckArgs;
use crate::duck_register_builder::RegisterBuilder;
use crate::value_types::duck_value_type::DuckValueType;
use libduckdb_sys::{
    duckdb_aggregate_state, duckdb_connection, duckdb_data_chunk, duckdb_function_info,
    duckdb_vector, idx_t,
};
use quack_rs::aggregate::{AggregateFunctionBuilder, AggregateState, FfiState};
use quack_rs::data_chunk::DataChunk;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::AggregateFunctionInfo;

pub trait AggregateFunctionAdapter: AggregateState + Sized + 'static {
    unsafe extern "C" fn c_state_size(_info: duckdb_function_info) -> idx_t {
        FfiState::<Self>::size_callback(_info)
    }

    unsafe extern "C" fn c_state_init(info: duckdb_function_info, state: duckdb_aggregate_state) {
        unsafe { FfiState::<Self>::init_callback(info, state) };
    }

    unsafe extern "C" fn c_update(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        states: *mut duckdb_aggregate_state,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(_info) };
        let chunk = unsafe { DataChunk::from_raw(input) };
        let readers = Self::Args::create_readers(&chunk);
        let row_count = chunk.size();
        for row in 0..row_count {
            let args = Self::Args::read(&readers, row);
            let state_ptr = unsafe { *states.add(row) };
            if let Some(st) = unsafe { FfiState::<Self>::with_state_mut(state_ptr) } {
                let result = st.handle_row_with_null(args);
                if let Err(e) = result {
                    info.set_error(e.as_str());
                    return;
                }
            }
        }
    }

    unsafe extern "C" fn c_combine(
        _info: duckdb_function_info,
        source: *mut duckdb_aggregate_state,
        target: *mut duckdb_aggregate_state,
        count: idx_t,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(_info) };
        for i in 0..count as usize {
            let src_ptr = unsafe { *source.add(i) };
            let tgt_ptr = unsafe { *target.add(i) };
            let src = unsafe { FfiState::<Self>::with_state(src_ptr) };
            let tgt = unsafe { FfiState::<Self>::with_state_mut(tgt_ptr) };
            if let (Some(s), Some(t)) = (src, tgt) {
                let result = t.combine(s);
                if let Err(e) = result {
                    info.set_error(e.as_str());
                    return;
                }
            }
        }
    }

    unsafe extern "C" fn c_finalize(
        _info: duckdb_function_info,
        source: *mut duckdb_aggregate_state,
        result: duckdb_vector,
        count: idx_t,
        offset: idx_t,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(_info) };
        if offset != 0 {
            info.set_error(
                format!(
                    "non-zero aggregate finalize result offset is not supported: {}",
                    offset
                )
                .as_str(),
            );
            return;
        }

        let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let state_ptr = unsafe { *source.add(i) };
            match unsafe { FfiState::<Self>::with_state(state_ptr) } {
                Some(st) => unsafe {
                    let result1 = (st.result());
                    match result1 {
                        Ok(r) => output_vec.push(r),
                        Err(e) => {
                            info.set_error(e.as_str());
                            return;
                        }
                    };
                },
                None => output_vec.push(None),
            }
        }
        Self::Output::write_batch(result, &output_vec);
    }

    unsafe extern "C" fn c_state_destroy(states: *mut duckdb_aggregate_state, count: idx_t) {
        unsafe { FfiState::<Self>::destroy_callback(states, count) };
    }

    fn register_builder() -> AggregateFunctionBuilder {
        AggregateFunctionBuilder::new(Self::NAME)
            .state_size(Self::c_state_size)
            .init(Self::c_state_init)
            .update(Self::c_update)
            .combine(Self::c_combine)
            .finalize(Self::c_finalize)
            .destructor(Self::c_state_destroy)
            .with_return_type(Self::Output::logical_type())
            .with_params(Self::Args::params())
    }

    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        Self::register_builder().register(con)
    }

    const NAME: &'static str;

    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckArgs;
    type Output: DuckValueType;

    fn handle_row_with_null(&mut self, args: Option<Self::Args>) -> DuckResult<()> {
        if let Some(args) = args {
            self.handle_row(args)?;
        }
        Ok(())
    }
    fn handle_row(&mut self, args: Self::Args) -> DuckResult<()>;
    fn combine(&mut self, other: &Self) -> DuckResult<()>;
    fn result(&self) -> DuckResult<Option<Self::Output>>;
}
