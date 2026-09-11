use duckfn::duck_scalar_function;
use duckfn::{
    DuckBlob, DuckDate, DuckDecimal, DuckTime, DuckTimeTz, DuckTimestamp, DuckTimestampMs,
    DuckTimestampNs, DuckTimestampS, DuckTimestampTz, DuckUuid,
};
use quack_rs::interval::DuckInterval;

// ============================================================================
// 包装类型 echo 函数
//
// 覆盖 wrapper_types.rs 中已实现的全部 DuckValueType：
// Timestamp / TimestampTz / TimestampS / TimestampMs / TimestampNs /
// Time / TimeTz / Date / Decimal / Blob / Uuid / Interval
//
// 每个函数同时验证：
//   - 入参：VectorReader.read_* -> DuckValueType::read
//   - 出参：DuckValueType::write -> VectorWriter.write_*
//   - 类型：type_id / logical_type 映射是否正确
// ============================================================================

/// TypeId::Timestamp // micros since epoch
/// ```sql
/// SELECT dfn_echo_timestamp(TIMESTAMP '2024-01-02 03:04:05');
/// ```
#[duck_scalar_function]
fn dfn_echo_timestamp(i: DuckTimestamp) -> DuckTimestamp {
    i
}

/// TypeId::TimestampTz // micros since epoch (UTC)
/// ```sql
/// SELECT dfn_echo_timestamptz(TIMESTAMPTZ '2024-01-02 03:04:05+00');
/// ```
#[duck_scalar_function]
fn dfn_echo_timestamptz(i: DuckTimestampTz) -> DuckTimestampTz {
    i
}

/// TypeId::TimestampS // seconds since epoch
/// ```sql
/// SELECT dfn_echo_timestamp_s(TIMESTAMP_S '2024-01-02 03:04:05');
/// ```
#[duck_scalar_function]
fn dfn_echo_timestamp_s(i: DuckTimestampS) -> DuckTimestampS {
    i
}

/// TypeId::TimestampMs // millis since epoch
/// ```sql
/// SELECT dfn_echo_timestamp_ms(TIMESTAMP_MS '2024-01-02 03:04:05.123');
/// ```
#[duck_scalar_function]
fn dfn_echo_timestamp_ms(i: DuckTimestampMs) -> DuckTimestampMs {
    i
}

/// TypeId::TimestampNs // nanos since epoch
/// ```sql
/// SELECT dfn_echo_timestamp_ns(TIMESTAMP_NS '2024-01-02 03:04:05.123456789');
/// ```
#[duck_scalar_function]
fn dfn_echo_timestamp_ns(i: DuckTimestampNs) -> DuckTimestampNs {
    i
}

/// TypeId::Time // micros since midnight
/// ```sql
/// SELECT dfn_echo_time(TIME '03:04:05.123456');
/// ```
#[duck_scalar_function]
fn dfn_echo_time(i: DuckTime) -> DuckTime {
    i
}

/// TypeId::TimeTz // packed u64
/// ```sql
/// SELECT dfn_echo_timetz(TIMETZ '03:04:05+02');
/// ```
#[duck_scalar_function]
fn dfn_echo_timetz(i: DuckTimeTz) -> DuckTimeTz {
    i
}

/// TypeId::Date // days since epoch
/// ```sql
/// SELECT dfn_echo_date(DATE '2024-01-02');
/// ```
#[duck_scalar_function]
fn dfn_echo_date(i: DuckDate) -> DuckDate {
    i
}

/// TypeId::Decimal // DECIMAL(18, 3)
/// ```sql
/// SELECT dfn_echo_decimal(1.234::DECIMAL(18,3));
/// ```
#[duck_scalar_function]
fn dfn_echo_decimal(i: DuckDecimal<18, 3>) -> DuckDecimal<18, 3> {
    i
}

/// TypeId::Blob // Vec<u8>
/// ```sql
/// SELECT dfn_echo_blob('\xAA\xBB'::BLOB);
/// ```
#[duck_scalar_function]
fn dfn_echo_blob(i: DuckBlob) -> DuckBlob {
    i
}

/// TypeId::Uuid // textual 128 bits
/// ```sql
/// SELECT dfn_echo_uuid('11111111-2222-3333-4444-555555555555'::UUID);
/// ```
#[duck_scalar_function]
fn dfn_echo_uuid(i: DuckUuid) -> DuckUuid {
    i
}

/// TypeId::Interval // months / days / micros
/// ```sql
/// SELECT dfn_echo_interval(INTERVAL 1 MONTH);
/// ```
#[duck_scalar_function]
fn dfn_echo_interval(i: DuckInterval) -> DuckInterval {
    i
}
