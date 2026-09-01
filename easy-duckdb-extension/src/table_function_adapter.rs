use crate::{duck_error, duck_scalar_unwind, DuckArgs, DuckOptionResult, DuckResult, DuckValueReader, DuckValueType, panic_to_string, panic_to_duck_error};
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    BindInfo, LogicalType, ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder,
    TableFunctionBuilder, TypeId,
};
use quack_rs::table::builder;
use quack_rs::vector::vector_size;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub trait TableFunctionAdapter: Sized + 'static {
    fn table_function_builder() -> DuckResult<TableFunctionBuilder> {
        let mut builder = TableFunctionBuilder::new("count_down");
        builder = Self::config_params(builder);
        // 1. bind closure: declare the output schema, read parameters,
        //    return the initial scan state.
        builder
            .with_state(Self::with_state)
            // 2. scan closure: mutate state, write rows, set chunk size.
            // .scan(|state, chunk| Self::scan(state, chunk))
            .scan(Self::scan)
            .build()
    }

    fn config_params(builder: TableFunctionBuilder) -> TableFunctionBuilder {
        let mut builder = builder;
        for ty in Self::Args::param_logical() {
            builder = builder.param_logical(ty);
        }
        for (name, ty) in Self::Args::named_param_logical() {
            builder = builder.named_param_logical(&name, ty);
        }
        builder
    }

    fn with_state(bind: &BindInfo) -> DuckResult<Self::DataIterator> {
        catch_unwind(|| {
            let args: Self::Args = Self::read_args(bind)?;
            Self::config_result_columns(bind, &args);
            Self::init_data_iterator(args)
        }).map_err(panic_to_duck_error).flatten()
    }

    fn config_result_columns(bind: &BindInfo, args: &Self::Args) {
        bind.add_result_column("n", TypeId::BigInt);
    }

    fn read_args(bind: &BindInfo) -> DuckResult<Self::Args> {
        let raw = unsafe { bind.get_named_parameter_value("start") };
        let i = raw.as_i64();
        //         // let x = State {
        //         //     remaining: raw.as_i64_or(0).max(0) as u64,
        //         // };
        Self::Args::read_args(bind)
    }

    //     pub fn scan<F>(mut self, f: F) -> Self
    //     where
    //         F: Fn(&mut S, &DataChunk) -> Result<(), ExtensionError> + Send + Sync + 'static,
    fn scan(state: &mut Self::DataIterator, chunk: &DataChunk) -> DuckResult<()> {
        catch_unwind(AssertUnwindSafe(|| {
            let size = vector_size();
            // println!("size: {}", size);
            let mut writer = unsafe { chunk.writer(0) };
            for i in 0..size {
                let option = state.next();
                match option {
                    Some(value) => {
                        // unsafe { writer.write_i64(i as usize, value) } ;
                        todo!()
                    }
                    None => {
                        unsafe { chunk.set_size(i as usize) };
                        return Ok(());
                    }
                }
            }
            unsafe { chunk.set_size(size as usize) };
            Ok(())
        })).map_err(panic_to_duck_error).flatten()
    }

    const NAME: &'static str;
    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckBindArgs;
    type Output: DuckValueType;
    type DataIterator: Iterator<Item = Self::Output> + Send + Sync + 'static;

    fn init_data_iterator(args: Self::Args) -> DuckResult<Self::DataIterator>;
}
pub trait DuckBindArgs: Sized {
    fn read_args(bind: &BindInfo) -> DuckResult<Self>;

    fn param_logical() -> Vec<LogicalType>;
    fn named_param_logical() -> Vec<(String, LogicalType)>;
}
