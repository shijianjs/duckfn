//! `IndexMap<K, V>`（别名 `DuckMap`）与 DuckDB `MAP` 的映射（保留键的插入顺序）。
//!
//! Mappings between `IndexMap<K, V>` (aliased as `DuckMap`) and DuckDB `MAP` (preserving key
//! insertion order).

use crate::value_types::vector_layout::{
    finish_elements, map_keys, map_values, reserve_elements, set_entry, write_null_row,
};
use crate::{DuckResult, DuckValueReader, DuckValueType, DuckValueWriter, duck_error};
use indexmap::IndexMap;
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{LogicalType, MapVector, TypeId, Value};
use std::hash::Hash;

/// `IndexMap<K, V>` 的别名，与 [`DuckList`](crate::DuckList) / [`DuckArray`](crate::DuckArray)
/// 命名一致。
///
/// 值是否可空由值类型自己决定：写 `IndexMap<K, Option<V>>` 就是值可空（键永远不可为空），
/// 没有单独的别名。
///
/// An alias for `IndexMap<K, V>`, named to match [`DuckList`](crate::DuckList) /
/// [`DuckArray`](crate::DuckArray). Value nullability is up to the value type — write
/// `IndexMap<K, Option<V>>` for nullable values (keys are never NULL); there is no separate alias.
pub type DuckMap<K, V> = IndexMap<K, V>;

/// `IndexMap<K, V>` ↔ `MAP(K, V)`，值是否可为 NULL 由值类型 `V` 自己决定（键永远不可为空）。
///
/// DuckDB 的 MAP 物理上是 `LIST(STRUCT(key, value))`：外层 entry 决定每行的键值对数量，
/// 键/值分别存放在两个子向量里（用 [`MapVector::keys`] / [`MapVector::values`] 取出）。
/// 用 `IndexMap` 是因为 DuckDB 的 map 语义要求键唯一且保持插入顺序。
///
/// 值为 NULL 时按 [`DuckValueType::from_null`] 处理：`V` 不可空则整行变 NULL，
/// `V = Option<U>` 则该值保持 NULL。键则始终走 `?`：读到 NULL 键即整行变 NULL。
///
/// `IndexMap<K, V>` ↔ `MAP(K, V)`, where value nullability is up to `V` (keys are never NULL).
/// DuckDB's MAP is physically `LIST(STRUCT(key, value))`: the outer entry gives the number of
/// pairs in each row, and keys/values live in two child vectors (obtained via
/// [`MapVector::keys`] / [`MapVector::values`]). `IndexMap` is used because DuckDB's map
/// semantics require unique keys with a stable insertion order.
///
/// A NULL value is resolved through [`DuckValueType::from_null`], so a non-nullable `V` turns the
/// whole row NULL while `V = Option<U>` merely keeps that value NULL. Keys always use `?`: a NULL
/// key turns the whole row NULL.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<K: DuckValueType + Hash + Eq, V: DuckValueType> DuckValueType for IndexMap<K, V> {
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
            // 键永远不可为空：读不出来（NULL）就整行变 NULL
            let k = K::read(&reader.child_reader[0], idx)?;
            // 值为 NULL：值类型能表示 NULL 就取它的空值，否则整行变 NULL
            let v = V::read_slot(&reader.child_reader[1], idx)?;
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
        reserve_elements(vector, total_elements);
        let k_vector = map_keys(vector);

        // 值本身可能是 `None`（值类型为 `Option<U>`），但它仍占一对键值的位置，
        // 因此这里按下标全部保留，NULL 与否交给 `write_valid` 处理。
        //
        // A value may itself be `None` (value type `Option<U>`) but it still owns a key/value
        // pair, so every entry is kept here and NULL-ness is decided in `write_valid`.
        let k_vec: Vec<Option<&K>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|map| map.iter().map(|(k, _v)| Some(k)))
            .collect();
        let k_writer = K::create_writer_batch(k_vector, &k_vec);

        let v_vector = map_values(vector);
        let v_vec: Vec<Option<&V>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|map| map.iter().map(|(_k, v)| Some(v)))
            .collect();
        let v_writer = V::create_writer_batch(v_vector, &v_vec);

        writer.child_writer = vec![k_writer, v_writer];
        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        set_entry(writer.c_duckdb_vector, idx, offset, len);

        for (i, (k, v)) in v.iter().enumerate() {
            let k_writer = &mut writer.child_writer[0];
            let idx = offset + i;
            K::write_valid(k_writer, idx, k);
            let v_writer = &mut writer.child_writer[1];
            // 走 null-aware 的 `write`：值类型是 `Option<U>` 且值为 `None` 时写 NULL
            V::write(v_writer, idx, Some(v));
        }
        writer.offset += len;
    }

    /// NULL 行：父向量置 NULL 之外，把 entry 显式写成空区间 `(0, 0)`。
    ///
    /// MAP 物理上是 `LIST(STRUCT(key, value))`，与 LIST 走同一套约定；
    /// 具体理由见 `vector_layout::write_null_row`。
    ///
    /// NULL rows: besides marking the parent NULL, write an explicit empty entry `(0, 0)`. MAP is
    /// physically `LIST(STRUCT(key, value))` and follows the same convention as LIST; see
    /// `vector_layout::write_null_row` for the rationale.
    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        write_null_row(writer, idx);
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        K::write_finish(&mut writer.child_writer[0]);
        V::write_finish(&mut writer.child_writer[1]);

        finish_elements(writer.c_duckdb_vector, writer.offset);
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        let map_size = value.map_len();
        let mut map = Self::with_capacity(map_size);
        for i in 0..map_size {
            let k = match value.map_key(i) {
                Some(k_value) => K::read_by_duck_value(&k_value)?,
                None => None,
            };
            let k = k.ok_or_else(|| duck_error("Map key cannot be null"))?;

            let v = match value.map_value(i) {
                Some(v_value) => V::read_slot_by_duck_value(&v_value)?,
                None => None,
            };
            let v = v.ok_or_else(|| duck_error("Map value cannot be null"))?;

            map.insert(k, v);
        }
        Ok(map)
    }
}
