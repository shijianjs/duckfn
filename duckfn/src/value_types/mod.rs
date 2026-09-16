//! DuckDB 值类型映射：核心 trait 与各类具体实现。
//!
//! DuckDB value-type mappings: the core trait and the concrete implementations.

/// 核心 trait `DuckValueType` 及读写辅助类型 `DuckValueReader` / `DuckValueWriter`。
///
/// The core `DuckValueType` trait plus the read/write helpers `DuckValueReader` /
/// `DuckValueWriter`.
pub(crate) mod duck_value_type;
/// Rust 基础类型（整数、浮点、布尔、字符串）与 DuckDB 标量类型的映射。
///
/// Mappings between Rust primitives (integers, floats, bool, string) and DuckDB scalar types.
pub(crate) mod simple_types;
/// `Vec<T>` / `Vec<Option<T>>`（别名 `DuckList` / `DuckOptionList`）与 DuckDB `LIST` 的映射。
///
/// Mappings between `Vec<T>` / `Vec<Option<T>>` (aliased as `DuckList` / `DuckOptionList`) and
/// DuckDB `LIST`.
pub(crate) mod duck_list;
/// 定长数组 `[T; N]` / `[Option<T>; N]` 与 DuckDB `ARRAY` 的映射。
///
/// Mappings between fixed-size arrays `[T; N]` / `[Option<T>; N]` and DuckDB `ARRAY`.
pub(crate) mod duck_array;
/// 时间戳、日期、时间、UUID、Blob 等「物理表示相同、语义不同」的包装类型。
///
/// Wrapper types (timestamp, date, time, UUID, blob, ...) whose physical representation is
/// shared but whose logical semantics differ.
pub(crate) mod wrapper_types;
/// `Option<T>` 与可空值的映射：可空性由值承载，逻辑类型与 `T` 相同。
///
/// Mappings between `Option<T>` and nullable values: nullability is carried by the value and the
/// logical type equals `T`'s.
pub(crate) mod duck_option;
/// `IndexMap<K, V>`（别名 `DuckMap` / `DuckOptionMap`）与 DuckDB `MAP` 的映射。
///
/// Mappings between `IndexMap<K, V>` (aliased as `DuckMap` / `DuckOptionMap`) and DuckDB `MAP`.
pub(crate) mod duck_map;
/// `#[derive(DuckStruct)]` 生成的 STRUCT 结构体所需的 `DuckStructTrait` 及通用实现。
///
/// `DuckStructTrait` (required by `#[derive(DuckStruct)]`-generated STRUCT structs) plus the
/// blanket implementations wiring it into `DuckValueType` / `DuckColumns` / `DuckBindArgs`.
pub(crate) mod duck_struct;

pub use duck_value_type::*;
pub use duck_array::*;
pub use duck_list::*;
pub use duck_map::*;
pub use wrapper_types::*;
pub use duck_struct::*;
