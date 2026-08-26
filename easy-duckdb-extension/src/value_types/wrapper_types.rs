use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use quack_rs::interval::DuckInterval;
use quack_rs::prelude::{LogicalType, TypeId, VectorReader, VectorWriter};
use std::marker::PhantomData;

///TypeId::Timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestamp {
    pub micros_since_epoch: i64,
}

impl DuckValueType for DuckTimestamp {
    fn type_id() -> TypeId {
        TypeId::Timestamp
    }

    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_epoch: unsafe { reader.read_timestamp(row) },
        }
    }

    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp(idx, v.micros_since_epoch) }
    }
}

// TypeId::TimestampTz
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampTz {
    pub millis_since_epoch: i64,
}

// pub const unsafe fn write_timestamp_ms(&mut self, idx: usize, millis_since_epoch: i64) {
impl DuckValueType for DuckTimestampTz {
    fn type_id() -> TypeId {
        TypeId::TimestampTz
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            millis_since_epoch: unsafe { reader.read_timestamp_tz(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_ms(idx, v.millis_since_epoch) }
    }
}

// TypeId::TimestampS
// pub const unsafe fn write_timestamp_s(&mut self, idx: usize, seconds_since_epoch: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampS {
    pub seconds_since_epoch: i64,
}

impl DuckValueType for DuckTimestampS {
    fn type_id() -> TypeId {
        TypeId::TimestampS
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            seconds_since_epoch: unsafe { reader.read_timestamp_s(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_s(idx, v.seconds_since_epoch) }
    }
}

// TypeId::TimestampMs
// pub const unsafe fn write_timestamp_ms(&mut self, idx: usize, millis_since_epoch: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampMs {
    pub millis_since_epoch: i64,
}

impl DuckValueType for DuckTimestampMs {
    fn type_id() -> TypeId {
        TypeId::TimestampMs
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            millis_since_epoch: unsafe { reader.read_timestamp_ms(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_ms(idx, v.millis_since_epoch) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampNs {
    pub nanos_since_epoch: i64,
}

impl DuckValueType for DuckTimestampNs {
    fn type_id() -> TypeId {
        TypeId::TimestampNs
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            nanos_since_epoch: unsafe { reader.read_timestamp_ns(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_ns(idx, v.nanos_since_epoch) }
    }
}

// TypeId::TimeTz
// pub const unsafe fn write_time_tz(&mut self, idx: usize, bits: u64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimeTz {
    pub bits: u64,
}

impl DuckValueType for DuckTimeTz {
    fn type_id() -> TypeId {
        TypeId::TimeTz
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            bits: unsafe { reader.read_time_tz(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_time_tz(idx, v.bits) }
    }
}

// TypeId::Decimal
// pub const unsafe fn read_decimal(&self, idx: usize, WIDTH: u8) -> i128 {
// pub const unsafe fn write_decimal(&mut self, idx: usize, WIDTH: u8, unscaled: i128) {
pub trait DecimalShapeDef:Sized+Clone{
    const WIDTH: u8;
    const SCALE: u8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DuckDecimal<T: DecimalShapeDef> {
    pub unscaled: i128,
    pub scale: u8,
    pub shape: PhantomData<T>,
}

impl<T: DecimalShapeDef> DuckValueType for DuckDecimal<T> {
    fn type_id() -> TypeId {
        TypeId::Decimal
    }
    fn logical_type() -> LogicalType {
        todo!("Decimal暂不可用：\
        可能Decimal有问题，但不清楚怎么处理，且我自己用不到decimal，后面再说；\
        输入的decimal指定类型不合适，可能就是处理decimal的函数；输出可指定类型、但也未必合适了；\
        或许可以分为两个类型、一个读一个写，读用获取到的类型、写用指定的类型");
        LogicalType::decimal(T::WIDTH, T::SCALE)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        let logical = unsafe { quack_rs::vector::vector_get_column_type(reader.c_duckdb_vector) };
        let width = unsafe { logical.decimal_width() };
        let scale = unsafe { logical.decimal_scale() };
        Self {
            scale,
            shape: PhantomData::<T>,
            unscaled: unsafe { reader.vector_reader.read_decimal(row, width) },
        }
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        let logical = unsafe { quack_rs::vector::vector_get_column_type(writer.c_duckdb_vector) };
        let width = unsafe { logical.decimal_width() };
        // let scale = unsafe { logical.decimal_scale() };
        unsafe { writer.vector_writer.write_decimal(idx, width, vo.unscaled) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckDate {
    pub days_since_epoch: i32,
}

impl DuckValueType for DuckDate {
    fn type_id() -> TypeId {
        TypeId::Date
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            days_since_epoch: unsafe { reader.read_date(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_date(idx, v.days_since_epoch) }
    }
}

// pub const unsafe fn write_time(&mut self, idx: usize, micros_since_midnight: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTime {
    pub micros_since_midnight: i64,
}

impl DuckValueType for DuckTime {
    fn type_id() -> TypeId {
        TypeId::Time
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_midnight: unsafe { reader.read_time(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_time(idx, v.micros_since_midnight) }
    }
}

// pub unsafe fn read_blob(&self, idx: usize) -> &[u8] {
// pub unsafe fn write_blob(&mut self, idx: usize, value: &[u8]) {
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckBlob {
    pub value: Vec<u8>,
}

impl DuckValueType for DuckBlob {
    fn type_id() -> TypeId {
        TypeId::Blob
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            value: unsafe { reader.read_blob(row).to_vec() },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_blob(idx, v.value.as_slice()) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckUuid {
    pub value: u128,
}

impl DuckValueType for DuckUuid {
    fn type_id() -> TypeId {
        TypeId::Uuid
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            value: unsafe { reader.read_uuid(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_uuid(idx, v.value) }
    }
}

///TypeId::Interval
impl DuckValueType for DuckInterval {
    fn type_id() -> TypeId {
        TypeId::Interval
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_interval(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_interval(idx, *v) }
    }
}