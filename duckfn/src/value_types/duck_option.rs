//! `Option<T>` 与 DuckDB 可空值的映射：可空性由值本身承载，逻辑类型与 `T` 相同。
//!
//! Mappings between `Option<T>` and nullable DuckDB values: nullability is carried by the value
//! itself and the logical type equals `T`'s.

use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use crate::DuckResult;
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{LogicalType, TypeId, Value};

/// `Option<T>` ↔ 与 `T` 相同的 DuckDB 类型，只是允许 SQL NULL。
///
/// DuckDB 的逻辑类型里没有「可空」这一维 —— NULL 由向量自带的 validity 表示，因此
/// `Option<T>` 的 [`type_id`](DuckValueType::type_id) / [`logical_type`](DuckValueType::logical_type)
/// 与 `T` 完全一致，读写也只是转发给 `T`，只在两处不同：
///
/// - [`from_null`](DuckValueType::from_null) 返回 `Some(None)`：容器与结构体字段遇到 NULL 槽位时，
///   这里取到的值就是 `None`，而不是「整个值作废」；
/// - [`write_valid`](DuckValueType::write_valid) 收到 `None` 时写 NULL（转发 `T::write_null`）。
///
/// 有了这个实现，「元素/值/字段是否可空」在类型层面统一表达：写 `T` 就是 NOT NULL，写
/// `Option<T>` 就是可空；`Vec<T>` / `[T; N]` / `IndexMap<K, V>` 只需一份实现。
///
/// 于是「多套一层」也是合法的：`Option<Option<T>>` 与 `Option<T>` 完全等价 —— 逻辑类型相同、
/// NULL 只会读成外层 `None`、`None` 与 `Some(None)` 都写 NULL。封装层不方便把中间类型剥出来时
/// 可以直接套一层，不必为「本来就是 `Option`」单独加判断（能力用例见
/// `test/sql/types/duck_opt_option_scalar_echo.test` 与 `duck_opt_option_table_echo.test`）。
///
/// `Option<T>` ↔ the same DuckDB type as `T`, just allowing SQL NULL. DuckDB's logical types have
/// no "nullable" dimension — NULL lives in the vector's validity mask — so `Option<T>` shares
/// `T`'s [`type_id`](DuckValueType::type_id) / [`logical_type`](DuckValueType::logical_type) and
/// merely forwards reads and writes to `T`. Only two things differ:
/// [`from_null`](DuckValueType::from_null) returns `Some(None)` (a NULL slot yields the value
/// `None` instead of invalidating the whole value) and
/// [`write_valid`](DuckValueType::write_valid) turns `None` into a NULL write (forwarding
/// `T::write_null`). This is what lets a single `Vec<T>` / `[T; N]` / `IndexMap<K, V>` impl cover
/// both nullable and non-nullable element types. It also makes a second layer valid:
/// `Option<Option<T>>` is equivalent to `Option<T>` — same logical type, a NULL reads back only as
/// the outer `None`, and `None` and `Some(None)` both write NULL — which is handy when a wrapper
/// cannot easily strip the middle type.
// 裸指针由 DuckDB FFI 提供，此处直接转发
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<T: DuckValueType> DuckValueType for Option<T> {
    fn type_id() -> TypeId {
        T::type_id()
    }

    fn logical_type() -> LogicalType {
        T::logical_type()
    }

    /// 可空类型的标志：NULL 槽位取值为 `None`，而不是让上层整体作废。
    ///
    /// The marker of a nullable type: a NULL slot yields `None` instead of invalidating the
    /// enclosing value.
    fn from_null() -> Option<Self> {
        Some(None)
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        T::create_reader_from_vector(vector, size)
    }

    /// 读到有效行时把 `T` 的值包成 `Some`；`T` 自己表达不了该行时（返回 `None`）整体视为 NULL。
    ///
    /// Reads a valid row and wraps `T`'s value in `Some`; when `T` itself cannot represent that
    /// row (returns `None`) the whole value is NULL.
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        T::read_valid(reader, row).map(Some)
    }

    /// 把 `&[Option<&Option<T>>]` 压平成 `&[Option<&T>]` 后转发给 `T`：
    /// 行值存在（`Some`）但自身是 `None` 时，这里会变成「写 NULL」。
    ///
    /// Flattens `&[Option<&Option<T>>]` into `&[Option<&T>]` and forwards to `T`: a present row
    /// (`Some`) whose value is `None` becomes a NULL write.
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let flattened: Vec<Option<&T>> = output_vec
            .iter()
            .map(|row| row.as_ref().and_then(|value| value.as_ref()))
            .collect();
        T::create_writer_batch(vector, &flattened)
    }

    /// 必须重写：默认实现会走 [`write_valid_to_vector_writer`](DuckValueType::write_valid_to_vector_writer)
    /// 的 `todo!()`。`Some(v)` 转发 `T::write_valid`，`None` 转发 `T::write_null`。
    ///
    /// Must be overridden: the default would fall through to the `todo!()` in
    /// [`write_valid_to_vector_writer`](DuckValueType::write_valid_to_vector_writer).
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        match vo {
            Some(value) => T::write_valid(writer, idx, value),
            None => T::write_null(writer, idx),
        }
    }

    /// NULL 行同样要转发给 `T`：STRUCT 必须把子字段一起置 NULL，LIST / MAP 还要写空 entry。
    ///
    /// NULL rows forward to `T` as well: a STRUCT must NULL its child fields and a LIST / MAP
    /// must write the empty entry.
    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        T::write_null(writer, idx)
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(writer)
    }

    fn struct_field_writer_batch(
        struct_writer: &DuckValueWriter,
        field_index: usize,
        output_vec: &[Option<&Self>],
    ) -> DuckValueWriter {
        let flattened: Vec<Option<&T>> = output_vec
            .iter()
            .map(|row| row.as_ref().and_then(|value| value.as_ref()))
            .collect();
        T::struct_field_writer_batch(struct_writer, field_index, &flattened)
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Some(T::read_by_duck_value_valid(value)?))
    }

    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Some(T::read_by_duck_value_valid_simple(value))
    }
}
