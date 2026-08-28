use crate::value_types::duck_value_type::{ DuckValueReader, DuckValueType};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::LogicalType;

pub trait DuckArgs: Sized {
    fn create_arg_readers(chunk: &DataChunk) -> Vec<DuckValueReader>;

    fn read_args(readers: &[DuckValueReader], row: usize) -> Option<Self>;

    fn arg_types() -> Vec<LogicalType>;
}

impl<A: DuckValueType> DuckArgs for (Option<A>,) {
    fn create_arg_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        vec![A::create_reader(chunk, 0)]
    }
    

    fn read_args(readers: &[DuckValueReader], row: usize) -> Option<Self> {
        Some((A::read(&readers[0], row),))
    }

    fn arg_types() -> Vec<LogicalType> {
        vec![A::logical_type()]
    }
}

impl<A: DuckValueType, B: DuckValueType> DuckArgs for (Option<A>, Option<B>) {

    fn create_arg_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        vec![A::create_reader(chunk, 0), B::create_reader(chunk, 1)]
    }
    fn read_args(readers: &[DuckValueReader], row: usize) -> Option<Self> {
        Some((A::read(&readers[0], row), B::read(&readers[1], row)))
    }

    fn arg_types() -> Vec<LogicalType> {
        vec![A::logical_type(), B::logical_type()]
    }
}