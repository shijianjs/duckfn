//! `Vec<T>` / `Vec<Option<T>>` 与 DuckDB `LIST` 的映射。
//!
//! Mappings between `Vec<T>` / `Vec<Option<T>>` and DuckDB `LIST`.

use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ListVector, LogicalType, TypeId, Value};
use crate::{duck_error, DuckResult};

/// `Vec<Option<T>>` ↔ `LIST(T)`：元素可以是 SQL NULL。
///
/// 读写都围绕 `ListVector` 的「子向量 + entry(offset, length)」结构进行：
/// 读取时按 entry 逐元素读子向量；写入时先 `reserve` 出总元素数，再按 offset 追加。
///
/// `Vec<Option<T>>` ↔ `LIST(T)` where elements may be SQL NULL. Both directions work around
/// the `ListVector` "child vector + entry(offset, length)" layout: reading walks each entry's
/// slice of the child vector, while writing reserves the total element count first and then
/// appends at the running offset.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<T: DuckValueType> DuckValueType for Vec<Option<T>> {
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
            vec.push(T::read(child_reader, idx));
        }
        Some(vec)
    }

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
        T::write_finish(&mut writer.child_writer[0]);

        unsafe {
            ListVector::set_size(writer.c_duckdb_vector, writer.offset);
        }
    }


    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
         value.list_items()
            .iter()
            .map(T::read_by_duck_value)
            .collect::<DuckResult<Vec<_>>>()
    }
}

/// `Vec<T>` ↔ `LIST(T)`：元素不允许为 SQL NULL。
///
/// 读写完全复用 `Vec<Option<T>>` 的实现，只在转换时收紧类型：
/// 读出来若有 NULL 元素则整体视为 NULL，写出去时把元素包成 `Some`。
///
/// `Vec<T>` ↔ `LIST(T)` where elements must not be SQL NULL. Both directions reuse the
/// `Vec<Option<T>>` implementation and only tighten the type on conversion: reading yields
/// NULL for the whole row if any element is NULL, and writing wraps elements in `Some`.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<T: DuckValueType> DuckValueType for Vec<T> {
    fn type_id() -> TypeId {
        TypeId::List
    }

    fn logical_type() -> LogicalType {
        Vec::<Option<T>>::logical_type()
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        Vec::<Option<T>>::create_reader_from_vector(vector, size)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        // Option<Vec<Option<T>>> -> Option<Vec<T>>
        Vec::<Option<T>>::read_valid(reader, row)
            .and_then(|v| v.into_iter().collect::<Option<Vec<_>>>())
    }

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
                list.iter().map(Some)
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
        Vec::<Option<T>>::write_finish(writer)
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
