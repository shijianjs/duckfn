//! `Vec<T>`（别名 `DuckList` / `DuckOptionList`）与 DuckDB `LIST` 的映射。
//!
//! Mappings between `Vec<T>` (aliased as `DuckList` / `DuckOptionList`) and DuckDB `LIST`.

use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use crate::{DuckResult, duck_error};
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ListVector, LogicalType, TypeId, Value};

/// 元素可为 NULL 的列表：`LIST(T)`。
///
/// 只是 `Vec<Option<T>>` 的别名，没有专用包装类型；存在的意义是让命名与
/// [`DuckOptionArray`](crate::DuckOptionArray) 对称。
///
/// A list whose elements may be NULL: `LIST(T)`. Merely an alias for `Vec<Option<T>>` — there is
/// no dedicated wrapper type; it exists so that the naming mirrors
/// [`DuckOptionArray`](crate::DuckOptionArray).
pub type DuckOptionList<T> = Vec<Option<T>>;
/// 元素不可为 NULL 的列表：`LIST(T)`。
///
/// 只是 `Vec<T>` 的别名，没有专用包装类型；存在的意义是让命名与
/// [`DuckArray`](crate::DuckArray) 对称。
///
/// A list whose elements must not be NULL: `LIST(T)`. Merely an alias for `Vec<T>` — there is no
/// dedicated wrapper type; it exists so that the naming mirrors [`DuckArray`](crate::DuckArray).
pub type DuckList<T> = Vec<T>;

/// `Vec<T>` ↔ `LIST(T)`，元素的可空性由元素类型 `T` 自己决定。
///
/// 读写都围绕 `ListVector` 的「子向量 + entry(offset, length)」结构进行：
/// 读取时按 entry 逐元素读子向量；写入时先 `reserve` 出总元素数，再按 offset 追加。
///
/// 元素为 NULL 时按 [`DuckValueType::from_null`] 处理，于是两种既有语义都由这一份实现给出：
///
/// - `T` 不可空（如 `i32`）：该元素读不出来 → 整行变成 SQL NULL；
/// - `T = Option<U>`：元素取到 `None` → 该元素保持 SQL NULL，其余元素照常。
///
/// `Vec<T>` ↔ `LIST(T)`, where element nullability is up to the element type `T` itself. Both
/// directions work around the `ListVector` "child vector + entry(offset, length)" layout: reading
/// walks each entry's slice of the child vector, while writing reserves the total element count
/// first and then appends at the running offset.
///
/// A NULL element is resolved through [`DuckValueType::from_null`], so both historical semantics
/// come out of this single implementation: a non-nullable `T` (e.g. `i32`) makes the whole row
/// NULL, while `T = Option<U>` keeps the element NULL and leaves the other elements alone.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<T: DuckValueType> DuckValueType for Vec<T> {
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

        let child_reader = &reader.child_reader[0];
        let mut vec: Vec<T> = Vec::with_capacity(entry.length as usize);
        for i in 0..entry.length as usize {
            let idx = entry.offset as usize + i;
            // NULL 元素：元素类型能表示 NULL 就取它的空值，否则整行变 NULL
            vec.push(T::read_slot(child_reader, idx)?);
        }
        Some(vec)
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec
            .iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        // 元素值本身可为 `None`（元素类型是 `Option<U>`），但它仍占子向量里的一个槽位，
        // 因此这里按下标全部保留，NULL 与否交给 `write_valid` 处理。
        //
        // An element may itself be `None` (when the element type is `Option<U>`); it still owns a
        // child slot, so every element is kept here and NULL-ness is decided in `write_valid`.
        let child_values: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(Some))
            .collect();

        let child_writer = T::create_writer_batch(child_vector, &child_values);

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
            // 走 null-aware 的 `write`：元素类型是 `Option<U>` 且值为 `None` 时写 NULL
            T::write(child_writer, offset + i, Some(value));
        }
        writer.offset += len;
    }

    /// NULL 行：父向量置 NULL 之外，把 entry 显式写成空区间 `(0, 0)`。
    ///
    /// NULL 行的 `list_entry_t` 本来不会被写，留着未初始化的 offset/length。
    /// 实测 DuckDB 的 list 算子都会先查父向量 validity（子向量长度也被
    /// `write_finish` 的 `set_size` 收窄到已写范围），所以不写 entry 也安全；
    /// 这里补上是为了让 duckfn 不依赖这一前提 —— ARRAY 的教训正是
    /// 「不要假设对方一定先查父 validity」。
    ///
    /// NULL rows: besides marking the parent NULL, write an explicit empty entry
    /// `(0, 0)`. DuckDB's list operators do check the parent validity (and the child
    /// length is clamped by `set_size`), so this is defensive rather than a fix.
    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        unsafe {
            writer.vector_writer.set_null(idx);
            ListVector::set_entry(writer.c_duckdb_vector, idx, 0, 0);
        }
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(&mut writer.child_writer[0]);

        unsafe {
            ListVector::set_size(writer.c_duckdb_vector, writer.offset);
        }
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        let values = value.list_items();
        let mut vec: Vec<T> = Vec::with_capacity(values.len());
        for x in values.iter() {
            vec.push(
                T::read_slot_by_duck_value(x)?
                    .ok_or_else(|| duck_error("Vec<T> value is None"))?,
            );
        }
        Ok(vec)
    }
}
