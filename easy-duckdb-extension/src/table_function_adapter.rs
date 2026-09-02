use crate::{
    DuckColumns, DuckOptionResult, DuckResult, DuckValueReader, DuckValueType, duck_error,
    duck_scalar_unwind, panic_to_duck_error, panic_to_string, vec_option_to_ref,
};
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    BindInfo, LogicalType, ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder,
    TableFunctionBuilder, TypeId,
};
use quack_rs::table::builder;
use quack_rs::vector::vector_size;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub type DuckDataIterator<T> = Box<dyn Iterator<Item = DuckOptionResult<T>> + Send>;
pub trait TableFunctionAdapter: Sized + 'static {
    fn table_function_builder() -> DuckResult<TableFunctionBuilder> {
        let mut builder = TableFunctionBuilder::new(Self::NAME);
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
        for (name, ty) in Self::Args::bind_param_logical() {
            if let Some(name) = name {
                builder = builder.named_param_logical(&name.into(), ty);
            } else {
                builder = builder.param_logical(ty)
            }
        }
        builder
    }

    fn with_state(
        bind: &BindInfo,
    ) -> DuckResult<DuckDataIterator<Self::Output>> {
        catch_unwind(|| {
            let args: Self::Args = Self::read_args(bind)?;
            Self::config_result_columns(bind, &args);
            let x: DuckDataIterator<Self::Output> =
                Box::new(Self::init_data_iterator(args)?);
            Ok(x)
        })
        .map_err(panic_to_duck_error)
        .flatten()
    }

    fn config_result_columns(bind: &BindInfo, args: &Self::Args) {
        for (name, ty) in Self::Output::named_column_types() {
            bind.add_result_column_with_type(&name.into(), &ty);
        }
    }

    fn read_args(bind: &BindInfo) -> DuckResult<Self::Args> {
        // let raw = unsafe { bind.get_named_parameter_value("start") };
        // let i = raw.as_i64();
        //         // let x = State {
        //         //     remaining: raw.as_i64_or(0).max(0) as u64,
        //         // };
        Self::Args::read_bind_args(bind)
    }

    //     pub fn scan<F>(mut self, f: F) -> Self
    //     where
    //         F: Fn(&mut S, &DataChunk) -> Result<(), ExtensionError> + Send + Sync + 'static,
    fn scan(
        state: &mut DuckDataIterator<Self::Output>,
        chunk: &DataChunk,
    ) -> DuckResult<()> {
        catch_unwind(AssertUnwindSafe(|| {
            let size = vector_size();
            // println!("size: {}", size);
            // let mut writer = unsafe { chunk.writer(0) };
            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(size as usize);

            let mut count = size;
            for i in 0..size {
                let option = state.next();
                if let Some(value) = option {
                    output_vec.push(value?);
                } else if let None = option {
                    count = i;
                    break;
                }
            }
            Self::Output::write_columns_batch(chunk, &vec_option_to_ref(&output_vec));
            unsafe { chunk.set_size(count as usize) };
            Ok(())
        }))
        .map_err(panic_to_duck_error)
        .flatten()
    }

    const NAME: &'static str;
    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckBindArgs;
    type Output: DuckColumns;
    // type DataIterator: Iterator<Item = DuckOptionResult<Self::Output>> + Send + 'static;

    fn init_data_iterator(
        args: Self::Args,
    ) -> DuckResult<DuckDataIterator<Self::Output>>;
}
pub trait DuckBindArgs: Sized {
    fn read_bind_args(bind: &BindInfo) -> DuckResult<Self>;

    fn bind_param_logical() -> Vec<(Option<impl Into<String>>, LogicalType)>;
}
