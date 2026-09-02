use crate::value_types::duck_value_type::{ DuckValueReader, DuckValueType};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::LogicalType;

pub trait DuckColumns: Sized {
    fn create_arg_readers(chunk: &DataChunk) -> Vec<DuckValueReader>;

    fn read_args(readers: &[DuckValueReader], row: usize) -> Option<Self>;

    fn arg_types() -> Vec<LogicalType>{
        Self::named_column_types().into_iter().map(|(_, t)| t).collect()
    }

    fn named_column_types() -> Vec<(String, LogicalType)>{
        todo!()
    }

    fn write_columns_batch(chunk: &DataChunk, row: &Vec<Option<&Self>>){
        todo!()
    }
}

impl<A: DuckValueType> DuckColumns for (Option<A>,) {
    fn create_arg_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        vec![A::create_reader(chunk, 0)]
    }
    

    fn read_args(readers: &[DuckValueReader], row: usize) -> Option<Self> {
        Some((A::read(&readers[0], row),))
    }

    fn arg_types() -> Vec<LogicalType> {
        vec![A::logical_type()]
    }

    fn named_column_types() -> Vec<(String, LogicalType)> {
        Vec::from( [("arg0".to_string(), A::logical_type())])
    }

    fn write_columns_batch(chunk: &DataChunk, row: &Vec<Option<&Self>>) {
        todo!()
    }
}

impl<A: DuckValueType, B: DuckValueType> DuckColumns for (Option<A>, B) {

    fn create_arg_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        vec![A::create_reader(chunk, 0), B::create_reader(chunk, 1)]
    }
    fn read_args(readers: &[DuckValueReader], row: usize) -> Option<Self> {
        Some((A::read(&readers[0], row), B::read(&readers[1], row)?))
    }

    // fn arg_types() -> Vec<LogicalType> {
    //     vec![A::logical_type(), B::logical_type()]
    // }
    fn named_column_types() -> Vec<(String, LogicalType)>{
        Vec::from([
            ("arg0".to_string(), A::logical_type()),
            ("arg1".to_string(), B::logical_type())
        ])
    }

    fn write_columns_batch(chunk: &DataChunk, row: &Vec<Option<&Self>>) {
        A::write_batch(unsafe{ chunk.vector(0) }, &row.iter()
            .map(|r| r.and_then(|r| r.0.as_ref()))
            .collect::<Vec<_>>());
        B::write_batch(unsafe{ chunk.vector(1) }, &row.iter()
            .map(|r| r.map(|r| &r.1))
            .collect::<Vec<_>>());
    }
}