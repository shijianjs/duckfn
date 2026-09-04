use crate::{DuckResult, DuckValueReader, DuckValueType, DuckValueWriter, duck_error};
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ArrayVector, LogicalType, TypeId, Value};

pub type DuckOptionArray<T, const N: usize> = [Option<T>; N];
pub type DuckArray<T, const N: usize> = [T; N];

impl<T: DuckValueType, const N: usize> DuckValueType for DuckOptionArray<T, N> {
    fn type_id() -> TypeId {
        TypeId::Array
    }

    fn logical_type() -> LogicalType {
        LogicalType::array_from_logical(&T::logical_type(), N as u64)
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let child_vector = unsafe { ArrayVector::get_child(vector) };
        // let child_size = unsafe { ListVector::get_size(vector) };
        let child_reader = T::create_reader_from_vector(child_vector, N);

        reader.child_reader = vec![child_reader];
        reader
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        // 之前是照着官方的写法写在这里的
        let child_reader = &reader.child_reader[0];
        let mut vec: Vec<Option<T>> = Vec::with_capacity(N as usize);
        for i in 0..N {
            let idx = row * N + i;
            vec.push(T::read(&child_reader, idx));
        }
        vec.try_into().ok()
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let child_vector = unsafe { ArrayVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(|x| x.as_ref()))
            .collect();

        let child_writer = T::create_writer_batch(child_vector, &vec);

        writer.child_writer.push(child_writer);

        writer
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let child_writer = &mut writer.child_writer[0];
        let offset = idx*N;
        for (i, value) in v.iter().enumerate() {
            T::write(child_writer, offset + i, value.as_ref());
        }
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(&mut writer.child_writer[0]);
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Err(duck_error("Bind value to array type is not supported"))
    }
}

trait  Helper{
type H;
}
impl<T: DuckValueType, const N: usize> Helper for DuckArray<T, N> {
    type H = DuckOptionArray<T,N>;
}

impl<T: DuckValueType, const N: usize> DuckValueType for DuckArray<T, N> {
    fn type_id() -> TypeId {
        <Self as Helper>::H::type_id()
    }

    fn logical_type() -> LogicalType {
        // LogicalType::array_from_logical(&T::logical_type(), N as u64)
        <Self as Helper>::H::logical_type()
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        // let mut reader = DuckValueReader::new_from_vector(vector, size);
        // let child_vector = unsafe { ArrayVector::get_child(vector) };
        // // let child_size = unsafe { ListVector::get_size(vector) };
        // let child_reader = T::create_reader_from_vector(child_vector, N);
        //
        // reader.child_reader = vec![child_reader];
        // reader
        <Self as Helper>::H::create_reader_from_vector(vector, size)

    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        // 之前是照着官方的写法写在这里的
        let child_reader = &reader.child_reader[0];
        let mut vec: Vec<T> = Vec::with_capacity(N as usize);
        for i in 0..N {
            let idx = row * N + i;
            vec.push(T::read(&child_reader, idx)?);
        }
        vec.try_into().ok()
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let child_vector = unsafe { ArrayVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(|x| Some(x)))
            .collect();

        let child_writer = T::create_writer_batch(child_vector, &vec);

        writer.child_writer.push(child_writer);

        writer
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let child_writer = &mut writer.child_writer[0];
        let offset = idx*N;
        for (i, value) in v.iter().enumerate() {
            T::write_valid(child_writer, offset + i, value);
        }
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        <Self as Helper>::H::write_finish(writer)
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Err(duck_error("Bind value to array type is not supported"))
    }
}
