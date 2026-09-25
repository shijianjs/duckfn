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
/// `Vec<T>`（别名 `DuckList`）与 DuckDB `LIST` 的映射。
///
/// Mappings between `Vec<T>` (aliased as `DuckList`) and DuckDB `LIST`.
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
// 时间包装类型与 chrono 的互操作（`to_naive_date` / `from_naive_date` 等固有方法），
// 由 `chrono` feature 开关；模块文档见 `src/value_types/chrono_bridge.rs`。
//
// Interop between the time wrapper types and chrono (`to_naive_date` / `from_naive_date` and
// friends), behind the `chrono` feature; the module docs live in
// `src/value_types/chrono_bridge.rs`.
#[cfg(feature = "chrono")]
pub(crate) mod chrono_bridge;
// `DuckUuid` 与 uuid crate 的互操作（`to_uuid` / `from_uuid`），由 `uuid` feature 开关；
// 模块文档见 `src/value_types/uuid_bridge.rs`。
//
// Interop between `DuckUuid` and the uuid crate (`to_uuid` / `from_uuid`), behind the `uuid`
// feature; the module docs live in `src/value_types/uuid_bridge.rs`.
#[cfg(feature = "uuid")]
pub(crate) mod uuid_bridge;
// `DuckDecimal<W, S>` 与 rust_decimal 的互操作（`to_decimal` / `from_decimal`），由
// `rust_decimal` feature 开关；模块文档见 `src/value_types/rust_decimal_bridge.rs`。
//
// Interop between `DuckDecimal<W, S>` and rust_decimal (`to_decimal` / `from_decimal`), behind the
// `rust_decimal` feature; the module docs live in `src/value_types/rust_decimal_bridge.rs`.
#[cfg(feature = "rust_decimal")]
pub(crate) mod rust_decimal_bridge;
/// `Option<T>` 与可空值的映射：可空性由值承载，逻辑类型与 `T` 相同。
///
/// Mappings between `Option<T>` and nullable values: nullability is carried by the value and the
/// logical type equals `T`'s.
pub(crate) mod duck_option;
/// `IndexMap<K, V>`（别名 `DuckMap`）与 DuckDB `MAP` 的映射。
///
/// Mappings between `IndexMap<K, V>` (aliased as `DuckMap`) and DuckDB `MAP`.
pub(crate) mod duck_map;
/// `#[derive(DuckStruct)]` 生成的 STRUCT 结构体所需的 `DuckStructTrait` 及通用实现。
///
/// `DuckStructTrait` (required by `#[derive(DuckStruct)]`-generated STRUCT structs) plus the
/// blanket implementations wiring it into `DuckValueType` / `DuckColumns` / `DuckBindArgs`.
pub(crate) mod duck_struct;
/// `DuckLazy<T>`：本行内的延迟读取（只读），用于「多行不变的复杂配置项」这类场景。
///
/// `DuckLazy<T>`: a deferred read scoped to one row (read-only), for cases such as a complex
/// configuration argument that stays constant across rows.
pub(crate) mod duck_lazy;
/// `DuckLazySlot<T>`：把 `DuckLazy<T>` 解析一次并留在聚合状态里，供后续所有行与合并复用。
///
/// `DuckLazySlot<T>`: parse a `DuckLazy<T>` once and keep it in the aggregate state, so every later
/// row and every merge reuses it.
pub(crate) mod duck_lazy_slot;
/// `#[derive(DuckEnum)]` 生成的 ENUM 读写工具，以及可选的「加载期建类型」。
///
/// ENUM read/write helpers used by `#[derive(DuckEnum)]`-generated code, plus the optional
/// "create the type at load time" step.
pub(crate) mod duck_enum;
/// 把逻辑类型渲染成 SQL，并在加载期注册成 DuckDB 的命名类型（`create_type`）。
///
/// Renders a logical type as SQL and registers it as a named DuckDB type at load time
/// (`create_type`).
pub(crate) mod named_types;
/// `LIST` / `MAP` / `STRUCT` 的物理布局写法：静态类型通路与动态列通路共用同一份实现。
///
/// Physical-layout writes for `LIST` / `MAP` / `STRUCT`, shared by the static-type path and the
/// dynamic-column path.
pub(crate) mod vector_layout;

pub use named_types::*;
pub use duck_value_type::*;
pub use duck_array::*;
pub use duck_list::*;
pub use duck_map::*;
pub use wrapper_types::*;
pub use duck_struct::*;
pub use duck_lazy::*;
pub use duck_lazy_slot::*;
pub use duck_enum::*;

