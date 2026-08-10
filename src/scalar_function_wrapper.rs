use crate::duck_args_type::DuckArgs;
use crate::duck_value_type_convertor::DuckValueType;
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{ScalarFunctionBuilder, ScalarFunctionInfo, VectorReader, VectorWriter};

pub trait ScalarFunctionAdapter: Sized + 'static {
    unsafe extern "C" fn scalar_function_wrapper(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
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
    fn handle_row(row: usize, readers: &[VectorReader], writer: &mut VectorWriter) {
        let args = Self::Args::read(readers, row);
        let result = Self::apply(args);
        Self::Output::write(writer, row, result);
    }
    fn register_builder() -> ScalarFunctionBuilder {
        let mut builder = ScalarFunctionBuilder::new(Self::NAME)
            .returns(Self::Output::type_id())
            .function(Self::scalar_function_wrapper);
        for x in Self::Args::params() {
            builder = builder.param(x);
        }
        builder
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
