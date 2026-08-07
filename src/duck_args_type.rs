use crate::duck_value_type_convertor::DuckValueType;
use quack_rs::prelude::{TypeId, VectorReader};

pub trait DuckArgs : Sized{
    const COUNT: usize;

    fn read(readers: &[VectorReader], row: usize) -> Self;

    fn params() -> Vec<TypeId>;
}

impl<A: DuckValueType> DuckArgs for (Option<A>,) {
    const COUNT: usize = 1;

    fn read(readers: &[VectorReader], row: usize) -> Self {
        (A::read(&readers[0], row),)
    }

    fn params() -> Vec<TypeId> {
        vec![A::type_id()]
    }
}

impl<A: DuckValueType, B: DuckValueType> DuckArgs for (Option<A>, Option<B>) {
    const COUNT: usize = 2;

    fn read(readers: &[VectorReader], row: usize) -> Self {
        (A::read(&readers[0], row), B::read(&readers[1], row))
    }

    fn params() -> Vec<TypeId> {
        vec![A::type_id(), B::type_id()]
    }
}