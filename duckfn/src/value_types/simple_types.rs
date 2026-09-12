use crate::value_types::duck_value_type::DuckValueType;
use quack_rs::prelude::{TypeId, Value, VectorReader, VectorWriter};
use crate::DuckResult;

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

/// TypeId::BigInt      // i64
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

/// TypeId::TinyInt     // i8
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

/// TypeId::SmallInt    // i16
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

/// TypeId::Integer     // i32
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

/// TypeId::UTinyInt    // u8
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

/// TypeId::USmallInt   // u16
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

// TypeId::UInteger    // u32

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

/// TypeId::HugeInt     // i128
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

/// TypeId::UHugeInt    // u128 不考虑，没read_u128这个方法
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

/// TypeId::Float       // f32
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

/// TypeId::Double      // f64
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

/// TypeId::Varchar
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
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        value.as_str()
    }
}