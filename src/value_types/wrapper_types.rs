//! 「物理表示相同、语义不同」的 DuckDB 包装类型。
//!
//! 同一个物理表示（`i64` / `i128` / `u128` / `u64`）在不同 DuckDB 逻辑类型下含义不同
//! （时间戳精度、时间、日期、UUID...），因此用 newtype 包装，避免映射歧义。
//!
//! Wrapper types for DuckDB logical types that share a physical representation but differ in
//! semantics. The same physical layout (`i64` / `i128` / `u128` / `u64`) means different
//! things under different DuckDB logical types (timestamp precision, time, date, UUID, ...),
//! so newtypes are used to remove the ambiguity.

use std::fmt::Debug;
use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType, DuckValueWriter};
use quack_rs::interval::DuckInterval;
use quack_rs::prelude::{LogicalType, TypeId, Value, VectorReader, VectorWriter};
use crate::DuckResult;

/// DuckDB `TIMESTAMP`（微秒精度，无时区）。
///
/// DuckDB `TIMESTAMP` (microsecond precision, without time zone).
//TypeId::Timestamp
#[derive(Default,Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestamp {
    /// 自 Unix 纪元（1970-01-01）起的微秒数。
    ///
    /// Microseconds since the Unix epoch (1970-01-01).
    pub micros_since_epoch: i64,
}

/// `DuckValueType` 实现：按 `TIMESTAMP` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIMESTAMP`.
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

/// DuckDB `TIMESTAMP WITH TIME ZONE`（微秒精度，UTC）。
///
/// `TIMESTAMPTZ` 与 `TIMESTAMP` 共用同一种 `i64` 存储，单位也是**微秒**（不是毫秒）：
/// quack-rs 的 `read_timestamp_tz` / `Value::as_timestamp_tz` 都按微秒返回。
///
/// DuckDB `TIMESTAMP WITH TIME ZONE` (microsecond precision, UTC). `TIMESTAMPTZ` shares
/// `TIMESTAMP`'s `i64` storage and its unit is **microseconds**, not milliseconds: quack-rs'
/// `read_timestamp_tz` and `Value::as_timestamp_tz` both return microseconds.
// TypeId::TimestampTz
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampTz {
    /// 自 Unix 纪元起的微秒数（UTC）。
    ///
    /// Microseconds since the Unix epoch (UTC).
    pub micros_since_epoch: i64,
}

// pub const unsafe fn write_timestamp_tz(&mut self, idx: usize, micros_since_epoch: i64) {

/// `DuckValueType` 实现：按 `TIMESTAMP WITH TIME ZONE` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIMESTAMP WITH TIME ZONE`.
impl DuckValueType for DuckTimestampTz {
    fn type_id() -> TypeId {
        TypeId::TimestampTz
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_epoch: unsafe { reader.read_timestamp_tz(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_tz(idx, v.micros_since_epoch) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self{
            micros_since_epoch:value.as_timestamp_tz()
        }
    }

}

/// DuckDB `TIMESTAMP_S`（秒精度）。
///
/// DuckDB `TIMESTAMP_S` (second precision).
// TypeId::TimestampS
// pub const unsafe fn write_timestamp_s(&mut self, idx: usize, seconds_since_epoch: i64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampS {
    /// 自 Unix 纪元起的秒数。
    ///
    /// Seconds since the Unix epoch.
    pub seconds_since_epoch: i64,
}

/// `DuckValueType` 实现：按 `TIMESTAMP_S` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIMESTAMP_S`.
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

/// DuckDB `TIMESTAMP_MS`（毫秒精度）。
///
/// DuckDB `TIMESTAMP_MS` (millisecond precision).
// TypeId::TimestampMs
// pub const unsafe fn write_timestamp_ms(&mut self, idx: usize, millis_since_epoch: i64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampMs {
    /// 自 Unix 纪元起的毫秒数。
    ///
    /// Milliseconds since the Unix epoch.
    pub millis_since_epoch: i64,
}

/// `DuckValueType` 实现：按 `TIMESTAMP_MS` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIMESTAMP_MS`.
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

/// DuckDB `TIMESTAMP_NS`（纳秒精度）。
///
/// DuckDB `TIMESTAMP_NS` (nanosecond precision).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampNs {
    /// 自 Unix 纪元起的纳秒数。
    ///
    /// Nanoseconds since the Unix epoch.
    pub nanos_since_epoch: i64,
}

/// `DuckValueType` 实现：按 `TIMESTAMP_NS` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIMESTAMP_NS`.
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

/// DuckDB `TIME WITH TIME ZONE`。
///
/// DuckDB `TIME WITH TIME ZONE`.
// TypeId::TimeTz
// pub const unsafe fn write_time_tz(&mut self, idx: usize, bits: u64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimeTz {
    /// 时间的位编码（微秒 + 时区偏移，按 DuckDB 内部格式打包）。
    ///
    /// Bit-packed representation of the time (microseconds plus time-zone offset, in DuckDB's
    /// internal format).
    pub bits: u64,
}

/// `DuckValueType` 实现：按 `TIME WITH TIME ZONE` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIME WITH TIME ZONE`.
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

