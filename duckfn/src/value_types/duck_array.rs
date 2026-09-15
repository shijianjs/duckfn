//! 定长数组 `[T; N]` / `[Option<T>; N]` 与 DuckDB `ARRAY` 的映射。
//!
//! Mappings between fixed-size arrays `[T; N]` / `[Option<T>; N]` and DuckDB `ARRAY`.

use crate::{DuckResult, DuckValueReader, DuckValueType, DuckValueWriter, duck_error};
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

/// `[Option<T>; N]` ↔ `ARRAY(T, N)`。
///
/// DuckDB 的 ARRAY 与 LIST 物理布局类似（子向量 + 定长 entry），但每个 entry 长度固定为
/// `N`，因此第 `row` 行的元素在子向量中的下标是 `row * N + i`。
///
/// `[Option<T>; N]` ↔ `ARRAY(T, N)`. DuckDB's ARRAY shares LIST's physical layout (a child
/// vector plus entries) but every entry has exactly `N` elements, so the element of row `row`
/// sits at child index `row * N + i`.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
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
        let mut vec: Vec<Option<T>> = Vec::with_capacity(N);
        for i in 0..N {
            let idx = row * N + i;
            vec.push(T::read(child_reader, idx));
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

// 用于把 `DuckArray<T, N>` 复用 `DuckOptionArray<T, N>` 的实现。
//
// Helper that lets `DuckArray<T, N>` reuse the `DuckOptionArray<T, N>` implementation.
trait  Helper{
type H;
}
impl<T: DuckValueType, const N: usize> Helper for DuckArray<T, N> {
    type H = DuckOptionArray<T,N>;
}

/// `[T; N]` ↔ `ARRAY(T, N)`：元素不允许为 SQL NULL。
///
/// 读写复用 `[Option<T>; N]` 的实现，读时任一元素为 NULL 则整体视为 NULL。
///
/// `[T; N]` ↔ `ARRAY(T, N)` where elements must not be NULL. Both directions reuse the
/// `[Option<T>; N]` implementation; reading yields NULL for the whole row if any element is
/// NULL.
// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
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
        let mut vec: Vec<T> = Vec::with_capacity(N);
        for i in 0..N {
            let idx = row * N + i;
            vec.push(T::read(child_reader, idx)?);
        }
        vec.try_into().ok()
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let child_vector = unsafe { ArrayVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| list.iter().map(Some))
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

    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        <Self as Helper>::H::write_null(writer, idx)
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        <Self as Helper>::H::write_finish(writer)
    }

    /// 同 `[Option<T>; N]`：ARRAY 不支持从 [`Value`] 读取。
    ///
    /// Same as `[Option<T>; N]`: ARRAY does not support reading from a [`Value`].
    fn read_by_duck_value_valid(_value: &Value) -> DuckResult<Self> {
        Err(duck_error("Bind value to array type is not supported"))
    }
}
