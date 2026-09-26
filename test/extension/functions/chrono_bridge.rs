// ============================================================================
// chrono 时间桥：duckfn 的 `chrono` feature
//
//   DuckDB 的时间包装类型（DuckDate / DuckTimestamp / DuckTimestampS / DuckTimestampMs /
//   DuckTimestampNs / DuckTimestampTz / DuckTime / DuckTimeNs）只存「原始标量 + 单位」，
//   例如 DuckTimestamp 是「自 1970-01-01 起的微秒数」。要把它们变成可读、可算的日期时间，
//   每个下游扩展都得自己写一遍纪元数学 —— 这正是 duckfn 的 `chrono` feature 要收掉的那份。
//
//   开启后这些类型各拿到一对双向转换（见 src/value_types/chrono_bridge.rs）：
//
//     DuckDate::to_naive_date / from_naive_date
//     DuckTimestamp{,S,Ms,Ns}::to_naive_datetime / from_naive_datetime
//     DuckTimestampTz::to_datetime_utc / from_datetime_utc
//     DuckTime{,Ns}::to_naive_time / from_naive_time
//
//   两条约定：越界（含 DATE / TIMESTAMP 的 `infinity` / `-infinity`）返回 DuckResult 错误而不是
//   panic；micros / millis / seconds / nanos 的换算由方法负责，调用方不用猜字段名。
//
//   The `chrono` feature of duckfn. DuckDB's time wrapper types only hold "a raw scalar plus a
//   unit" — `DuckTimestamp` is "microseconds since 1970-01-01", for instance — so every downstream
//   extension that wants readable, computable date-times ends up writing the epoch arithmetic
//   itself. That is the copy the feature removes: each type gains a pair of conversions (see
//   src/value_types/chrono_bridge.rs), out-of-range values — including DATE / TIMESTAMP
//   `infinity` / `-infinity` — come back as `DuckResult` errors rather than panics, and the
//   micros / millis / seconds / nanos conversion is done for you.
//
// 前置条件：本示例 crate 在 Cargo.toml 里打开了 duckfn 的 `chrono` feature，并直接依赖 chrono
// （duckfn 不 re-export chrono，用到哪些类型就自己加依赖）。
//
// The example crate enables duckfn's `chrono` feature in Cargo.toml and depends on chrono directly
// (duckfn does not re-export chrono; add the dependency for the types you use).
// ============================================================================

use chrono::{Days, Timelike};
use duckfn::{
    DuckDate, DuckOptionResult, DuckTime, DuckTimestamp, DuckTimestampTz, duck_error,
    duck_scalar_function,
};

/// 日期加减天数：`DATE` -> `NaiveDate` -> 加减 -> 再回到 `DATE`。
///
/// 两个方向都要：`to_naive_date` 让日期变得可算，`from_naive_date` 才是把结果写回 DuckDB 的那一步。
/// 越界（`DATE 'infinity'`、或结果超出 chrono 的年份范围）报错，不会 panic。
///
/// ```sql
/// SELECT dfn_chrono_date_add(DATE '2024-01-31', 1);   -- 2024-02-01
/// SELECT dfn_chrono_date_add(DATE '2024-03-01', -1);  -- 2024-02-29
/// ```
///
/// Date arithmetic in days: `DATE` -> `NaiveDate` -> shift -> back to `DATE`. Both directions
/// matter: `to_naive_date` makes the date computable and `from_naive_date` is what writes the
/// result back into DuckDB. Out-of-range input (`DATE 'infinity'`, or a result beyond chrono's
/// year range) is an error, never a panic.
#[duck_scalar_function]
fn dfn_chrono_date_add(d: DuckDate, days: i64) -> DuckOptionResult<DuckDate> {
    let date = d.to_naive_date()?;
    let shifted = if days >= 0 {
        date.checked_add_days(Days::new(days.unsigned_abs()))
    } else {
        date.checked_sub_days(Days::new(days.unsigned_abs()))
    }
    .ok_or_else(|| {
        duck_error(format!(
            "dfn_chrono_date_add: {date} shifted by {days} days is out of chrono's range"
        ))
    })?;
    Ok(Some(DuckDate::from_naive_date(shifted)?))
}

/// `TIMESTAMP` 渲染成 ISO 文本：演示「微秒」这一层的单位换算由 `to_naive_datetime` 负责。
///
/// ```sql
/// SELECT dfn_chrono_timestamp_iso(TIMESTAMP '2024-01-02 03:04:05.123456');
/// -- 2024-01-02 03:04:05.123456
/// ```
///
/// Renders a `TIMESTAMP` as ISO text: the microsecond handling belongs to `to_naive_datetime`, not
/// to the caller.
#[duck_scalar_function]
fn dfn_chrono_timestamp_iso(ts: DuckTimestamp) -> DuckOptionResult<String> {
    let value = ts.to_naive_datetime()?;
    Ok(Some(value.format("%Y-%m-%d %H:%M:%S%.6f").to_string()))
}

/// `TIMESTAMPTZ` 渲染成带 `+00:00` 的 UTC 文本。
///
/// DuckDB 的 `TIMESTAMPTZ` 与 `TIMESTAMP` 共用 `i64` 存储，单位同为微秒（UTC），所以这里给出的
/// 就是 UTC 时间；DuckDB 按会话时区显示它，函数则固定按 UTC 输出。
///
/// ```sql
/// SELECT dfn_chrono_timestamptz_utc(TIMESTAMPTZ '2024-01-02 03:04:05.123456+00');
/// -- 2024-01-02 03:04:05.123456+00:00
/// ```
///
/// Renders a `TIMESTAMPTZ` as UTC text with a `+00:00` offset. DuckDB's `TIMESTAMPTZ` shares
/// `TIMESTAMP`'s `i64` storage and is likewise in microseconds (UTC), so this is the UTC instant —
/// DuckDB displays it in the session's time zone, while this function always emits UTC.
#[duck_scalar_function]
fn dfn_chrono_timestamptz_utc(ts: DuckTimestampTz) -> DuckOptionResult<String> {
    let value = ts.to_datetime_utc()?;
    Ok(Some(value.format("%Y-%m-%d %H:%M:%S%.6f%:z").to_string()))
}

/// 把 `TIME` 拆成 `[时, 分, 秒, 纳秒]`：`to_naive_time` 之后就回到了 chrono 的常规取值方式。
///
/// ```sql
/// SELECT dfn_chrono_time_parts(TIME '03:04:05.123456');
/// -- [3, 4, 5, 123456000]
/// ```
///
/// Splits a `TIME` into `[hour, minute, second, nanosecond]`: once `to_naive_time` has run, the
/// usual chrono accessors apply.
#[duck_scalar_function]
fn dfn_chrono_time_parts(t: DuckTime) -> DuckOptionResult<Vec<i32>> {
    let time = t.to_naive_time()?;
    Ok(Some(vec![
        time.hour() as i32,
        time.minute() as i32,
        time.second() as i32,
        time.nanosecond() as i32,
    ]))
}
