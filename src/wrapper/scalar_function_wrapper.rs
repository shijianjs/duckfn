use crate::wrapper::duck_args_type::DuckArgs;
use crate::wrapper::duck_register_builder::RegisterBuilder;
use crate::wrapper::duck_value_type_convertor::DuckValueType;
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{
    ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder, VectorWriter,
};

pub trait ScalarFunctionAdapter: Sized + 'static {
    unsafe extern "C" fn scalar_function_wrapper(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
        let info = unsafe { ScalarFunctionInfo::new(_info) };

        // SAFETY: input is a valid data chunk provided by DuckDB.
        // let reader = unsafe { VectorReader::new(input, 0) };
        let chunk: DataChunk = unsafe { DataChunk::from_raw(input) };



        // let readers = (0..chunk.column_count())
        //     .map(|i| unsafe { chunk.reader(i) })
        //     .collect::<Vec<_>>();
        let readers = Self::Args::create_readers(&chunk);
        let mut writer = Self::Output::create_writer(output);
        let row_count = chunk.size();

        for row in 0..row_count {
            let args = Self::Args::read(&readers, row);
            let result = Self::apply(args);
            Self::Output::write(&mut writer, row, result);
        }
    }
    fn register_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .function(Self::scalar_function_wrapper)
            .with_return_type(Self::Output::type_info())
            .with_params(Self::Args::params())
    }
    fn register_overload_builder() -> ScalarOverloadBuilder {
        ScalarOverloadBuilder::new()
            // .returns(Self::Output::type_id())
            .function(Self::scalar_function_wrapper)
            .with_return_type(Self::Output::type_info())
            .with_params(Self::Args::params())
    }

    unsafe fn register(con: duckdb_connection) -> Result<(), ExtensionError> {
        Self::register_builder().register(con)
    }

    const NAME: &'static str;
    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckArgs;
    type Output: DuckValueType;

    fn apply(args: Self::Args) -> Option<Self::Output>;
}