/// 定点小数 `DECIMAL(WIDTH, SCALE)`：用 i128 存未缩放整数。
///
/// 实际值是 `unscaled / 10^SCALE`；`WIDTH` 与 `SCALE` 由常量泛型参数决定，
/// 从而在类型层面区分 `DECIMAL(18, 2)` 与 `DECIMAL(18, 4)` 等。
///
/// Fixed-point decimal `DECIMAL(WIDTH, SCALE)` backed by an unscaled i128. The real value is
/// `unscaled / 10^SCALE`; `WIDTH` and `SCALE` are const generic parameters, so `DECIMAL(18, 2)`
/// and `DECIMAL(18, 4)` are distinct types.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckDecimal<const WIDTH: u8, const SCALE: u8> {
    /// 未缩放的整数值。
    ///
    /// The unscaled integer value.
    pub unscaled: i128,
}

/// `DuckValueType` 实现：按 `DECIMAL(WIDTH, SCALE)` 读写。
///
/// `DuckValueType` implementation: reads and writes as `DECIMAL(WIDTH, SCALE)`.
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
    /// DECIMAL 的 `read_valid` 需要 `WIDTH` 才能解出未缩放值，因此整体重写。
    ///
    /// DECIMAL's `read_valid` needs `WIDTH` to decode the unscaled value, so it is overridden
    /// as a whole.
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self {
            unscaled: unsafe { reader.vector_reader.read_decimal(row, WIDTH) },
        })
    }
    /// 同理，写入时需要把 `WIDTH` 传给底层 writer。
    ///
    /// Likewise, writing must pass `WIDTH` down to the underlying writer.
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        unsafe { writer.vector_writer.write_decimal(idx, WIDTH, vo.unscaled) }
    }
}

/// DuckDB `DATE`（自纪元起的天数）。
///
/// DuckDB `DATE` (days since the epoch).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckDate {
    /// 自 1970-01-01 起的天数（可为负）。
    ///
    /// Days since 1970-01-01 (may be negative).
    pub days_since_epoch: i32,
}

/// `DuckValueType` 实现：按 `DATE` 读写。
///
/// `DuckValueType` implementation: reads and writes as `DATE`.
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

/// DuckDB `TIME`（自午夜起的微秒数）。
///
/// DuckDB `TIME` (microseconds since midnight).
// pub const unsafe fn write_time(&mut self, idx: usize, micros_since_midnight: i64) {
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTime {
    /// 自 00:00:00 起的微秒数。
    ///
    /// Microseconds since 00:00:00.
    pub micros_since_midnight: i64,
}

/// `DuckValueType` 实现：按 `TIME` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIME`.
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

/// DuckDB `TIME_NS`（自 00:00:00 起的纳秒数）。
///
/// 需要开启 `duckdb-1-5` feature：`TIME_NS` 是 DuckDB 1.5 新增的类型，
/// quack-rs 里对应的 `TypeId::TimeNs` 由该 feature 门控。
///
/// DuckDB `TIME_NS` (nanoseconds since midnight). Requires the `duckdb-1-5` feature:
/// `TIME_NS` was added in DuckDB 1.5, and quack-rs gates `TypeId::TimeNs` behind that feature.
#[cfg(feature = "duckdb-1-5")]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimeNs {
    /// 自 00:00:00 起的纳秒数。
    ///
    /// Nanoseconds since 00:00:00.
    pub nanos_since_midnight: i64,
}

/// `DuckValueType` 实现：按 `TIME_NS` 读写。
///
/// `DuckValueType` implementation: reads and writes as `TIME_NS`.
#[cfg(feature = "duckdb-1-5")]
impl DuckValueType for DuckTimeNs {
    fn type_id() -> TypeId {
        TypeId::TimeNs
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        // TIME_NS 在向量里就是一个 i64（纳秒）。quack-rs 0.16 还没有 read_time_ns，
        // 因此直接读底层槽位。
        //
        // A TIME_NS slot is a plain i64 (nanoseconds). quack-rs 0.16 has no `read_time_ns`
        // yet, so the underlying slot is read directly.
        Self {
            nanos_since_midnight: unsafe { reader.read_i64(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i64(idx, v.nanos_since_midnight) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self {
            nanos_since_midnight: value.as_time_ns(),
        }
    }
}

/// DuckDB `BLOB`（任意字节串）。
///
/// DuckDB `BLOB` (an arbitrary byte string).
// pub unsafe fn read_blob(&self, idx: usize) -> &[u8] {
// pub unsafe fn write_blob(&mut self, idx: usize, value: &[u8]) {
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct DuckBlob {
    /// 字节内容。
    ///
    /// The byte contents.
    pub value: Vec<u8>,
}

/// `DuckValueType` 实现：按 `BLOB` 读写。
///
/// `DuckValueType` implementation: reads and writes as `BLOB`.
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
    /// `Value::as_blob` 会失败，因此返回 `DuckResult`。
    ///
    /// `Value::as_blob` is fallible, hence the `DuckResult`.
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Self{
            value:value.as_blob()?
        })
    }
}

/// DuckDB `UUID`（128 位）。
///
/// DuckDB `UUID` (128 bits).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckUuid {
    /// UUID 的 128 位表示。
    ///
    /// The 128-bit UUID value.
    pub value: u128,
}

/// `DuckValueType` 实现：按 `UUID` 读写。
///
/// `DuckValueType` implementation: reads and writes as `UUID`.
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

/// DuckDB `INTERVAL`：直接复用 quack-rs 的 [`DuckInterval`]。
///
/// DuckDB `INTERVAL`: reuses quack-rs' [`DuckInterval`] directly.
///
/// `DuckValueType` 实现：按 `INTERVAL` 读写。
///
/// `DuckValueType` implementation: reads and writes as `INTERVAL`.
//TypeId::Interval
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
