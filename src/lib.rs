use duckdb::{
    core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId},
    duckdb_entrypoint_c_api,
    ffi::duckdb_string_t,
    types::DuckString,
    vscalar::{ScalarFunctionSignature, VScalar},
    vtab::{arrow::WritableVector, BindInfo, InitInfo, TableFunctionInfo, VTab},
    Connection, Result,
};
use std::{
    error::Error,
    ffi::CString,
    sync::atomic::{AtomicBool, Ordering},
};
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use duckdb::ffi::{duckdb_aggregate_state, duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector, idx_t};
use quack_rs::aggregate::{AggregateFunctionBuilder, AggregateState, FfiState};
use quack_rs::types::TypeId;
use quack_rs::vector::{VectorReader, VectorWriter};

struct EchoScalar;

impl VScalar for EchoScalar {
    type State = ();

    fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_vec = input.flat_vector(0);
        let values = unsafe { input_vec.as_slice_with_len::<duckdb_string_t>(input.len()) };
        let mut output = output.flat_vector();

        for (i, value) in values.iter().enumerate() {
            if input_vec.row_is_null(i as u64) {
                output.set_null(i);
                continue;
            }

            let mut value = *value;
            let s = DuckString::new(&mut value).as_str();
            output.insert(i, format!("🐤 {s} 🦀 {s}").as_str());
        }
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![LogicalTypeId::Varchar.into()],
            LogicalTypeId::Varchar.into(),
        )]
    }
}

#[repr(C)]
struct HelloBindData {
    name: String,
}

#[repr(C)]
struct HelloInitData {
    done: AtomicBool,
}

struct HelloVTab;

impl VTab for HelloVTab {
    type InitData = HelloInitData;
    type BindData = HelloBindData;

    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        bind.add_result_column("column0", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        let name = bind.get_parameter(0).to_string();
        Ok(HelloBindData { name })
    }

    fn init(_: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(HelloInitData {
            done: AtomicBool::new(false),
        })
    }

    fn func(
        func: &TableFunctionInfo<Self>,
        output: &mut DataChunkHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let init_data = func.get_init_data();
        let bind_data = func.get_bind_data();
        if init_data.done.swap(true, Ordering::Relaxed) {
            output.set_len(0);
        } else {
            let vector = output.flat_vector(0);
            let result = CString::new(format!("Rusty Quack {} 🐥", bind_data.name))?;
            vector.insert(0, result);
            output.set_len(1);
        }
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }
}


#[derive(Default, Debug)]
struct WordCountState {
    count: i64,
}

impl AggregateState for WordCountState {

}
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
/// Counts whitespace-separated words in a string.
pub fn count_words(s: &str) -> i64 {
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
            None     => unsafe { writer.set_null(offset as usize + i) },
        }
    }
}
unsafe extern "C" fn wc_state_destroy(
    states: *mut duckdb_aggregate_state,
    count: idx_t,
) {
    unsafe { FfiState::<WordCountState>::destroy_callback(states, count) };
}



// 只定义你需要的前缀布局
#[repr(C)]
struct FakeInnerConnection {
    _database: Arc<Mutex<()>>,
    con: duckdb_connection,
}

#[repr(C)]
struct FakeConnection {
    db: RefCell<FakeInnerConnection>,
}

unsafe fn raw_connection(conn: &Connection) -> duckdb_connection {
    let fake = &*(conn as *const Connection as *const FakeConnection);

    (*fake.db.as_ptr()).con
}

#[duckdb_entrypoint_c_api]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    con.register_scalar_function::<EchoScalar>("rusty_echo")?;
    con.register_table_function::<HelloVTab>("rusty_quack")?;
    unsafe {
        let connection = raw_connection(&con);
        AggregateFunctionBuilder::new("word_count")
            .param(TypeId::Varchar)
            .returns(TypeId::BigInt)
            .state_size(wc_state_size)
            .init(wc_state_init)
            .update(wc_update)
            .combine(wc_combine)
            .finalize(wc_finalize)
            .destructor(wc_state_destroy)
            .register(connection)?;
    }
    Ok(())
}
