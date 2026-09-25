//! 时间包装类型与 `chrono` 的互操作（由 `chrono` feature 开关）。
//!
//! 本模块只给 [`wrapper_types`](crate::value_types::wrapper_types) 里的时间类型加**固有关联方法**：
//! `DuckDate` / `DuckTimestamp` / `DuckTimestampS` / `DuckTimestampMs` / `DuckTimestampNs` /
//! `DuckTimestampTz` / `DuckTime`（以及 `duckdb-1-5` 下的 `DuckTimeNs`）各自与 chrono 的对应类型
//! 互转。没有这些方法时，每个用到时间的下游扩展都得自己写一遍「自纪元起的微秒」数学。
//!
//! 两条硬约束：
//!
//! - **不 panic**：DuckDB 的时间值域（`DATE` 是 `i32` 天、`TIMESTAMP` 是 `i64` 微秒）远大于
//!   chrono 能表示的范围，而且 `DATE` / `TIMESTAMP` 还有 `infinity` / `-infinity` 两个哨兵值
//!   （实测 `DATE` 是 `±i32::MAX`，`TIMESTAMP` 系列是 `±i64::MAX`）。越界一律返回
//!   [`DuckResult`] 错误，不做饱和、也不截断成另一个时间点。
//! - **单位换算由本模块承担**：`micros` / `millis` / `seconds` / `nanos` 的换算写在方法里，
//!   调用方不必再去猜字段名对应哪个精度。
//!
//! 与 DuckDB 保持一致的一点：从高精度往低精度转（`from_naive_datetime` 写进
//! `TIMESTAMP_S` / `TIMESTAMP_MS`，或写进 `TIME`）按 DuckDB 自己的做法**向零截断**。
//!
//! Interop between the time wrapper types and `chrono`, behind the `chrono` feature.
//!
//! This module only adds **inherent methods** to the time types in
//! [`wrapper_types`](crate::value_types::wrapper_types): `DuckDate` / `DuckTimestamp` /
//! `DuckTimestampS` / `DuckTimestampMs` / `DuckTimestampNs` / `DuckTimestampTz` / `DuckTime` (plus
//! `DuckTimeNs` under `duckdb-1-5`) each convert to and from the corresponding chrono type. Without
//! them every downstream extension that touches time has to redo the "microseconds since the epoch"
//! arithmetic itself.
//!
//! Two hard rules:
//!
//! - **No panicking.** DuckDB's time domains (`DATE` is `i32` days, `TIMESTAMP` is `i64`
//!   microseconds) are far wider than what chrono can represent, and `DATE` / `TIMESTAMP` also carry
//!   the sentinels `infinity` / `-infinity` (measured: `DATE` uses `±i32::MAX` and the `TIMESTAMP`
//!   family uses `±i64::MAX`). Anything out of range returns a [`DuckResult`] error: no saturation
//!   and no silent wrap to a different instant.
//! - **Unit conversion is this module's job**: micros / millis / seconds / nanos are handled here so
//!   callers do not have to guess which precision a field name means.
//!
//! One behaviour mirrors DuckDB: narrowing a value (`from_naive_datetime` into `TIMESTAMP_S` /
//! `TIMESTAMP_MS`, or into `TIME`) **truncates toward zero**, exactly as DuckDB's own casts do.

use crate::duck_error;
use crate::value_types::wrapper_types::{
    DuckDate, DuckTime, DuckTimestamp, DuckTimestampMs, DuckTimestampNs, DuckTimestampS,
    DuckTimestampTz,
};
#[cfg(feature = "duckdb-1-5")]
use crate::value_types::wrapper_types::DuckTimeNs;
use crate::DuckResult;
use chrono::{
    DateTime, Datelike, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc,
};

/// Unix 纪元，`NaiveDateTime` 形式。
///
/// 这里从 `DateTime::<Utc>::UNIX_EPOCH` 取而不是直接用 `NaiveDateTime::UNIX_EPOCH`：后者在新版
/// chrono 里已经标记为 deprecated。
///
/// The Unix epoch as a `NaiveDateTime`. It is derived from `DateTime::<Utc>::UNIX_EPOCH` rather
/// than read off `NaiveDateTime::UNIX_EPOCH`, which newer chrono versions mark deprecated.
fn unix_epoch_naive() -> NaiveDateTime {
    DateTime::<Utc>::UNIX_EPOCH.naive_utc()
}

