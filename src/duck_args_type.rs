use crate::duck_value_type_convertor::{DuckTypeInfo, DuckValueType};
use quack_rs::prelude::{TypeId, VectorReader};

pub trait DuckArgs : Sized{

    fn read(readers: &[VectorReader], row: usize) -> Self;

    fn params() -> Vec<DuckTypeInfo>;
}

impl<A: DuckValueType> DuckArgs for (Option<A>,) {

    fn read(readers: &[VectorReader], row: usize) -> Self {
        (A::read_by_vector_reader(&readers[0], row),)
    }

    fn params() -> Vec<DuckTypeInfo> {
        vec![A::type_info()]
    }
}

impl<A: DuckValueType, B: DuckValueType> DuckArgs for (Option<A>, Option<B>) {

    fn read(readers: &[VectorReader], row: usize) -> Self {
        (A::read_by_vector_reader(&readers[0], row), B::read_by_vector_reader(&readers[1], row))
    }

    fn params() -> Vec<DuckTypeInfo> {
        vec![A::type_info(), B::type_info()]
    }
}