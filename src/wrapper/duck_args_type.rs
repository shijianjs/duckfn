use crate::wrapper::duck_value_type_convertor::{DuckTypeInfo, DuckValueReader, DuckValueType};
use quack_rs::data_chunk::DataChunk;

pub trait DuckArgs: Sized {
    fn create_readers(chunk: &DataChunk) -> Vec<DuckValueReader>;

    fn read(readers: &[DuckValueReader], row: usize) -> Self;

    fn params() -> Vec<DuckTypeInfo>;
}

impl<A: DuckValueType> DuckArgs for (Option<A>,) {
    fn create_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        vec![A::create_reader(chunk, 0)]
    }
    

    fn read(readers: &[DuckValueReader], row: usize) -> Self {
        (A::read(&readers[0], row),)
    }

    fn params() -> Vec<DuckTypeInfo> {
        vec![A::type_info()]
    }
}

impl<A: DuckValueType, B: DuckValueType> DuckArgs for (Option<A>, Option<B>) {

    fn create_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        vec![A::create_reader(chunk, 0), B::create_reader(chunk, 1)]
    }
    fn read(readers: &[DuckValueReader], row: usize) -> Self {
        (A::read(&readers[0], row), B::read(&readers[1], row))
    }

    fn params() -> Vec<DuckTypeInfo> {
        vec![A::type_info(), B::type_info()]
    }
}