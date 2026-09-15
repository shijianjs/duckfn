use duckfn::duck_table_function;
use duckfn::{
    DuckBlob, DuckDate, DuckDecimal, DuckStruct, DuckTime, DuckTimeNs, DuckTimeTz, DuckTimestamp,
    DuckTimestampMs, DuckTimestampNs, DuckTimestampS, DuckTimestampTz, DuckUuid,
};
use quack_rs::interval::DuckInterval;

use super::table_echo_util::echo_rows;

// ============================================================================
// 包装类型的表函数回显（wrapper_types_scalar_echo.rs 的表函数版本）
//
// 这些类型的 Value 读取依赖 quack-rs 的 as_timestamp / as_decimal / as_blob 等，
// 与向量读取走的是两套实现，所以标量用例覆盖不到 bind 阶段的转换。
//
// 参数统一 Option<T>：NULL 或不传 -> 回显 NULL 行。行数规则见 table_echo_util.rs。
// ============================================================================

/// TIMESTAMP // DuckTimestamp
/// ```sql
/// SELECT v FROM dfn_table_echo_timestamp(TIMESTAMP '2024-01-02 03:04:05');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimestampRow {
    pub v: Option<DuckTimestamp>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_timestamp(
    v: Option<DuckTimestamp>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimestampRow> {
    echo_rows(v, count, |v| TableEchoTimestampRow { v })
}

/// TIMESTAMP WITH TIME ZONE // DuckTimestampTz
/// ```sql
/// SELECT v FROM dfn_table_echo_timestamptz(TIMESTAMPTZ '2024-01-02 03:04:05+08');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimestamptzRow {
    pub v: Option<DuckTimestampTz>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_timestamptz(
    v: Option<DuckTimestampTz>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimestamptzRow> {
    echo_rows(v, count, |v| TableEchoTimestamptzRow { v })
}

/// TIMESTAMP_S // DuckTimestampS
/// ```sql
/// SELECT v FROM dfn_table_echo_timestamp_s(TIMESTAMP_S '2024-01-02 03:04:05');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimestampSRow {
    pub v: Option<DuckTimestampS>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_timestamp_s(
    v: Option<DuckTimestampS>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimestampSRow> {
    echo_rows(v, count, |v| TableEchoTimestampSRow { v })
}

/// TIMESTAMP_MS // DuckTimestampMs
/// ```sql
/// SELECT v FROM dfn_table_echo_timestamp_ms(TIMESTAMP_MS '2024-01-02 03:04:05.123');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimestampMsRow {
    pub v: Option<DuckTimestampMs>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_timestamp_ms(
    v: Option<DuckTimestampMs>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimestampMsRow> {
    echo_rows(v, count, |v| TableEchoTimestampMsRow { v })
}

/// TIMESTAMP_NS // DuckTimestampNs
/// ```sql
/// SELECT v FROM dfn_table_echo_timestamp_ns(TIMESTAMP_NS '2024-01-02 03:04:05.123456789');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimestampNsRow {
    pub v: Option<DuckTimestampNs>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_timestamp_ns(
    v: Option<DuckTimestampNs>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimestampNsRow> {
    echo_rows(v, count, |v| TableEchoTimestampNsRow { v })
}

/// TIME // DuckTime
/// ```sql
/// SELECT v FROM dfn_table_echo_time(TIME '03:04:05.123456');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimeRow {
    pub v: Option<DuckTime>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_time(
    v: Option<DuckTime>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimeRow> {
    echo_rows(v, count, |v| TableEchoTimeRow { v })
}

/// TIME_NS // DuckTimeNs
/// ```sql
/// SELECT v FROM dfn_table_echo_time_ns(TIME_NS '03:04:05.123456789');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimeNsRow {
    pub v: Option<DuckTimeNs>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_time_ns(
    v: Option<DuckTimeNs>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimeNsRow> {
    echo_rows(v, count, |v| TableEchoTimeNsRow { v })
}

/// TIME WITH TIME ZONE // DuckTimeTz
/// ```sql
/// SELECT v FROM dfn_table_echo_timetz(TIMETZ '03:04:05+02');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTimeTzRow {
    pub v: Option<DuckTimeTz>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_timetz(
    v: Option<DuckTimeTz>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTimeTzRow> {
    echo_rows(v, count, |v| TableEchoTimeTzRow { v })
}

/// DATE // DuckDate
/// ```sql
/// SELECT v FROM dfn_table_echo_date(DATE '2024-01-02');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoDateRow {
    pub v: Option<DuckDate>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_date(
    v: Option<DuckDate>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoDateRow> {
    echo_rows(v, count, |v| TableEchoDateRow { v })
}

/// DECIMAL(18,3) // DuckDecimal<18, 3>
/// ```sql
/// SELECT v FROM dfn_table_echo_decimal((-1.234)::DECIMAL(18,3));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoDecimalRow {
    pub v: Option<DuckDecimal<18, 3>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_decimal(
    v: Option<DuckDecimal<18, 3>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoDecimalRow> {
    echo_rows(v, count, |v| TableEchoDecimalRow { v })
}

/// BLOB // DuckBlob
/// ```sql
/// SELECT v FROM dfn_table_echo_blob('\xAA\xBB'::BLOB);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoBlobRow {
    pub v: Option<DuckBlob>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_blob(
    v: Option<DuckBlob>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoBlobRow> {
    echo_rows(v, count, |v| TableEchoBlobRow { v })
}

/// UUID // DuckUuid
/// ```sql
/// SELECT v FROM dfn_table_echo_uuid('3f333df6-90a4-4fda-8dd3-9485d27cee36');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoUuidRow {
    pub v: Option<DuckUuid>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_uuid(
    v: Option<DuckUuid>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoUuidRow> {
    echo_rows(v, count, |v| TableEchoUuidRow { v })
}

/// INTERVAL // DuckInterval
/// ```sql
/// SELECT v FROM dfn_table_echo_interval(INTERVAL 1 MONTH);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoIntervalRow {
    pub v: Option<DuckInterval>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_interval(
    v: Option<DuckInterval>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoIntervalRow> {
    echo_rows(v, count, |v| TableEchoIntervalRow { v })
}
