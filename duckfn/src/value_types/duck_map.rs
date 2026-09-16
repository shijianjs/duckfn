//! `IndexMap<K, V>`（别名 `DuckMap` / `DuckOptionMap`）与 DuckDB `MAP` 的映射（保留键的插入顺序）。
//!
//! Mappings between `IndexMap<K, V>` (aliased as `DuckMap` / `DuckOptionMap`) and DuckDB `MAP`
//! (preserving key insertion order).

use crate::{DuckResult, DuckValueReader, DuckValueType, DuckValueWriter, duck_error};
use indexmap::IndexMap;
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ListVector, LogicalType, MapVector, TypeId, Value};
use std::hash::Hash;

/// 值可为 NULL 的映射：`MAP(K, V)`（键永远不可为 NULL）。
///
/// 只是 `IndexMap<K, Option<V>>` 的别名，没有专用包装类型；存在的意义是让命名与
/// [`DuckOptionArray`](crate::DuckOptionArray) 对称。
///
/// A map whose values may be NULL: `MAP(K, V)` (keys are never NULL). Merely an alias for
/// `IndexMap<K, Option<V>>` — there is no dedicated wrapper type; it exists so that the naming
/// mirrors [`DuckOptionArray`](crate::DuckOptionArray).
pub type DuckOptionMap<K, V> = IndexMap<K, Option<V>>;
/// 键和值都不可为 NULL 的映射：`MAP(K, V)`。
///
/// 只是 `IndexMap<K, V>` 的别名，没有专用包装类型；存在的意义是让命名与
/// [`DuckArray`](crate::DuckArray) 对称。
///
/// A map whose keys and values must not be NULL: `MAP(K, V)`. Merely an alias for
/// `IndexMap<K, V>` — there is no dedicated wrapper type; it exists so that the naming mirrors
/// [`DuckArray`](crate::DuckArray).
pub type DuckMap<K, V> = IndexMap<K, V>;

/// `IndexMap<K, Option<V>>` ↔ `MAP(K, V)`：值可以为 SQL NULL（键不允许为 NULL）。
///
/// DuckDB 的 MAP 物理上是 `LIST(STRUCT(key, value))`：外层 entry 决定每行的键值对数量，
/// 键/值分别存放在两个子向量里（用 [`MapVector::keys`] / [`MapVector::values`] 取出）。
/// 用 `IndexMap` 是因为 DuckDB 的 map 语义要求键唯一且保持插入顺序。
///
/// `IndexMap<K, Option<V>>` ↔ `MAP(K, V)` where values may be SQL NULL (keys must not be).
/// DuckDB's MAP is physically `LIST(STRUCT(key, value))`: the outer entry gives the number of
/// pairs in each row, and keys/values live in two child vectors (obtained via
/// [`MapVector::keys`] / [`MapVector::values`]). `IndexMap` is used because DuckDB's map
/// semantics require unique keys with a stable insertion order.
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

    /// NULL 行：父向量置 NULL 之外，把 entry 显式写成空区间 `(0, 0)`。
    ///
    /// MAP 物理上是 `LIST(STRUCT(key, value))`，与 LIST 同理：entry 不写也安全
    /// （DuckDB 先查父 validity，子向量长度由 `set_size` 收窄），这里补上只是
    /// 让 duckfn 不依赖那个前提，与 LIST / ARRAY 的处理保持一致。
    ///
    /// NULL rows: besides marking the parent NULL, write an explicit empty entry
    /// `(0, 0)`. MAP is physically `LIST(STRUCT(key, value))`, so this mirrors LIST.
    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        unsafe {
            writer.vector_writer.set_null(idx);
            ListVector::set_entry(writer.c_duckdb_vector, idx, 0, 0);
        }
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

// 用于把 `IndexMap<K, V>` 复用 `IndexMap<K, Option<V>>` 的实现。
//
// Helper that lets `IndexMap<K, V>` reuse the `IndexMap<K, Option<V>>` implementation.
trait Helper {
    type H;
}

impl<K: DuckValueType + Hash + Eq, V: DuckValueType> Helper for IndexMap<K, V> {
    type H = IndexMap<K, Option<V>>;
}

/// `IndexMap<K, V>` ↔ `MAP(K, V)`：键和值都不允许为 SQL NULL。
///
/// 读写复用 `IndexMap<K, Option<V>>` 的实现：读时键或值为 NULL 会直接报错，
/// 写时把值包成 `Some`。
///
/// `IndexMap<K, V>` ↔ `MAP(K, V)` where neither keys nor values may be SQL NULL. Both
/// directions reuse the `IndexMap<K, Option<V>>` implementation: on read a NULL key or value
/// is an error, and on write values are wrapped in `Some`.
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

    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        <Self as Helper>::H::write_null(writer, idx)
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
