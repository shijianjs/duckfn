// src/lib.rs

use quack_rs::{entry_point, entry_point_v2};
use quack_rs::error::ExtensionError;
use quack_rs::scalar::ScalarFunctionBuilder;
use quack_rs::types::TypeId;
use quack_rs::vector::{VectorReader, VectorWriter};
use libduckdb_sys::{duckdb_connection, duckdb_function_info, duckdb_data_chunk, duckdb_vector};
use quack_rs::connection::Connection;
use quack_rs::scalar::builder::ScalarFn;

fn double_it(value:i64)->i64{
    value * 2
}

trait UnaryI64 {
    fn apply(v: i64) -> i64;
}
struct DoubleIt;

impl UnaryI64 for DoubleIt {

    fn apply(v: i64) -> i64 {
        v * 2
    }
}


fn create_fn<T: UnaryI64>() -> ScalarFn {
    /// Scalar function: double_it(BIGINT) → BIGINT
    unsafe extern "C" fn wrapper<T: UnaryI64>(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
        // SAFETY: input is a valid data chunk provided by DuckDB.
        let reader = unsafe { VectorReader::new(input, 0) };
        let mut writer = unsafe { VectorWriter::new(output) };
        let row_count = reader.row_count();

        for row in 0..row_count {
            if unsafe { !reader.is_valid(row) } {
                unsafe { writer.set_null(row) };
                continue;
            }
            let value = unsafe { reader.read_i64(row) };
            unsafe { writer.write_i64(row, T::apply(value)) };
        }
    }
    wrapper::<T>
}

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let con = connection.as_raw_connection();
    unsafe {
        ScalarFunctionBuilder::new("double_it5")
            .param(TypeId::BigInt)
            .returns(TypeId::BigInt)
            .function(create_fn::<DoubleIt>())
            .register(con)?;
    }
    Ok(())
}

entry_point_v2!(rusty_quack_init_c_api,  register);
