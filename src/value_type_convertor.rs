use quack_rs::prelude::{TypeId, VectorReader, VectorWriter};

pub trait DuckValueType: Sized {
    fn type_id() -> TypeId;
    fn read(reader: &VectorReader, row: usize) -> Option<Self> {
        if unsafe { reader.is_valid(row) } {
            Some(Self::read_valid(reader, row))
        } else {
            None
        }
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self;

    fn write(writer: &mut VectorWriter, row: usize, vo: Option<Self>) {
        match vo {
            None => unsafe { writer.set_null(row) },
            Some(v) => Self::write_valid(writer, row, v),
        }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self);
}

impl DuckValueType for i64 {
    fn type_id() -> TypeId {
        TypeId::BigInt
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i64(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i64(row, v) }
    }
}

impl DuckValueType for String {
    fn type_id() -> TypeId {
        TypeId::Varchar
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_str(row).to_string() }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_str(row, v.as_str()) }
    }
}