/// `NaiveDate` 的天数编号里，1970-01-01 是第几天（天数从 0001-01-01 的第 1 天起算）。
///
/// The day number of 1970-01-01 in `NaiveDate`'s day numbering, which counts days from 0001-01-01
/// as day 1.
const DAYS_FROM_CE_TO_UNIX_EPOCH: i64 = 719_163;

/// 一天的微秒数；`TIME` 的合法取值上界（不含）。
///
/// Microseconds in a day; the exclusive upper bound of a valid `TIME`.
const MICROS_PER_DAY: i64 = 86_400 * 1_000_000;

/// 一天的纳秒数；`TIME_NS` 的合法取值上界（不含）。
///
/// Nanoseconds in a day; the exclusive upper bound of a valid `TIME_NS`.
#[cfg(feature = "duckdb-1-5")]
const NANOS_PER_DAY: i64 = 86_400 * 1_000_000_000;

/// 判断一个 `i64` 时间戳值是否为 DuckDB 的 `infinity` / `-infinity` 哨兵。
///
/// Whether an `i64` timestamp value is one of DuckDB's `infinity` / `-infinity` sentinels.
fn is_infinite_i64(value: i64) -> bool {
    value == i64::MAX || value == -i64::MAX
}

/// 构造「chrono 表示不了」的错误。
///
/// Builds the "not representable by chrono" error.
fn out_of_range(
    duck_type: &str,
    value: impl std::fmt::Display,
    unit: &str,
) -> quack_rs::error::ExtensionError {
    duck_error(format!(
        "duckfn: {duck_type} value {value} ({unit}) is outside the range chrono can represent; \
         DuckDB's `infinity` / `-infinity` and values beyond chrono's ±262143 years cannot be \
         converted"
    ))
}

