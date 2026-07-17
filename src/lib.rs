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

trait ScalarFunctionAdapter<K> {
    fn run(_info: duckdb_function_info, input: duckdb_data_chunk, output: duckdb_vector) {
        // SAFETY: input is a valid data chunk provided by DuckDB.
        // let reader = unsafe { VectorReader::new(input, 0) };
        let readers = (0..Self::COLUMN_COUNT)
            .map(|i| unsafe { VectorReader::new(input, i) })
            .collect::<Vec<_>>();
        let mut writer = unsafe { VectorWriter::new(output) };
        let row_count = readers[0].row_count();

        for row in 0..row_count {
            Self::handle_row(row, &readers, &mut writer);
        }
    }
    const COLUMN_COUNT: usize;
    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter);

    fn register_builder() -> ScalarFunctionBuilder;
}

trait DuckValueType: Sized {
    fn type_id() -> TypeId;
    fn read(reader: &VectorReader, row: usize) -> Option<Self> {
        if unsafe { reader.is_valid(row) } {
            unsafe { Some(Self::read_valid(reader, row)) }
        } else {
            None
        }
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self;

    fn write(writer: &mut VectorWriter, row: usize, vo: Option<Self>) {
        match vo {
            None => unsafe { writer.set_null(row) },
            Some(v) => Self::write_not_null(writer, row, v),
        }
    }
    fn write_not_null(writer: &mut VectorWriter, row: usize, v: Self);
}
impl DuckValueType for i64 {
    fn type_id() -> TypeId {
        TypeId::BigInt
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i64(row) }
    }
    fn write_not_null(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i64(row, v) }
    }
}

impl DuckValueType for String {
    fn type_id() -> TypeId {
        TypeId::Varchar
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_str(row).to_string() }
    }
    fn write_not_null(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_str(row, v.as_str()) }
    }
}

trait OneArgScalarFunctionAdapter {
    const NAME: &'static str;
    type Arg1Type: DuckValueType;
    type ResultType: DuckValueType;
    /// 默认实现这个方法，空值已经映射为空值
    fn apply(v: Self::Arg1Type) -> Self::ResultType {
        todo!("需要实现")
    }
    /// 需要处理空值的话，需要实现这个方法，apply方法不用管
    fn applyHandleNull(v: Option<Self::Arg1Type>) -> Option<Self::ResultType> {
        v.map(|v| Self::apply(v))
    }
}
struct OneArg {}
impl<T: OneArgScalarFunctionAdapter> ScalarFunctionAdapter<OneArg> for T {
    const COLUMN_COUNT: usize = 1;
    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter) {
        let value = <Self as OneArgScalarFunctionAdapter>::Arg1Type::read(&readers[0], row);
        let f = Self::applyHandleNull(value);
        <Self as OneArgScalarFunctionAdapter>::ResultType::write(writer, row, f);
    }
    fn register_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .param(<Self as OneArgScalarFunctionAdapter>::Arg1Type::type_id())
            .returns(<Self as OneArgScalarFunctionAdapter>::ResultType::type_id())
            .function(scalar_function_wrapper::<T, OneArg>)
    }
}
struct TwoArg {}
trait TwoArgScalarFunctionAdapter {
    const NAME: &'static str;
    type Arg1Type: DuckValueType;
    type Arg2Type: DuckValueType;
    type ResultType: DuckValueType;
    /// 默认实现这个方法，空值已经映射为空值
    fn apply(v: Self::Arg1Type, v2: Self::Arg2Type) -> Self::ResultType {
        todo!("需要实现")
    }
    /// 需要处理空值的话，需要实现这个方法，apply方法不用管
    fn applyHandleNull(
        v: Option<Self::Arg1Type>,
        v2: Option<Self::Arg2Type>,
    ) -> Option<Self::ResultType> {
        v.zip(v2).map(|(v, v2)| Self::apply(v, v2))
    }
}

impl<T: TwoArgScalarFunctionAdapter> ScalarFunctionAdapter<TwoArg> for T {
    const COLUMN_COUNT: usize = 2;

    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter) {
        let value = <Self as TwoArgScalarFunctionAdapter>::Arg1Type::read(&readers[0], row);
        let value2 = <Self as TwoArgScalarFunctionAdapter>::Arg2Type::read(&readers[1], row);
        let f = Self::applyHandleNull(value, value2);
        <Self as TwoArgScalarFunctionAdapter>::ResultType::write(writer, row, f);
    }
    fn register_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .param(<Self as TwoArgScalarFunctionAdapter>::Arg1Type::type_id())
            .param(<Self as TwoArgScalarFunctionAdapter>::Arg2Type::type_id())
            .returns(<Self as TwoArgScalarFunctionAdapter>::ResultType::type_id())
            .function(scalar_function_wrapper::<T, TwoArg>)
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
struct AddIt;
impl TwoArgScalarFunctionAdapter for AddIt {
    const NAME: &'static str = "add_it5";
    type Arg1Type = i64;
    type Arg2Type = i64;
    type ResultType = i64;
    fn apply(v: i64, v2: i64) -> i64 {
        v + v2
    }
}

unsafe extern "C" fn scalar_function_wrapper<T: ScalarFunctionAdapter<K>, K>(
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
        AddIt::register_builder().register(con)?;
    }
    Ok(())
}

entry_point_v2!(rusty_quack_init_c_api, register);
