//! 定长数组 `[T; N]`（别名 `DuckArray` / `DuckOptionArray`）与 DuckDB `ARRAY` 的映射。
//!
//! Mappings between fixed-size arrays `[T; N]` (aliased as `DuckArray` / `DuckOptionArray`) and
//! DuckDB `ARRAY`.

use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use crate::{DuckResult, duck_error};
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ArrayVector, LogicalType, TypeId, Value};

/// 元素可为 NULL 的定长数组：`ARRAY(T, N)`。
///
/// A fixed-size array whose elements may be NULL: `ARRAY(T, N)`.
pub type DuckOptionArray<T, const N: usize> = [Option<T>; N];
/// 元素不可为 NULL 的定长数组：`ARRAY(T, N)`。
///
/// A fixed-size array whose elements must not be NULL: `ARRAY(T, N)`.
pub type DuckArray<T, const N: usize> = [T; N];

/// `[T; N]` ↔ `ARRAY(T, N)`，元素的可空性由元素类型 `T` 自己决定。
///
/// DuckDB 的 ARRAY 与 LIST 物理布局类似（子向量 + 定长 entry），但每个 entry 长度固定为
/// `N`，因此第 `row` 行的元素在子向量中的下标是 `row * N + i`。
/// 元素为 NULL 时按 [`DuckValueType::from_null`] 处理：`T` 不可空则整行变 NULL，
/// `T = Option<U>` 则只有该元素为 NULL。
///
/// `[T; N]` ↔ `ARRAY(T, N)`, where element nullability is up to `T` itself. DuckDB's ARRAY shares
/// LIST's physical layout (a child vector plus entries) but every entry has exactly `N` elements,
/// so the element of row `row` sits at child index `row * N + i`. A NULL element is resolved
/// through [`DuckValueType::from_null`]: a non-nullable `T` turns the whole row NULL, while
/// `T = Option<U>` leaves just that element NULL.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<T: DuckValueType, const N: usize> DuckValueType for [T; N] {
    fn type_id() -> TypeId {
        TypeId::Array
    }

    fn logical_type() -> LogicalType {
        LogicalType::array_from_logical(&T::logical_type(), N as u64)
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let child_vector = unsafe { ArrayVector::get_child(vector) };
        let child_reader = T::create_reader_from_vector(child_vector, N);

        reader.child_reader = vec![child_reader];
        reader
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        let child_reader = &reader.child_reader[0];
        let mut vec: Vec<T> = Vec::with_capacity(N);
        for i in 0..N {
            let idx = row * N + i;
            // NULL 元素：元素类型能表示 NULL 就取它的空值，否则整行变 NULL
            vec.push(T::read_slot(child_reader, idx)?);
        }
        vec.try_into().ok()
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let child_vector = unsafe { ArrayVector::get_child(vector) };

        // 元素本身可能是 `None`（元素类型为 `Option<U>`），但它仍占子向量里的一个槽位。
        //
        // An element may itself be `None` (element type `Option<U>`) but it still owns a child
        // slot.
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
        let child_writer = &mut writer.child_writer[0];
        let offset = idx * N;
        for (i, value) in v.iter().enumerate() {
            // 走 null-aware 的 `write`：元素类型是 `Option<U>` 且值为 `None` 时写 NULL
            T::write(child_writer, offset + i, Some(value));
        }
    }

    /// NULL 行：父向量置 NULL 之外，还要把子向量的 `N` 个槽一并置 NULL。
    ///
    /// ARRAY 的子向量按 `row * N` **稠密**索引，NULL 行同样占着这 `N` 个槽。
    /// 如果只置父向量的 validity，这些槽就是未初始化内存；DuckDB 的部分操作
    /// （典型的是 `ARRAY(BLOB)` / `ARRAY(VARCHAR)` 的 `CAST(... AS VARCHAR)`）
    /// 会整段转换子向量而不先看父向量 validity，于是把未初始化的 `string_t`
    /// 当成指针解引用 → 偶发/必现的访问违例。
    ///
    /// NULL rows: besides marking the parent NULL, also mark the `N` child slots
    /// NULL. ARRAY's child vector is densely indexed by `row * N`, so a NULL row still
    /// owns those `N` slots; leaving them uninitialised makes DuckDB treat garbage
    /// `string_t`s as pointers when it casts the whole child vector (e.g.
    /// `ARRAY(BLOB)` -> `VARCHAR`).
    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        unsafe { writer.vector_writer.set_null(idx) };
        let child_writer = &mut writer.child_writer[0];
        let offset = idx * N;
        for i in 0..N {
            // 走 T::write_null 而不是直接 set_null：嵌套 ARRAY / STRUCT 会继续
            // 递归到更深一层，保证整棵子向量树里没有未初始化槽。
            T::write_null(child_writer, offset + i);
        }
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(&mut writer.child_writer[0]);
    }

    /// ARRAY 的定长语义与表函数 bind 参数的自描述模型不匹配，因此不支持从 [`Value`] 读取。
    ///
    /// ARRAY's fixed-length semantics do not fit the self-describing bind-argument model of
    /// table functions, so reading from a [`Value`] is not supported.
    fn read_by_duck_value_valid(_value: &Value) -> DuckResult<Self> {
        Err(duck_error("Bind value to array type is not supported"))
    }
}
