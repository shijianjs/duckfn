use crate::value_types::duck_value_type::DuckValueReader;
use quack_rs::prelude::DataChunk;
use quack_rs::prelude::LogicalType;

pub trait DuckColumns: Sized {
    fn create_column_readers(chunk: &DataChunk) -> Vec<DuckValueReader>;

    fn read_columns(readers: &[DuckValueReader], row: usize) -> Option<Self>;

    fn column_types() -> Vec<LogicalType>{
        Self::named_column_types().into_iter().map(|(_, t)| t).collect()
    }

    fn named_column_types() -> Vec<(String, LogicalType)>;

    fn write_columns_batch(_chunk: &DataChunk, _row: &Vec<Option<&Self>>){
        todo!()
    }
}

