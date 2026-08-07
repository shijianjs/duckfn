use crate::value_type_convertor::DuckValueType;
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{
    ScalarFunctionBuilder, ScalarFunctionInfo, TypeId, VectorReader, VectorWriter,
};

pub unsafe extern "C" fn scalar_function_wrapper<T: ScalarFunctionAdapter<K>, K>(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    T::run(_info, input, output);
}

pub trait ScalarFunctionAdapter<K> {
    fn run(_info: duckdb_function_info, input: duckdb_data_chunk, output: duckdb_vector) {
        let info = unsafe { ScalarFunctionInfo::new(_info) };

        // SAFETY: input is a valid data chunk provided by DuckDB.
        // let reader = unsafe { VectorReader::new(input, 0) };
        let chunk = unsafe { DataChunk::from_raw(input) };
        let readers = (0..chunk.column_count())
            .map(|i| unsafe { chunk.reader(i) })
            .collect::<Vec<_>>();
        let mut writer = unsafe { VectorWriter::new(output) };
        let row_count = chunk.size();

        for row in 0..row_count {
            Self::handle_row(row, &readers, &mut writer);
        }
    }
    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter);

    fn register_builder() -> ScalarFunctionBuilder;

    unsafe fn register(con: duckdb_connection) -> Result<(), ExtensionError> {
        Self::register_builder().register(con)
    }
}

impl<T: OneArgScalarFunctionAdapter> ScalarFunctionAdapter<OneArg> for T {
    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter) {
        let value = <Self as OneArgScalarFunctionAdapter>::Arg1Type::read(&readers[0], row);
        let f = Self::apply_option(value);
        <Self as OneArgScalarFunctionAdapter>::ResultType::write(writer, row, f);
    }
    fn register_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .param(<Self as OneArgScalarFunctionAdapter>::Arg1Type::type_id())
            .returns(<Self as OneArgScalarFunctionAdapter>::ResultType::type_id())
            .function(scalar_function_wrapper::<T, OneArg>)
    }
}

impl<T: TwoArgScalarFunctionAdapter> ScalarFunctionAdapter<TwoArg> for T {

    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter) {
        let value = <Self as TwoArgScalarFunctionAdapter>::Arg1Type::read(&readers[0], row);
        let value2 = <Self as TwoArgScalarFunctionAdapter>::Arg2Type::read(&readers[1], row);
        let f = Self::apply_option(value, value2);
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

pub trait OneArgScalarFunctionAdapter {
    const NAME: &'static str;
    type Arg1Type: DuckValueType;
    type ResultType: DuckValueType;
    /// 默认实现这个方法，空值已经映射为空值
    fn apply(v: Self::Arg1Type) -> Self::ResultType {
        todo!("需要实现")
    }
    /// 需要处理空值的话，需要实现这个方法，apply方法不用管
    fn apply_option(v: Option<Self::Arg1Type>) -> Option<Self::ResultType> {
        v.map(|v| Self::apply(v))
    }
}

pub struct OneArg;

pub struct TwoArg;

pub trait TwoArgScalarFunctionAdapter {
    const NAME: &'static str;
    type Arg1Type: DuckValueType;
    type Arg2Type: DuckValueType;
    type ResultType: DuckValueType;
    /// 默认实现这个方法，空值已经映射为空值
    fn apply(v: Self::Arg1Type, v2: Self::Arg2Type) -> Self::ResultType {
        todo!("需要实现")
    }
    /// 需要处理空值的话，需要实现这个方法，apply方法不用管
    fn apply_option(
        v: Option<Self::Arg1Type>,
        v2: Option<Self::Arg2Type>,
    ) -> Option<Self::ResultType> {
        v.zip(v2).map(|(v, v2)| Self::apply(v, v2))
    }
}

pub trait DuckArgs : Sized{
    const COUNT: usize;

    fn read(readers: &[VectorReader], row: usize) -> Self;

    fn params() -> Vec<TypeId>;
}
impl<A: DuckValueType> DuckArgs for (Option<A>,) {
    const COUNT: usize = 1;

    fn read(readers: &[VectorReader], row: usize) -> Self {
        (A::read(&readers[0], row),)
    }

    fn params() -> Vec<TypeId> {
        vec![A::type_id()]
    }
}
impl<A: DuckValueType, B: DuckValueType> DuckArgs for (Option<A>, Option<B>) {
    const COUNT: usize = 2;

    fn read(readers: &[VectorReader], row: usize) -> Self {
        (A::read(&readers[0], row), B::read(&readers[1], row))
    }

    fn params() -> Vec<TypeId> {
        vec![A::type_id(), B::type_id()]
    }
}
pub trait ScalarFunction {
    const NAME: &'static str;
    type Args: DuckArgs;
    type Result: DuckValueType;

    fn apply(args: Self::Args) -> Option<Self::Result>;
}
pub struct TupleArg;

impl<T: ScalarFunction> ScalarFunctionAdapter<TupleArg> for T {

    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter) {
        let args = T::Args::read(readers, row);
        let result = T::apply(args);
        <Self as ScalarFunction>::Result::write(writer, row, result);
    }
    fn register_builder() -> ScalarFunctionBuilder {
        let mut builder = ScalarFunctionBuilder::new(Self::NAME)
            .returns(<Self as ScalarFunction>::Result::type_id())
            .function(scalar_function_wrapper::<T, TupleArg>);
        for x in T::Args::params() {
            builder = builder.param(x);
        }
        builder
    }
}
