//! Rust 基础类型与 DuckDB 标量类型的映射（布尔、整数、浮点、字符串）。
//!
//! Mappings between Rust primitives and DuckDB scalar types (bool, integers, floats, strings).

use crate::value_types::duck_value_type::DuckValueType;
use quack_rs::prelude::{TypeId, Value, VectorReader, VectorWriter};
use crate::DuckResult;

/// `bool` ↔ `BOOLEAN`。
///
/// `bool` ↔ `BOOLEAN`.
// TypeId::Boolean

impl DuckValueType for bool {
    fn type_id() -> TypeId {
        TypeId::Boolean
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_bool(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_bool(idx, *v) }
    }

    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_bool()
    }
    
}

/// `i64` ↔ `BIGINT`。
///
/// `i64` ↔ `BIGINT`.
// TypeId::BigInt      // i64
impl DuckValueType for i64 {
    fn type_id() -> TypeId {
        TypeId::BigInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i64(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i64(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_i64()
    }
}

/// `i8` ↔ `TINYINT`。
///
/// `i8` ↔ `TINYINT`.
// TypeId::TinyInt     // i8
impl DuckValueType for i8 {
    fn type_id() -> TypeId {
        TypeId::TinyInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i8(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i8(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_i8()
    }
}

/// `i16` ↔ `SMALLINT`。
///
/// `i16` ↔ `SMALLINT`.
// TypeId::SmallInt    // i16
impl DuckValueType for i16 {
    fn type_id() -> TypeId {
        TypeId::SmallInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i16(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i16(idx, *v) }

    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_i16()
    }
}

/// `i32` ↔ `INTEGER`。
///
/// `i32` ↔ `INTEGER`.
// TypeId::Integer     // i32
impl DuckValueType for i32 {
    fn type_id() -> TypeId {
        TypeId::Integer
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i32(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i32(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_i32()
    }
}

/// `u8` ↔ `UTINYINT`。
///
/// `u8` ↔ `UTINYINT`.
// TypeId::UTinyInt    // u8
impl DuckValueType for u8 {
    fn type_id() -> TypeId {
        TypeId::UTinyInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u8(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u8(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_u8()
    }
}

/// `u16` ↔ `USMALLINT`。
///
/// `u16` ↔ `USMALLINT`.
// TypeId::USmallInt   // u16
impl DuckValueType for u16 {
    fn type_id() -> TypeId {
        TypeId::USmallInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u16(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u16(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_u16()
    }
}

// TypeId::UInteger    // u32：quack-rs 目前没有对应的 read/write 方法，暂未映射。
// TypeId::UInteger    // u32: quack-rs has no matching read/write method yet, so it is unmapped.

/// `u64` ↔ `UBIGINT`。
///
/// `u64` ↔ `UBIGINT`.
// TypeId::UBigInt     // u64
impl DuckValueType for u64 {
    fn type_id() -> TypeId {
        TypeId::UBigInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u64(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u64(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_u64()
    }
}

/// `i128` ↔ `HUGEINT`。
///
/// `i128` ↔ `HUGEINT`.
// TypeId::HugeInt     // i128
impl DuckValueType for i128 {
    fn type_id() -> TypeId {
        TypeId::HugeInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i128(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i128(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_i128()
    }
}

/// `u128` ↔ `UHUGEINT`。
///
/// 注：原注释认为 quack-rs 没有 `read_u128`，但当前版本已提供，故一并实现。
///
/// `u128` ↔ `UHUGEINT`. The original comment claimed quack-rs has no `read_u128`, but the
/// current version provides it, so it is implemented here as well.
// TypeId::UHugeInt    // u128 不考虑，没read_u128这个方法
impl DuckValueType for u128 {
    fn type_id() -> TypeId {
        TypeId::UHugeInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u128(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u128(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_u128()
    }
}

/// `f32` ↔ `FLOAT`。
///
/// `f32` ↔ `FLOAT`.
// TypeId::Float       // f32
impl DuckValueType for f32 {
    fn type_id() -> TypeId {
        TypeId::Float
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f32(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f32(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_f32()
    }
}

/// `f64` ↔ `DOUBLE`。
///
/// `f64` ↔ `DOUBLE`.
// TypeId::Double      // f64
impl DuckValueType for f64 {
    fn type_id() -> TypeId {
        TypeId::Double
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f64(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f64(idx, *v) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_f64()
    }
}

/// `String` ↔ `VARCHAR`。
///
/// `String` ↔ `VARCHAR`.
// TypeId::Varchar
impl DuckValueType for String {
    fn type_id() -> TypeId {
        TypeId::Varchar
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_str(row).to_string() }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_str(idx, v.as_str()) }
    }
    /// `Value::as_str` 在非字符串值上会失败，因此这里返回 `DuckResult` 而不是直接取值。
    ///
    /// `Value::as_str` fails on non-string values, so this returns a `DuckResult` instead of a
    /// plain value.
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        value.as_str()
    }
}
