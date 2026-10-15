//! DuckDB 1.5 的 `TIME_NS`（[`DuckTimeNs`]）：这个模块整个挂在 `duckdb-1-5` feature 后面。
//!
//! DuckDB 1.5's `TIME_NS` ([`DuckTimeNs`]): this whole module sits behind the `duckdb-1-5` feature.
//!
//! `TIME_NS` 是 DuckDB 1.5 新增的逻辑类型，quack-rs 把 `TypeId::TimeNs`（以及
//! `Value::as_time_ns` / `Value::time_ns`）也门控在同一个 feature 后面。所以这里不是「某个函数
//! 需要开 feature」，而是**这一整块能力需要**：feature 关掉时模块根本不存在，`duckfn::DuckTimeNs`
//! 也就没有 —— feature 标在模块边界上，不散落到各个函数上（本 crate 的惯例）。
//!
//! `TIME_NS` is a logical type DuckDB added in 1.5, and quack-rs gates `TypeId::TimeNs` (along with
//! `Value::as_time_ns` / `Value::time_ns`) behind the same feature. It is not "one function needs a
//! feature" but "this whole capability does": with the feature off the module does not exist, and
//! neither does `duckfn::DuckTimeNs` — features are marked at module boundaries rather than sprinkled
//! over individual functions (this crate's convention).
//!
//! chrono 互操作（[`DuckTimeNs::to_naive_time`] / [`DuckTimeNs::from_naive_time`]）还额外要 `chrono`
//! feature —— 那是另一条独立的轴，所以那部分用块级 `#[cfg]` 留在本文件里。
//!
//! The chrono interop ([`DuckTimeNs::to_naive_time`] / [`DuckTimeNs::from_naive_time`]) additionally
//! needs the `chrono` feature. That is a second, independent axis, so it stays in this file behind an
//! item-level `#[cfg]`.

use quack_rs::prelude::{TypeId, Value, VectorReader, VectorWriter};

use crate::{DuckResult, DuckValueType, duck_error};

/// DuckDB `TIME_NS`（自 00:00:00 起的纳秒数）。
///
/// DuckDB `TIME_NS` (nanoseconds since midnight).
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
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Self {
            nanos_since_midnight: value
                .as_time_ns()
                .ok_or_else(|| duck_error("expected a TIME_NS value"))?,
        })
    }
}

// ---------------------------------------------------------------------------
// chrono 互操作（`chrono` feature；本模块本身已经要 `duckdb-1-5`）
//
// The chrono interop (`chrono` feature; the module itself already needs `duckdb-1-5`).
// ---------------------------------------------------------------------------

/// 一天的纳秒数；`TIME_NS` 的合法取值上界（不含）。
///
/// Nanoseconds in a day; the exclusive upper bound of a valid `TIME_NS`.
#[cfg(feature = "chrono")]
const NANOS_PER_DAY: i64 = 86_400 * 1_000_000_000;

/// 把「chrono 表示不了」构造成错误：复用 chrono 桥那一份实现，两边的措辞保持一致。
///
/// Builds the "not representable by chrono" error: it reuses the chrono bridge's implementation, so
/// both sides word it the same way.
#[cfg(feature = "chrono")]
use crate::value_types::chrono_bridge::out_of_range;
#[cfg(feature = "chrono")]
use chrono::{NaiveTime, Timelike};

#[cfg(feature = "chrono")]
impl DuckTimeNs {
    /// 转成 `chrono::NaiveTime`。
    ///
    /// `TIME_NS` 是「自 00:00:00 起的纳秒数」，合法区间是 `[0, 86_400_000_000_000)`。
    ///
    /// Converts to a `chrono::NaiveTime`. `TIME_NS` counts nanoseconds since 00:00:00 and is only
    /// valid in `[0, 86_400_000_000_000)`.
    ///
    /// # Errors
    ///
    /// 取值不在上面那个区间内时返回错误（不 panic，也不做钳位）。
    ///
    /// Returns an error when the value is outside that range (no panic, no clamping).
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
    ///
    /// # Errors
    ///
    /// 输入是 chrono 的闰秒表示（同一天的 23:59:60 之后）时返回错误 —— DuckDB 的 `TIME_NS` 装不下它。
    ///
    /// Returns an error for chrono's leap-second representation (past 23:59:60 of that day), which a
    /// DuckDB `TIME_NS` cannot hold.
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

#[cfg(all(test, feature = "chrono"))]
mod tests {
    use super::DuckTimeNs;
    use chrono::NaiveTime;

    /// 纳秒精度的往返不丢位（`TIME` 那条路会截到微秒，这条不会）。
    ///
    /// A nanosecond-precision round trip loses nothing (the `TIME` path truncates to microseconds;
    /// this one does not).
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

    /// 越界（合法区间是一天）返回错误，而不是 panic 或钳位。
    ///
    /// Out-of-range values (the valid range is one day) are an error rather than a panic or a clamp.
    #[test]
    fn time_ns_out_of_range_is_an_error() {
        for nanos in [-1, 86_400 * 1_000_000_000] {
            let value = DuckTimeNs {
                nanos_since_midnight: nanos,
            };
            let error = value.to_naive_time().expect_err("out of range");
            assert!(error.as_str().contains("TIME_NS"), "{error}");
        }
    }
}
