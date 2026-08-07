use crate::scalar_function_wrapper::ScalarFunctionAdapter;
use libduckdb_sys::{
    duckdb_aggregate_state, duckdb_connection, duckdb_data_chunk, duckdb_function_info,
    duckdb_vector, idx_t,
};
use quack_rs::prelude::{
    AggregateFunctionBuilder, AggregateState, ExtensionError, FfiState, ScalarFunctionBuilder,
    TypeId, VectorReader, VectorWriter,
};

unsafe extern "C" fn wrapper_state_size<T: AggregateFunctionAdapter<T, K>, K>(
    _info: duckdb_function_info,
) -> idx_t {
    FfiState::<T>::size_callback(_info)
}
unsafe extern "C" fn wrapper_state_init<T: AggregateFunctionAdapter<T, K>, K>(
    info: duckdb_function_info,
    state: duckdb_aggregate_state,
) {
    unsafe { FfiState::<T>::init_callback(info, state) };
}
unsafe extern "C" fn wrapper_state_destroy<T: AggregateFunctionAdapter<T, K>, K>(
    states: *mut duckdb_aggregate_state,
    count: idx_t,
) {
    unsafe { FfiState::<T>::destroy_callback(states, count) };
}

unsafe extern "C" fn wrapper_update<T: AggregateFunctionAdapter<T, K>, K>(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    states: *mut duckdb_aggregate_state,
) {
    T::duck_update(_info, input, states);
}

unsafe extern "C" fn wrapper_combine<T: AggregateFunctionAdapter<T, K>, K>(
    _info: duckdb_function_info,
    source: *mut duckdb_aggregate_state,
    target: *mut duckdb_aggregate_state,
    count: idx_t,
) {
    T::duck_combine(_info, source, target, count);
}

unsafe extern "C" fn wrapper_finalize<T: AggregateFunctionAdapter<T, K>, K>(
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

pub trait AggregateFunctionAdapter<T: AggregateFunctionAdapter<T, K>, K>: AggregateState {
    fn duck_update(
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

    fn duck_combine(
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

    fn wrapper_finalize(
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
    fn register_builder() -> AggregateFunctionBuilder;

    unsafe fn register(con: duckdb_connection) -> Result<(), ExtensionError> {
        Self::register_builder()
            .state_size(wrapper_state_size::<T, K>)
            .init(wrapper_state_init::<T, K>)
            .update(wrapper_update::<T, K>)
            .combine(wrapper_combine::<T, K>)
            .finalize(wrapper_finalize::<T, K>)
            .destructor(wrapper_state_destroy::<T, K>)
            .register(con)?;
        Ok(())
    }
}

struct OneArg;

/// ============= demo wrapper  ============
///

#[derive(Default, Debug, Clone)]
struct WordCountStateWrapper {
    count: i64,
}

impl AggregateState for WordCountStateWrapper {}

impl AggregateFunctionAdapter<WordCountStateWrapper, OneArg> for WordCountStateWrapper {
    fn register_builder() -> AggregateFunctionBuilder {
        AggregateFunctionBuilder::new("word_count_wrapper")
            .param(TypeId::Varchar)
            .returns(TypeId::BigInt)
    }
}

/// ============= demo 原始的 ============

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

pub unsafe fn register_aggregate_demo(
    con: libduckdb_sys::duckdb_connection,
) -> Result<(), ExtensionError> {
    unsafe {
        AggregateFunctionBuilder::new("word_count")
            .param(TypeId::Varchar)
            .returns(TypeId::BigInt)
            .state_size(wc_state_size)
            .init(wc_state_init)
            .update(wc_update)
            .combine(wc_combine)
            .finalize(wc_finalize)
            .destructor(wc_state_destroy)
            .register(con)?;

        WordCountStateWrapper::register(con)?;
    }
    Ok(())
}
