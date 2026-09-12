use crate::duck_columns::DuckColumns;
use crate::utils::builder_with_params::BuilderWithParams;
use crate::value_types::duck_value_type::DuckValueType;
use crate::{DuckOptionResult, DuckResult, duck_scalar_unwind, vec_option_to_ref};
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder};

pub trait ScalarFunctionAdapter: Sized + 'static {
    unsafe extern "C" fn scalar_function_wrapper(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
        let info: ScalarFunctionInfo = unsafe { ScalarFunctionInfo::new(_info) };
        duck_scalar_unwind(&info,|| {
            let chunk: DataChunk = unsafe { DataChunk::from_raw(input) };
            let readers = Self::Args::create_column_readers(&chunk);
            let row_count = chunk.size();

            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(row_count);
            for row in 0..row_count {
                let args = Self::Args::read_columns(&readers, row);
                let result = Self::apply_with_null(args);
                match result {
                    Ok(r) => output_vec.push(r),
                    Err(e) => {
                        info.set_error(e.as_str());
                        return;
                    }
                }
            }
            Self::Output::write_batch(output, &vec_option_to_ref(&output_vec));
        });
    }
    fn scalar_function_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .function(Self::scalar_function_wrapper)
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types())
    }
    fn scalar_overload_builder() -> ScalarOverloadBuilder {
        ScalarOverloadBuilder::new()
            .function(Self::scalar_function_wrapper)
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types())
    }

    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        Self::scalar_function_builder().register(con)
    }

    const NAME: &'static str;
    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckColumns;
    type Output: DuckValueType;

    fn apply_with_null(args_option: Option<Self::Args>) -> DuckOptionResult<Self::Output> {
        if let Some(args) = args_option {
            Self::apply(args)
        } else {
            Ok(None)
        }
    }
    fn apply(args: Self::Args) -> DuckOptionResult<Self::Output>;
}
