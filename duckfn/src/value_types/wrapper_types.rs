use std::fmt::Debug;
use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use quack_rs::interval::DuckInterval;
use quack_rs::prelude::{LogicalType, TypeId, Value, VectorReader, VectorWriter};
use crate::DuckResult;

///TypeId::Timestamp
#[derive(Default,Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            micros_since_epoch:value.as_timestamp()
        }
    }
}

// TypeId::TimestampTz
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            millis_since_epoch:value.as_timestamp_tz()
        }
    }

}

// TypeId::TimestampS
// pub const unsafe fn write_timestamp_s(&mut self, idx: usize, seconds_since_epoch: i64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            seconds_since_epoch:value.as_timestamp_s()
        }
    }
}

// TypeId::TimestampMs
// pub const unsafe fn write_timestamp_ms(&mut self, idx: usize, millis_since_epoch: i64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            millis_since_epoch:value.as_timestamp_ms()
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            nanos_since_epoch:value.as_timestamp_ns()
        }
    }
}

// TypeId::TimeTz
// pub const unsafe fn write_time_tz(&mut self, idx: usize, bits: u64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            bits:value.as_time_tz()
        }
    }
}


#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckDecimal<const WIDTH: u8, const SCALE: u8> {
    pub unscaled: i128,
}

impl<const WIDTH: u8, const SCALE: u8> DuckValueType for DuckDecimal<WIDTH, SCALE> {
    fn type_id() -> TypeId {
        TypeId::Decimal
    }
    fn logical_type() -> LogicalType {
        LogicalType::decimal(WIDTH, SCALE)
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self {
            unscaled: value.as_decimal().value,
        }
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self {
            unscaled: unsafe { reader.vector_reader.read_decimal(row, WIDTH) },
        })
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        unsafe { writer.vector_writer.write_decimal(idx, WIDTH, vo.unscaled) }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            days_since_epoch:value.as_date()
        }
    }
}

// pub const unsafe fn write_time(&mut self, idx: usize, micros_since_midnight: i64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            micros_since_midnight:value.as_time()
        }
    }
}

// pub unsafe fn read_blob(&self, idx: usize) -> &[u8] {
// pub unsafe fn write_blob(&mut self, idx: usize, value: &[u8]) {
#[derive(Default, Debug, Clone, PartialEq, Eq)]
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
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Self{
            value:value.as_blob()?
        })
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            value:value.as_uuid()
        }
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
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        value.as_interval()
    }
}