/// 由「自 Unix 纪元起的秒 + 纳秒」构造 `DateTime<Utc>`；越界返回 `None`（不 panic）。
///
/// Builds a `DateTime<Utc>` from "seconds + nanoseconds since the Unix epoch"; out-of-range values
/// yield `None` instead of panicking.
fn utc_from_epoch(seconds: i64, nanos: u32) -> Option<DateTime<Utc>> {
    match Utc.timestamp_opt(seconds, nanos) {
        LocalResult::Single(value) => Some(value),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// DuckDate <-> chrono::NaiveDate
// ---------------------------------------------------------------------------

impl DuckDate {
    /// 转成 `chrono::NaiveDate`。
    ///
    /// `DATE 'infinity'` / `DATE '-infinity'`（DuckDB 用 `±i32::MAX` 表示）以及超出 chrono 年份
    /// 范围（约 ±262143 年）的值返回错误，不会 panic。
    ///
    /// ```rust
    /// # use duckfn::{DuckDate, DuckResult};
    /// # use chrono::NaiveDate;
    /// # fn demo(d: DuckDate) -> DuckResult<NaiveDate> {
    /// d.to_naive_date()
    /// # }
    /// ```
    ///
    /// Converts to a `chrono::NaiveDate`. `DATE 'infinity'` / `DATE '-infinity'` (DuckDB stores
    /// them as `±i32::MAX`) and values outside chrono's year range (about ±262143 years) produce an
    /// error rather than a panic.
    pub fn to_naive_date(self) -> DuckResult<NaiveDate> {
        let days = self.days_since_epoch;
        if days == i32::MAX || days == -i32::MAX {
            return Err(out_of_range("DATE", days, "days since 1970-01-01"));
        }
        let ce_days = i64::from(days) + DAYS_FROM_CE_TO_UNIX_EPOCH;
        let ce_days = i32::try_from(ce_days)
            .map_err(|_| out_of_range("DATE", days, "days since 1970-01-01"))?;
        NaiveDate::from_num_days_from_ce_opt(ce_days)
            .ok_or_else(|| out_of_range("DATE", days, "days since 1970-01-01"))
    }

    /// 由 `chrono::NaiveDate` 构造。
    ///
    /// chrono 的日期范围换算成天数后不可能超出 `i32`，但仍然走一次检查而不是 `as` 截断。
    ///
    /// Builds one from a `chrono::NaiveDate`. chrono's date range can never overflow the `i32` day
    /// count, but the conversion still goes through a checked conversion instead of an `as` cast.
    pub fn from_naive_date(date: NaiveDate) -> DuckResult<Self> {
        let days = i64::from(date.num_days_from_ce()) - DAYS_FROM_CE_TO_UNIX_EPOCH;
        let days = i32::try_from(days).map_err(|_| {
            duck_error(format!(
                "duckfn: {date} is outside the range of DATE (i32 days since 1970-01-01)"
            ))
        })?;
        Ok(Self {
            days_since_epoch: days,
        })
    }
}

// ---------------------------------------------------------------------------
// 时间戳与 chrono::NaiveDateTime（各自按自己的精度换算）
// Timestamps and chrono::NaiveDateTime (each in its own precision)
// ---------------------------------------------------------------------------

/// 为四种「无时区时间戳」生成同构的 `to_naive_datetime` / `from_naive_datetime`。
///
/// `$field` 是包装类型的字段名（`micros_since_epoch` / `seconds_since_epoch` / …），
/// `$to_epoch` / `$from_epoch` 是两个只用到绑定名（`$value` / `$delta`）的表达式。
///
/// Generates the isomorphic `to_naive_datetime` / `from_naive_datetime` for the four naive
/// timestamp types. `$field` is the wrapper's field name (`micros_since_epoch` /
/// `seconds_since_epoch` / ...) and `$to_epoch` / `$from_epoch` are expressions that only use the
/// bound names `$value` / `$delta`.
macro_rules! naive_timestamp_conversions {
    (
        $ty:ty, $field:ident, $duck_type:literal, $unit:literal,
        to_epoch: |$value:ident| $to_epoch:expr,
        from_epoch: |$delta:ident| $from_epoch:expr $(,)?
    ) => {
        impl $ty {
            /// 转成 `chrono::NaiveDateTime`（无时区）。
            ///
            /// DuckDB 的 `infinity` / `-infinity` 与超出 chrono 范围的值返回错误。
            ///
            /// Converts to a `chrono::NaiveDateTime` (no time zone). DuckDB's
            /// `infinity` / `-infinity` and values beyond chrono's range produce an error.
            pub fn to_naive_datetime(self) -> DuckResult<NaiveDateTime> {
                let $value = self.$field;
                if is_infinite_i64($value) {
                    return Err(out_of_range($duck_type, $value, $unit));
                }
                let (seconds, nanos): (i64, u32) = { $to_epoch };
                utc_from_epoch(seconds, nanos)
                    .map(|utc| utc.naive_utc())
                    .ok_or_else(|| out_of_range($duck_type, $value, $unit))
            }

            /// 由 `chrono::NaiveDateTime` 构造；目标精度装不下的位按 DuckDB 的做法**向零截断**。
            ///
            /// Builds one from a `chrono::NaiveDateTime`; digits the target precision cannot hold
            /// are **truncated toward zero**, as DuckDB's own casts do.
            pub fn from_naive_datetime(value: NaiveDateTime) -> DuckResult<Self> {
                let $delta = value.signed_duration_since(unix_epoch_naive());
                let raw: Option<i64> = { $from_epoch };
                raw.map(|raw| Self { $field: raw })
                    .ok_or_else(|| out_of_range($duck_type, value, $unit))
            }
        }
    };
}

naive_timestamp_conversions!(
    DuckTimestamp,
    micros_since_epoch,
    "TIMESTAMP",
    "microseconds since 1970-01-01",
    to_epoch: |micros| {
        (
            micros.div_euclid(1_000_000),
            micros.rem_euclid(1_000_000) as u32 * 1_000,
        )
    },
    from_epoch: |delta| delta.num_microseconds(),
);

naive_timestamp_conversions!(
    DuckTimestampS,
    seconds_since_epoch,
    "TIMESTAMP_S",
    "seconds since 1970-01-01",
    to_epoch: |seconds| (seconds, 0),
    from_epoch: |delta| Some(delta.num_seconds()),
);

naive_timestamp_conversions!(
    DuckTimestampMs,
    millis_since_epoch,
    "TIMESTAMP_MS",
    "milliseconds since 1970-01-01",
    to_epoch: |millis| {
        (
            millis.div_euclid(1_000),
            millis.rem_euclid(1_000) as u32 * 1_000_000,
        )
    },
    from_epoch: |delta| Some(delta.num_milliseconds()),
);

naive_timestamp_conversions!(
    DuckTimestampNs,
    nanos_since_epoch,
    "TIMESTAMP_NS",
    "nanoseconds since 1970-01-01",
    to_epoch: |nanos| {
        (
            nanos.div_euclid(1_000_000_000),
            nanos.rem_euclid(1_000_000_000) as u32,
        )
    },
    from_epoch: |delta| delta.num_nanoseconds(),
);

// ---------------------------------------------------------------------------
// DuckTimestampTz <-> chrono::DateTime<Utc>
// ---------------------------------------------------------------------------

impl DuckTimestampTz {
    /// 转成 `chrono::DateTime<Utc>`。
    ///
    /// DuckDB 的 `TIMESTAMPTZ` 与 `TIMESTAMP` 共用 `i64` 存储，单位同为**微秒**（UTC），因此这里
    /// 给的就是 UTC 时间；要显示成本地时间由调用方自己换时区。
    ///
    /// Converts to a `chrono::DateTime<Utc>`. DuckDB's `TIMESTAMPTZ` shares `TIMESTAMP`'s `i64`
    /// storage and is likewise in **microseconds** (UTC), so this is the UTC instant; converting to a
    /// local time zone is up to the caller.
    pub fn to_datetime_utc(self) -> DuckResult<DateTime<Utc>> {
        let micros = self.micros_since_epoch;
        if is_infinite_i64(micros) {
            return Err(out_of_range(
                "TIMESTAMPTZ",
                micros,
                "microseconds since 1970-01-01",
            ));
        }
        utc_from_epoch(
            micros.div_euclid(1_000_000),
            micros.rem_euclid(1_000_000) as u32 * 1_000,
        )
        .ok_or_else(|| out_of_range("TIMESTAMPTZ", micros, "microseconds since 1970-01-01"))
    }

    /// 由 `chrono::DateTime<Utc>` 构造；不足微秒的位**向零截断**。
    ///
    /// Builds one from a `chrono::DateTime<Utc>`; digits finer than a microsecond are **truncated
    /// toward zero**.
    pub fn from_datetime_utc(value: DateTime<Utc>) -> DuckResult<Self> {
        let micros = value
            .signed_duration_since(DateTime::<Utc>::UNIX_EPOCH)
            .num_microseconds()
            .ok_or_else(|| out_of_range("TIMESTAMPTZ", value, "microseconds since 1970-01-01"))?;
        Ok(Self {
            micros_since_epoch: micros,
        })
    }
}

// ---------------------------------------------------------------------------
// DuckTime / DuckTimeNs <-> chrono::NaiveTime
// ---------------------------------------------------------------------------

impl DuckTime {
    /// 转成 `chrono::NaiveTime`。
    ///
    /// `TIME` 是「自 00:00:00 起的微秒数」，合法区间是 `[0, 86_400_000_000)`；越界返回错误。
    ///
    /// Converts to a `chrono::NaiveTime`. `TIME` counts microseconds since 00:00:00 and is only
    /// valid in `[0, 86_400_000_000)`; anything else produces an error.
    pub fn to_naive_time(self) -> DuckResult<NaiveTime> {
        let micros = self.micros_since_midnight;
        if !(0..MICROS_PER_DAY).contains(&micros) {
            return Err(out_of_range("TIME", micros, "microseconds since 00:00:00"));
        }
        NaiveTime::from_num_seconds_from_midnight_opt(
            (micros / 1_000_000) as u32,
            (micros % 1_000_000) as u32 * 1_000,
        )
        .ok_or_else(|| out_of_range("TIME", micros, "microseconds since 00:00:00"))
    }

    /// 由 `chrono::NaiveTime` 构造；不足微秒的位**向零截断**。
    ///
    /// Builds one from a `chrono::NaiveTime`; digits finer than a microsecond are **truncated
    /// toward zero**.
    pub fn from_naive_time(value: NaiveTime) -> DuckResult<Self> {
        let micros = i64::from(value.num_seconds_from_midnight()) * 1_000_000
            + i64::from(value.nanosecond() / 1_000);
        if micros >= MICROS_PER_DAY {
            // chrono 把闰秒表示成同一天的 23:59:60 到 23:59:60.999…，DuckDB 的 TIME 装不下。
            //
            // chrono represents a leap second as 23:59:60 … 23:59:60.999… of the same day, which a
            // DuckDB TIME cannot hold.
            return Err(out_of_range("TIME", value, "as a time of day"));
        }
        Ok(Self {
            micros_since_midnight: micros,
        })
    }
}

#[cfg(feature = "duckdb-1-5")]
impl DuckTimeNs {
    /// 转成 `chrono::NaiveTime`。
    ///
    /// `TIME_NS` 是「自 00:00:00 起的纳秒数」，合法区间是 `[0, 86_400_000_000_000)`。
    ///
    /// Converts to a `chrono::NaiveTime`. `TIME_NS` counts nanoseconds since 00:00:00 and is only
    /// valid in `[0, 86_400_000_000_000)`.
    pub fn to_naive_time(self) -> DuckResult<NaiveTime> {
        let nanos = self.nanos_since_midnight;
        if !(0..NANOS_PER_DAY).contains(&nanos) {
            return Err(out_of_range(
                "TIME_NS",
                nanos,
                "nanoseconds since 00:00:00",
            ));
        }
        NaiveTime::from_num_seconds_from_midnight_opt(
            (nanos / 1_000_000_000) as u32,
            (nanos % 1_000_000_000) as u32,
        )
        .ok_or_else(|| out_of_range("TIME_NS", nanos, "nanoseconds since 00:00:00"))
    }

    /// 由 `chrono::NaiveTime` 构造（纳秒精度，不丢位）。
    ///
    /// Builds one from a `chrono::NaiveTime` at nanosecond precision, losing nothing.
    pub fn from_naive_time(value: NaiveTime) -> DuckResult<Self> {
        let nanos = i64::from(value.num_seconds_from_midnight()) * 1_000_000_000
            + i64::from(value.nanosecond());
        if nanos >= NANOS_PER_DAY {
            return Err(out_of_range("TIME_NS", value, "as a time of day"));
        }
        Ok(Self {
            nanos_since_midnight: nanos,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1970-01-01 在 `NaiveDate` 的天数编号里是 719163（`from_num_days_from_ce_opt` 会对不上号
    /// 的话，下面的往返测试全部会失败）。
    ///
    /// 1970-01-01 is day 719163 in `NaiveDate`'s numbering; if that offset were wrong, every
    /// round-trip test below would fail.
    #[test]
    fn epoch_day_number_is_stable() {
        assert_eq!(
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().num_days_from_ce(),
            DAYS_FROM_CE_TO_UNIX_EPOCH as i32
        );
    }

    #[test]
    fn date_round_trip() {
        for date in [
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 2).unwrap(),
            NaiveDate::from_ymd_opt(1, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(-1000, 6, 15).unwrap(),
        ] {
            let duck = DuckDate::from_naive_date(date).unwrap();
            assert_eq!(duck.to_naive_date().unwrap(), date);
        }
        assert_eq!(
            DuckDate::from_naive_date(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
                .unwrap()
                .days_since_epoch,
            0
        );
    }

    #[test]
    fn date_infinity_is_an_error() {
        for days in [i32::MAX, -i32::MAX] {
            let err = DuckDate {
                days_since_epoch: days,
            }
            .to_naive_date()
            .unwrap_err();
            assert!(err.to_string().contains("infinity"), "{err}");
        }
        // 超出 chrono 年份范围（约 ±262143 年）但不是 infinity 的值同样报错。
        //
        // A value beyond chrono's year range (about ±262143 years) that is not infinity errors too.
        let err = DuckDate {
            days_since_epoch: 100_000_000,
        }
        .to_naive_date()
        .unwrap_err();
        assert!(err.to_string().contains("chrono"), "{err}");
    }

    #[test]
    fn timestamp_round_trip() {
        let some = NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_micro_opt(3, 4, 5, 123_456)
            .unwrap();
        let before = NaiveDate::from_ymd_opt(1969, 7, 20)
            .unwrap()
            .and_hms_micro_opt(20, 17, 40, 0)
            .unwrap();

        for value in [unix_epoch_naive(), some, before] {
            assert_eq!(
                DuckTimestamp::from_naive_datetime(value)
                    .unwrap()
                    .to_naive_datetime()
                    .unwrap(),
                value
            );
            assert_eq!(
                DuckTimestampNs::from_naive_datetime(value)
                    .unwrap()
                    .to_naive_datetime()
                    .unwrap(),
                value
            );
        }

        // 四种精度都从同一个值出发，秒/毫秒各自截断到自己的精度。
        //
        // The four precisions all start from the same instant and each truncates to its own unit.
        let micros = DuckTimestamp::from_naive_datetime(some).unwrap().micros_since_epoch;
        assert_eq!(
            micros,
            some.signed_duration_since(unix_epoch_naive())
                .num_microseconds()
                .unwrap()
        );
        assert_eq!(
            DuckTimestampS::from_naive_datetime(some)
                .unwrap()
                .seconds_since_epoch,
            micros / 1_000_000
        );
        assert_eq!(
            DuckTimestampMs::from_naive_datetime(some)
                .unwrap()
                .millis_since_epoch,
            micros / 1_000
        );
        assert_eq!(
            DuckTimestampNs::from_naive_datetime(some)
                .unwrap()
                .nanos_since_epoch,
            micros * 1_000
        );
    }

    #[test]
    fn narrowing_truncates_toward_zero() {
        // 1969-12-31 23:59:59.999999：向零截断得到 0 秒，向下取整会得到 -1。
        //
        // 1969-12-31 23:59:59.999999: truncating toward zero gives 0 seconds, flooring gives -1.
        let negative = unix_epoch_naive() - chrono::Duration::microseconds(1);
        assert_eq!(
            DuckTimestampS::from_naive_datetime(negative)
                .unwrap()
                .seconds_since_epoch,
            0
        );
        assert_eq!(
            DuckTimestampS::from_naive_datetime(negative)
                .unwrap()
                .to_naive_datetime()
                .unwrap(),
            unix_epoch_naive()
        );
    }

    #[test]
    fn timestamp_infinity_is_an_error() {
        for micros in [i64::MAX, -i64::MAX] {
            let err = DuckTimestamp {
                micros_since_epoch: micros,
            }
            .to_naive_datetime()
            .unwrap_err();
            assert!(err.to_string().contains("infinity"), "{err}");
        }
    }

    #[test]
    fn timestamp_tz_round_trip() {
        let value = Utc.timestamp_opt(1_704_164_645, 123_000_000).unwrap();
        assert_eq!(
            DuckTimestampTz::from_datetime_utc(value)
                .unwrap()
                .to_datetime_utc()
                .unwrap(),
            value
        );
        // TIMESTAMPTZ 是微秒精度（与 TIMESTAMP 共用 i64 存储），亚微秒的位会被截掉。
        //
        // TIMESTAMPTZ is microsecond precision (it shares TIMESTAMP's i64 storage), so
        // sub-microsecond digits are dropped.
        let finer = Utc.timestamp_opt(1_704_164_645, 123_456_789).unwrap();
        assert_eq!(
            DuckTimestampTz::from_datetime_utc(finer)
                .unwrap()
                .micros_since_epoch,
            1_704_164_645_123_456
        );
    }

    #[test]
    fn time_round_trip() {
        for time in [
            NaiveTime::from_hms_micro_opt(0, 0, 0, 0).unwrap(),
            NaiveTime::from_hms_micro_opt(12, 34, 56, 789_012).unwrap(),
            NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).unwrap(),
        ] {
            assert_eq!(
                DuckTime::from_naive_time(time)
                    .unwrap()
                    .to_naive_time()
                    .unwrap(),
                time
            );
        }
        // TIME 只有微秒精度，纳秒位会被截掉（TIME_NS 的纳秒往返见下一个测试）。
        //
        // TIME only has microsecond precision, so the nanosecond digits are dropped (nanosecond
        // round-trips for TIME_NS live in the next test).
        let ns = NaiveTime::from_hms_nano_opt(1, 2, 3, 456_789_012).unwrap();
        assert_eq!(
            DuckTime::from_naive_time(ns)
                .unwrap()
                .to_naive_time()
                .unwrap(),
            NaiveTime::from_hms_micro_opt(1, 2, 3, 456_789).unwrap()
        );
    }

    /// `TIME_NS` 需要 `duckdb-1-5`（该逻辑类型是 DuckDB 1.5 新增的）。
    ///
    /// `TIME_NS` needs `duckdb-1-5`, since that logical type was added in DuckDB 1.5.
    #[cfg(feature = "duckdb-1-5")]
    #[test]
    fn time_ns_round_trip() {
        let ns = NaiveTime::from_hms_nano_opt(1, 2, 3, 456_789_012).unwrap();
        assert_eq!(
            DuckTimeNs::from_naive_time(ns)
                .unwrap()
                .to_naive_time()
                .unwrap(),
            ns
        );
    }

    #[test]
    fn time_out_of_range_is_an_error() {
        for micros in [-1, MICROS_PER_DAY] {
            let err = DuckTime {
                micros_since_midnight: micros,
            }
            .to_naive_time()
            .unwrap_err();
            assert!(err.to_string().contains("TIME"), "{err}");
        }
    }
}
