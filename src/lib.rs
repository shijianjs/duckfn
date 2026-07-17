// src/lib.rs

use duckdb::arrow::compute::unary;
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
            // let value = unsafe { reader.read_i64(row) };
            // unsafe { writer.write_i64(row, Self::apply(value)) };
            Self::handle_row(row, &reader, &mut writer);
        }
    }
    fn handle_row(row: usize, reader: &VectorReader, writer: &mut VectorWriter);
    // fn apply(v: i64) -> i64;

    fn register_builder() -> ScalarFunctionBuilder;
}

trait DuckValueType {
    fn type_id() -> TypeId;
    fn read(reader: &VectorReader, row: usize) -> Option<Self>
    where
        Self: Sized;
    fn write(writer: &mut VectorWriter, row: usize, vo: Option<Self>)
    where
        Self: Sized;
}
impl DuckValueType for i64 {
    fn type_id() -> TypeId {
        TypeId::BigInt
    }
    fn read(reader: &VectorReader, row: usize) -> Option<Self> {
        if unsafe { reader.is_valid(row) } {
            unsafe { Some(reader.read_i64(row)) }
        } else {
            None
        }
    }
    fn write(writer: &mut VectorWriter, row: usize, vo: Option<Self>) {
        match vo {
            None => unsafe { writer.set_null(row) },
            Some(v) => unsafe { writer.write_i64(row, v) },
        }
    }
}
trait OneArgScalarFunctionAdapter {
    const NAME: &'static str;
    type Arg1Type: DuckValueType;
    type ResultType: DuckValueType;
    fn apply(v: Self::Arg1Type) -> Self::ResultType {
        todo!("需要实现")
    }
    fn applyHandleNull(v: Option<Self::Arg1Type>) -> Option<Self::ResultType> {
        v.map(|v| Self::apply(v))
    }
}

impl<T> ScalarFunctionAdapter for T
where
    T: OneArgScalarFunctionAdapter,
{
    fn handle_row(row: usize, reader: &VectorReader, writer: &mut VectorWriter) {
        // if unsafe { !reader.is_valid(row) } {
        //     unsafe { writer.set_null(row) };
        //     continue;
        // }
        let value = <Self as OneArgScalarFunctionAdapter>::Arg1Type::read(reader, row);
        let f = Self::applyHandleNull(value);
        <Self as OneArgScalarFunctionAdapter>::ResultType::write(writer, row, f);
    }
    fn register_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .param(<Self as OneArgScalarFunctionAdapter>::Arg1Type::type_id())
            .returns(<Self as OneArgScalarFunctionAdapter>::ResultType::type_id())
            .function(scalar_function_wrapper::<T>)
    }
}

struct DoubleIt;

impl OneArgScalarFunctionAdapter for DoubleIt {
    const NAME: &'static str = "double_it5";
    type Arg1Type = i64;
    type ResultType = i64;
    fn apply(v: i64) -> i64 {
        v * 2
    }
}

unsafe extern "C" fn scalar_function_wrapper<T: ScalarFunctionAdapter>(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    T::run(_info, input, output);
}

fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let con: duckdb_connection = connection.as_raw_connection();
    unsafe {
        DoubleIt::register_builder().register(con)?;
    }
    Ok(())
}

entry_point_v2!(rusty_quack_init_c_api, register);
