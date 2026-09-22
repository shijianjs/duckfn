//! `DuckUuid` 与 [`uuid::Uuid`] 的互操作（由 `uuid` feature 开关）。
//!
//! 这一对比时间桥简单得多：两边都是 128 位，`DuckUuid::value` 就是 DuckDB 渲染出来的那 128 位
//! （quack-rs 的 `read_uuid` / `write_uuid` / `Value::as_uuid` 已经替调用方撤销了 DuckDB 内部的
//! 最高位翻转，见它们的文档），而 `Uuid::as_u128` / `Uuid::from_u128` 用的是同一套大端编码。
//! 因此这里**没有**「越界」这种情形，两个方向都不返回 `DuckResult`。
//!
//! Interop between `DuckUuid` and [`uuid::Uuid`], behind the `uuid` feature.
//!
//! This pair is far simpler than the time bridge: both sides are 128 bits, `DuckUuid::value` is
//! exactly the 128 bits DuckDB renders (quack-rs' `read_uuid` / `write_uuid` / `Value::as_uuid`
//! already undo DuckDB's internal top-bit flip — see their docs), and `Uuid::as_u128` /
//! `Uuid::from_u128` use the same big-endian encoding. So there is no "out of range" case here and
//! neither direction returns a `DuckResult`.

use crate::value_types::wrapper_types::DuckUuid;
use uuid::Uuid;

impl DuckUuid {
    /// 转成 `uuid::Uuid`。
    ///
    /// 无损，因此不返回 `DuckResult`：
    ///
    /// ```rust
    /// # use duckfn::DuckUuid;
    /// # use uuid::Uuid;
    /// # fn demo(value: DuckUuid) -> String {
    /// value.to_uuid().to_string()
    /// # }
    /// ```
    ///
    /// Converts to a `uuid::Uuid`. The conversion is lossless, hence no `DuckResult`.
    pub fn to_uuid(self) -> Uuid {
        Uuid::from_u128(self.value)
    }

    /// 由 `uuid::Uuid` 构造。
    ///
    /// 同样无损，因此不需要 `DuckResult`。
    ///
    /// ```rust
    /// # use duckfn::DuckUuid;
    /// # use uuid::Uuid;
    /// # fn demo(value: Uuid) -> DuckUuid {
    /// DuckUuid::from_uuid(value)
    /// # }
    /// ```
    ///
    /// Builds one from a `uuid::Uuid`. Also lossless, so it needs no `DuckResult`.
    pub fn from_uuid(value: Uuid) -> Self {
        Self {
            value: value.as_u128(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4122 里那个「全是 1 到 5」的样例 UUID，用文本形式当参照。
    ///
    /// The "all 1s to 5s" sample UUID from RFC 4122, used here through its textual form.
    const SAMPLE: &str = "11111111-2222-3333-4444-555555555555";

    #[test]
    fn round_trip_is_the_same_uuid() {
        let uuid = Uuid::parse_str(SAMPLE).unwrap();
        let duck = DuckUuid::from_uuid(uuid);
        assert_eq!(duck.to_uuid(), uuid);
        assert_eq!(duck.to_uuid().to_string(), SAMPLE);
    }

    /// `value` 的 16 个大端字节就是 UUID 的规范字节序（文本里的前 8 个字节落在高 64 位）。
    ///
    /// `value`'s 16 big-endian bytes are the canonical UUID byte order, so the first 8 bytes of the
    /// textual form land in the top 64 bits.
    #[test]
    fn value_is_the_canonical_byte_order() {
        let uuid = Uuid::parse_str(SAMPLE).unwrap();
        let duck = DuckUuid::from_uuid(uuid);
        assert_eq!(duck.value.to_be_bytes(), *uuid.as_bytes());
        // 前 8 个字节 `11 11 11 11 22 22 33 33` 就是高 64 位，最高字节是文本的第一个字节。
        //
        // The leading bytes `11 11 11 11 22 22 33 33` are the top 64 bits, so the most significant
        // byte is the text's first byte.
        assert_eq!(duck.value.to_be_bytes()[0], 0x11);
        assert_eq!(duck.value >> 64, 0x1111_1111_2222_3333);
    }
}
