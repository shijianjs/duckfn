use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_data_chunk_get_vector, duckdb_function_info, duckdb_vector};
use quack_rs::connection::Connection;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{ListVector, LogicalType, Registrar, ScalarFunctionBuilder, TypeId, VectorReader, VectorWriter};
use tuple_transpose::TupleTranspose;
use crate::aggregate_function_demo;
use crate::scalar_function_wrapper::{
    ScalarFunctionAdapter,
};


///
///
/// ```shell
/// cargo duckdb-ext build; duckdb -unsigned -c "
///   LOAD './target/debug/rusty_quack.duckdb_extension';
///   SELECT double_it5(3);
///   ";
/// ```
pub struct DoubleIt;

impl ScalarFunctionAdapter for DoubleIt {
    const NAME: &'static str = "double_it5";
    type Args = (Option<i64>,);
    type Output = i64;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| v * 2)
    }
}


pub struct FirstWordTuple;

impl ScalarFunctionAdapter for FirstWordTuple {
    const NAME: &'static str = "first_word_tuple";
    type Args = (Option<String>,);
    type Output = String;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| {
            v.as_str()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
    }
}

pub struct AddItTuple;

impl ScalarFunctionAdapter for AddItTuple {
    const NAME: &'static str = "add_it_tuple";
    type Args = (Option<i64>, Option<i64>);
    type Output = i64;
    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v, v2)| v + v2)
    }
}

// ============================================================================
// Scalar: sum_list(LIST(BIGINT)) → BIGINT
// ============================================================================

unsafe extern "C" fn sum_list_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    let reader = unsafe { VectorReader::new(input, 0) };
    let mut writer = unsafe { VectorWriter::new(output) };
    let row_count = reader.row_count();
    let list_vec = unsafe { duckdb_data_chunk_get_vector(input, 0) };

    for row in 0..row_count {
        if !unsafe { reader.is_valid(row) } {
            unsafe { writer.set_null(row) };
            continue;
        }
        let entry = unsafe { ListVector::get_entry(list_vec, row) };
        let child_vec = unsafe { ListVector::get_child(list_vec) };
        let total_elements = unsafe { ListVector::get_size(list_vec) };
        let child_reader = unsafe { VectorReader::from_vector(child_vec, total_elements) };

        let mut sum: i64 = 0;
        for i in 0..entry.length as usize {
            let idx = entry.offset as usize + i;
            if unsafe { child_reader.is_valid(idx) } {
                sum += unsafe { child_reader.read_i64(idx) };
            }
        }
        unsafe { writer.write_i64(row, sum) };
    }
}


pub unsafe fn register(connection: &Connection) -> Result<(), ExtensionError> {
    unsafe {
        let builders = vec![
            DoubleIt::register_builder(),
            FirstWordTuple::register_builder(),
            AddItTuple::register_builder(),
        ];

        for builder in builders {
            connection.register_scalar(builder)?;
        }
        // ── Scalar: sum_list (param_logical) ────────────────────────────
        connection.register_scalar(
            ScalarFunctionBuilder::new("sum_list")
                .param_logical(LogicalType::list(TypeId::BigInt))
                .returns(TypeId::BigInt)
                .function(sum_list_scalar),
        )?;
    }
    Ok(())
}