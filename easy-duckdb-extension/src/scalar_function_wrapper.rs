use crate::{panic_to_string, DuckResult, duck_scalar_unwind, DuckOptionResult};
use crate::duck_args_type::DuckArgs;
use crate::duck_register_builder::RegisterBuilder;
use crate::value_types::duck_value_type::DuckValueType;
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder};
use std::panic::catch_unwind;

pub trait ScalarFunctionAdapter: Sized + 'static {
    unsafe extern "C" fn scalar_function_wrapper(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
        let info: ScalarFunctionInfo = unsafe { ScalarFunctionInfo::new(_info) };
        duck_scalar_unwind(&info,|| {
            let chunk: DataChunk = unsafe { DataChunk::from_raw(input) };
            let readers = Self::Args::create_arg_readers(&chunk);
            let row_count = chunk.size();

            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(row_count);
            for row in 0..row_count {
                let args = Self::Args::read_args(&readers, row);
                let result = Self::apply_with_null(args);
                match result {
                    Ok(r) => output_vec.push(r),
                    Err(e) => {
                        info.set_error(e.as_str());
                        return;
                    }
                }
            }
            Self::Output::write_batch(output, &output_vec);
        });
    }
    fn register_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .function(Self::scalar_function_wrapper)
            .with_return_type(Self::Output::logical_type())
            .with_params(Self::Args::arg_types())
    }
    fn register_overload_builder() -> ScalarOverloadBuilder {
        ScalarOverloadBuilder::new()
            .function(Self::scalar_function_wrapper)
            .with_return_type(Self::Output::logical_type())
            .with_params(Self::Args::arg_types())
    }

    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        Self::register_builder().register(con)
    }

    const NAME: &'static str;
    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckArgs;
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
