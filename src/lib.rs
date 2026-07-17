// src/lib.rs

use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::connection::Connection;
use quack_rs::error::ExtensionError;
use quack_rs::scalar::builder::ScalarFn;
use quack_rs::scalar::ScalarFunctionBuilder;
use quack_rs::types::TypeId;
use quack_rs::vector::{VectorReader, VectorWriter};
use quack_rs::{entry_point, entry_point_v2};

fn double_it(value: i64) -> i64 {
    value * 2
}

trait ScalarFunctionAdapter {

    unsafe fn run(_info: duckdb_function_info, input: duckdb_data_chunk, output: duckdb_vector) {
        // SAFETY: input is a valid data chunk provided by DuckDB.
        let reader = unsafe { VectorReader::new(input, 0) };
        let mut writer = unsafe { VectorWriter::new(output) };
        let row_count = reader.row_count();

        for row in 0..row_count {
            if unsafe { !reader.is_valid(row) } {
                unsafe { writer.set_null(row) };
                continue;
            }
            // let value = unsafe { reader.read_i64(row) };
            // unsafe { writer.write_i64(row, Self::apply(value)) };
            Self::handle_row(row, &reader, &mut writer);
        }
    }
    fn handle_row(row:usize, reader: &VectorReader, writer: &mut VectorWriter);
    // fn apply(v: i64) -> i64;
}

trait DuckValueType {
    fn type_id() -> TypeId;
    fn read(reader: &VectorReader, row: usize) -> Self;
    fn write(writer: &mut VectorWriter, row: usize, v: Self);
}
trait OneArgumentScalarFunctionAdapter {
    type Arg1Type: DuckValueType;
    type ResultType: DuckValueType;
    fn apply(v: Self::Arg1Type) -> Self::ResultType ;
}
impl<T> ScalarFunctionAdapter for T
where
    T: OneArgumentScalarFunctionAdapter,
{

    fn handle_row(row:usize, reader: &VectorReader, writer: &mut VectorWriter) {
        let value = <Self as OneArgumentScalarFunctionAdapter>::Arg1Type::read(reader, row);
        let f = Self::apply(value);
        <Self as OneArgumentScalarFunctionAdapter>::ResultType::write(writer, row, f);
    }
}

impl DuckValueType for i64 {
    fn type_id() -> TypeId {
        TypeId::BigInt
    }
    fn read(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i64(row) }
    }
    fn write(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i64(row, v) };
    }
}

struct DoubleIt;


impl OneArgumentScalarFunctionAdapter for DoubleIt {
    type Arg1Type = i64;
    type ResultType = i64;
    fn apply(v: i64) -> i64 {
        v * 2
    }
}

/// Scalar function: double_it(BIGINT) → BIGINT
unsafe extern "C" fn scalar_function_wrapper<T: ScalarFunctionAdapter>(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    T::run(_info, input, output);
}

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let con = connection.as_raw_connection();
    unsafe {
        ScalarFunctionBuilder::new("double_it5")
            .param(TypeId::BigInt)
            .returns(TypeId::BigInt)
            .function(scalar_function_wrapper::<DoubleIt>)
            .register(con)?;
    }
    Ok(())
}

entry_point_v2!(rusty_quack_init_c_api, register);
