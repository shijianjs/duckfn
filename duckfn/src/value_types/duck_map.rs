use crate::{DuckResult, DuckValueReader, DuckValueType, DuckValueWriter, duck_error};
use indexmap::IndexMap;
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ListVector, LogicalType, MapVector, TypeId, Value};
use std::hash::Hash;

// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<K: DuckValueType + Hash + Eq, V: DuckValueType> DuckValueType for IndexMap<K, Option<V>> {
    fn type_id() -> TypeId {
        TypeId::Map
    }
    fn logical_type() -> LogicalType {
        LogicalType::map_from_logical(&K::logical_type(), &V::logical_type())
    }

    fn create_reader_from_vector(map_vec: duckdb_vector, size: usize) -> DuckValueReader {
        let child_size = unsafe { MapVector::total_entry_count(map_vec) };
        let key_reader =
            K::create_reader_from_vector(unsafe { MapVector::keys(map_vec) }, child_size);
        let value_reader =
            V::create_reader_from_vector(unsafe { MapVector::values(map_vec) }, child_size);

        let mut reader = DuckValueReader::new_from_vector(map_vec, size);
        reader.child_reader = vec![key_reader, value_reader];
        reader
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        let map_vec = reader.c_duckdb_vector;
        let entry = unsafe { MapVector::get_entry(map_vec, row) };
        let map_size = entry.length;
        let mut map = Self::with_capacity(map_size as usize);
        for i in 0..map_size as usize {
            let idx = entry.offset as usize + i;
            let k = K::read(&reader.child_reader[0], idx)?;
            let v = V::read(&reader.child_reader[1], idx);
            map.insert(k, v);
        }
        Some(map)
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec
            .iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let k_vector = unsafe { MapVector::keys(vector) };

        let k_vec: Vec<Option<&K>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(|(k, _v)| Some(k)))
            .collect();
        let k_writer = K::create_writer_batch(k_vector, &k_vec);

        let v_vector = unsafe { MapVector::values(vector) };
        let v_vec: Vec<Option<&V>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(|(_k, v)| v.as_ref()))
            .collect();
        let v_writer = V::create_writer_batch(v_vector, &v_vec);

        writer.child_writer = vec![k_writer, v_writer];
        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        for (i, (k, v)) in v.iter().enumerate() {
            let k_writer = &mut writer.child_writer[0];
            let idx = offset + i;
            K::write_valid(k_writer, idx, k);
            let v_writer = &mut writer.child_writer[1];
            V::write(v_writer, idx, v.as_ref());
        }
        writer.offset += len;
    }
    fn write_finish(writer: &mut DuckValueWriter) {
        K::write_finish(&mut writer.child_writer[0]);
        V::write_finish(&mut writer.child_writer[1]);

        unsafe {
            MapVector::set_size(writer.c_duckdb_vector, writer.offset);
        }
    }
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        let map_size = value.map_len();
        let mut map = Self::with_capacity(map_size);
        for i in 0..map_size {
            let k_value_option = value.map_key(i);
            let k = if let Some(k_value) = k_value_option {
                K::read_by_duck_value(&k_value)?
            } else {
                None
            };
            let k_valid = k.ok_or_else(|| duck_error("Map key cannot be null"))?;
            let v_value_option = value.map_value(i);
            let v = if let Some(v_value) = v_value_option {
                V::read_by_duck_value(&v_value)?
            } else {
                None
            };
            map.insert(k_valid, v);
        }
        Ok(map)
    }
}

trait Helper {
    type H;
}

impl<K: DuckValueType + Hash + Eq, V: DuckValueType> Helper for IndexMap<K, V> {
    type H = IndexMap<K, Option<V>>;
}
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<K: DuckValueType + Hash + Eq, V: DuckValueType> DuckValueType for IndexMap<K, V> {
    fn type_id() -> TypeId {
        <Self as Helper>::H::type_id()
    }
    fn logical_type() -> LogicalType {
        // LogicalType::map_from_logical(&K::logical_type(), &V::logical_type())
        <Self as Helper>::H::logical_type()
    }

    fn create_reader_from_vector(map_vec: duckdb_vector, size: usize) -> DuckValueReader {
        // IndexMap::<K, Option<V>>::create_reader_from_vector(map_vec,size)
        <Self as Helper>::H::create_reader_from_vector(map_vec, size)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        let map_vec = reader.c_duckdb_vector;
        let entry = unsafe { MapVector::get_entry(map_vec, row) };
        let map_size = entry.length;
        let mut map = Self::with_capacity(map_size as usize);
        for i in 0..map_size as usize {
            let idx = entry.offset as usize + i;
            let k = K::read(&reader.child_reader[0], idx)?;
            let v = V::read(&reader.child_reader[1], idx)?;
            map.insert(k, v);
        }
        Some(map)
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec
            .iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let k_vector = unsafe { MapVector::keys(vector) };

        let k_vec: Vec<Option<&K>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(|(k, _v)| Some(k)))
            .collect();
        let k_writer = K::create_writer_batch(k_vector, &k_vec);

        let v_vector = unsafe { MapVector::values(vector) };
        let v_vec: Vec<Option<&V>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(|(_k, v)| Some(v)))
            .collect();
        let v_writer = V::create_writer_batch(v_vector, &v_vec);

        writer.child_writer = vec![k_writer, v_writer];
        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        for (i, (k, v)) in v.iter().enumerate() {
            let k_writer = &mut writer.child_writer[0];
            let idx = offset + i;
            K::write_valid(k_writer, idx, k);
            let v_writer = &mut writer.child_writer[1];
            V::write_valid(v_writer, idx, v);
        }
        writer.offset += len;
    }
    fn write_finish(writer: &mut DuckValueWriter) {
        <Self as Helper>::H::write_finish(writer)
    }
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        let map_size = value.map_len();
        let mut map = Self::with_capacity(map_size);
        for i in 0..map_size {
            let k_value_option = value.map_key(i);
            let k = if let Some(k_value) = k_value_option {
                K::read_by_duck_value(&k_value)?
            } else {
                None
            };
            let k_valid = k.ok_or_else(|| duck_error("Map key cannot be null"))?;
            let v_value_option = value.map_value(i);
            let v = if let Some(v_value) = v_value_option {
                V::read_by_duck_value(&v_value)?
            } else {
                None
            };
            let v_valid = v.ok_or_else(|| duck_error("Map value cannot be null"))?;
            map.insert(k_valid, v_valid);
        }
        Ok(map)
    }
}
