use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ListVector, LogicalType, TypeId, Value};
use crate::{duck_error, DuckResult};

// TypeId::List
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DuckList<T: DuckValueType> {
    pub value: Vec<Option<T>>,
}


impl<T: DuckValueType> DuckValueType for DuckList<T> {
    fn type_id() -> TypeId {
        TypeId::List
    }

    fn logical_type() -> LogicalType {
        LogicalType::list_from_logical(&T::logical_type())
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let child_vector = unsafe { ListVector::get_child(vector) };
        let child_size = unsafe { ListVector::get_size(vector) };
        let child_reader = T::create_reader_from_vector(child_vector, child_size);

        reader.child_reader = vec![child_reader];
        reader
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        let list_vec = reader.c_duckdb_vector;
        let entry = unsafe { ListVector::get_entry(list_vec, row) };

        // 之前是照着官方的写法写在这里的
        let child_reader = &reader.child_reader[0];
        let mut vec: Vec<Option<T>> = Vec::with_capacity(entry.length as usize);
        for i in 0..entry.length as usize {
            let idx = entry.offset as usize + i;
            vec.push(T::read(&child_reader, idx));
        }
        Some(DuckList { value: vec })
    }

    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //
    //     let child_vector = unsafe { ListVector::get_child(output) };
    //
    //     let child_writer = T::create_writer(child_vector);
    //
    //     writer.child_writer.push(child_writer);
    //
    //     writer
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec.iter()
            .filter_map(|x| x.as_ref().map(|v| v.value.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| {
                list.value.iter().map(|x| x.as_ref())
            })
            .collect();

        let child_writer = T::create_writer_batch(child_vector,&vec);

        writer.child_writer.push(child_writer);

        writer
    }
    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //     writer.list_builder = Some(unsafe{ ListBuilder::new(output) });
    //     writer
    // }

    // 尝试改为官方推荐的ListBuilder，失败，不支持递归嵌套
    // fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
    //     if let Some(builder) = &mut writer.list_builder {
    //         unsafe {
    //             builder.push_row(idx, vo.value.len(), move|writer, base| {
    //                 let mut child_writer = T::create_writer(writer.as_raw());
    //                 for (i, val) in vo.value.iter().enumerate() {
    //                     T::write(&mut child_writer, base + i, val);
    //                 }
    //                 T::write_finish(&mut child_writer);
    //             });
    //         }
    //     }
    // }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.value.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.value.iter().enumerate() {
            T::write(child_writer, offset + i, value.as_ref());
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(&mut writer.child_writer[0]);

        unsafe {
            ListVector::set_size(writer.c_duckdb_vector, writer.offset);
        }
    }
    // fn write_finish(writer: &mut DuckValueWriter) {
    //     unsafe {
    //         if let Some(builder) = writer.list_builder.take() {
    //            unsafe  { builder.finish(); }
    //         }
    //     }
    // }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        let vec = value.list_items()
            .iter()
            .map(T::read_by_duck_value)
            .collect::<DuckResult<Vec<_>>>()?;

        Ok(DuckList { value: vec })
    }
}

impl<T: DuckValueType> DuckValueType for Vec<Option<T>> {
    fn type_id() -> TypeId {
        DuckList::<T>::type_id()
    }

    fn logical_type() -> LogicalType {
        DuckList::<T>::logical_type()
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckList::<T>::create_reader_from_vector(vector, size)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        DuckList::<T>::read_valid(reader, row).map(|li| li.value)
    }

    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     DuckList::<T>::create_writer(output)
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec.iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| {
                list.iter().map(|x| x.as_ref())
            })
            .collect();

        let child_writer = T::create_writer_batch(child_vector,&vec);

        writer.child_writer.push(child_writer);

        writer
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.iter().enumerate() {
            T::write(child_writer, offset + i, value.as_ref());
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        DuckList::<T>::write_finish(writer)
    }


    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        DuckList::<T>::read_by_duck_value_valid(value).map(|li| li.value)
    }
}

impl<T: DuckValueType> DuckValueType for Vec<T> {
    fn type_id() -> TypeId {
        DuckList::<T>::type_id()
    }

    fn logical_type() -> LogicalType {
        DuckList::<T>::logical_type()
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckList::<T>::create_reader_from_vector(vector, size)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        // Option<Vec<Option<T>>> -> Option<Vec<T>>
        DuckList::<T>::read_valid(reader, row)
            .map(|li| {li.value})
            .and_then(|v| v.into_iter().collect::<Option<Vec<_>>>())
    }

    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     DuckList::<T>::create_writer(output)
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec.iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| {
                list.iter().map(|x| Some(x))
            })
            .collect();

        let child_writer = T::create_writer_batch(child_vector,&vec);

        writer.child_writer.push(child_writer);

        writer
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.iter().enumerate() {
            T::write_valid(child_writer, offset + i, value);
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        DuckList::<T>::write_finish(writer)
    }


    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        let values = value.list_items();
        let mut vec:Vec<T> = Vec::with_capacity(values.len());
        for x in values.iter() {
            let t = T::read_by_duck_value(x)?
                .ok_or_else(|| duck_error("Vec<T> value is None"))?;
            vec.push(t)
        }
        Ok(vec)
    }
}