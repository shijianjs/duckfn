//! `DuckDecimal<W, S>` 与 [`rust_decimal::Decimal`] 的互操作（由 `rust_decimal` feature 开关）。
//!
//! 与时间桥的差别在于**两边的能力不对等**：
//!
//! - DuckDB 的 `DECIMAL(W, S)` 是「未缩放整数（`i128`）+ 标度」，`W` 最大 38；
//! - `rust_decimal` 的尾数是 **96 位**（`2^96 - 1`，约 28~29 位有效数字），标度上限 **28**。
//!
//! 所以 `DECIMAL(38, 0)` 这样的宽类型里有相当一部分值 rust_decimal 装不下，而
//! `Decimal` 的高精度小数写回 `DECIMAL(18, 3)` 也可能丢位。两个方向因此都返回 [`DuckResult`]：
//! 越界、丢位一律报错，不静默截断、也不 panic。`rust_decimal` 自己也提供了不 panic 的
//! `Decimal::try_from_i128_with_scale`，这里用的就是它。
//!
//! Interop between `DuckDecimal<W, S>` and [`rust_decimal::Decimal`], behind the `rust_decimal`
//! feature.
//!
//! Unlike the time bridge, the two sides are **not equally capable**: DuckDB's `DECIMAL(W, S)` is
//! "an unscaled `i128` plus a scale" with `W` up to 38, while `rust_decimal`'s mantissa is **96
//! bits** (`2^96 - 1`, about 28-29 significant digits) and its scale tops out at **28**. So part of
//! a wide type such as `DECIMAL(38, 0)` simply does not fit, and a high-precision `Decimal` may not
//! fit back into `DECIMAL(18, 3)`. Both directions therefore return a [`DuckResult`]: out-of-range
//! and digit-losing conversions are errors — never a silent truncation and never a panic.

use crate::duck_error;
use crate::value_types::wrapper_types::DuckDecimal;
use crate::DuckResult;
use rust_decimal::Decimal;

impl<const WIDTH: u8, const SCALE: u8> DuckDecimal<WIDTH, SCALE> {
    /// 转成 `rust_decimal::Decimal`。
    ///
    /// 未缩放整数与标度都原样交给 `rust_decimal`，由它判断能不能放下：尾数超过 96 位
    /// （例如 `DECIMAL(38, 0)` 里的大值）或标度大于 28 时返回错误。
    ///
    /// ```rust
    /// # use duckfn::{DuckDecimal, DuckResult};
    /// # use rust_decimal::Decimal;
    /// # fn demo(value: DuckDecimal<18, 3>) -> DuckResult<Decimal> {
    /// value.to_decimal()
    /// # }
    /// ```
    ///
    /// Converts to a `rust_decimal::Decimal`. The unscaled value and the scale are handed over
    /// as-is and `rust_decimal` decides whether they fit: a mantissa beyond 96 bits (a large value
    /// in `DECIMAL(38, 0)`, say) or a scale above 28 is an error.
    pub fn to_decimal(self) -> DuckResult<Decimal> {
        Decimal::try_from_i128_with_scale(self.unscaled, u32::from(SCALE)).map_err(|error| {
            duck_error(format!(
                "duckfn: DECIMAL({WIDTH}, {SCALE}) value {} cannot be represented by \
                 rust_decimal ({error}); its mantissa is 96 bits (about 28 significant digits) and \
                 its scale is at most {}",
                self.unscaled,
                Decimal::MAX_SCALE,
            ))
        })
    }

    /// 由 `rust_decimal::Decimal` 构造。
    ///
    /// 先把标度调整到 `SCALE`（变细则补零，变粗则**要求整除**，否则会丢位、直接报错），再检查
    /// 结果是否还在 `WIDTH` 位以内。两者任一不成立都返回错误，不做静默舍入。
    ///
    /// Builds one from a `rust_decimal::Decimal`. The scale is first adjusted to `SCALE` (refining
    /// pads with zeros, coarsening **must divide exactly** or the value would lose digits and is
    /// rejected), then the result is checked against `WIDTH` digits. Either failure is an error
    /// rather than a silent rounding.
    pub fn from_decimal(value: Decimal) -> DuckResult<Self> {
        let unscaled = rescale_mantissa(value.mantissa(), value.scale(), u32::from(SCALE))
            .map_err(|reason| {
                duck_error(format!(
                    "duckfn: {value} cannot be stored in DECIMAL({WIDTH}, {SCALE}): {reason}"
                ))
            })?;
        if !fits_width(unscaled, WIDTH) {
            return Err(duck_error(format!(
                "duckfn: {value} does not fit DECIMAL({WIDTH}, {SCALE}), whose unscaled value may \
                 have at most {WIDTH} digits"
            )));
        }
        Ok(Self { unscaled })
    }
}

/// 把「未缩放整数 + 标度」从 `from_scale` 调到 `to_scale`；丢位或溢出时返回原因。
///
/// Rescales "unscaled value + scale" from `from_scale` to `to_scale`, returning the reason when
/// digits would be lost or the result would overflow.
fn rescale_mantissa(
    mantissa: i128,
    from_scale: u32,
    to_scale: u32,
) -> Result<i128, &'static str> {
    if from_scale == to_scale {
        return Ok(mantissa);
    }
    if from_scale < to_scale {
        // 变细则补零，不会丢位，只可能溢出 i128。
        //
        // Refining pads with zeros; no digit is lost, only an i128 overflow is possible.
        let factor = 10i128
            .checked_pow(to_scale - from_scale)
            .ok_or("the scale difference is too large for i128")?;
        mantissa
            .checked_mul(factor)
            .ok_or("adjusting the scale would overflow i128")
    } else {
        // 变粗则只有整除才不丢位（负数取余为 0 同样表示整除）。
        //
        // Coarsening keeps every digit only when the division is exact (for a negative value a
        // zero remainder still means divisible).
        let factor = 10i128
            .checked_pow(from_scale - to_scale)
            .ok_or("the scale difference is too large for i128")?;
        if mantissa % factor != 0 {
            return Err("adjusting the scale would drop digits");
        }
        Ok(mantissa / factor)
    }
}

/// 判断未缩放整数是否能用 `width` 位十进制数字表示。
///
/// Whether an unscaled value fits into `width` decimal digits.
fn fits_width(unscaled: i128, width: u8) -> bool {
    match 10u128.checked_pow(u32::from(width)) {
        Some(bound) => unscaled.unsigned_abs() < bound,
        // `10^WIDTH` 超出 u128（`WIDTH >= 39`）：任何 `i128` 都装得下。DuckDB 本身要求
        // `WIDTH < 39`，这种取值在注册阶段就会失败，这里不必再拦。
        //
        // `10^WIDTH` overflows u128 (`WIDTH >= 39`), which means every `i128` fits. DuckDB itself
        // requires `WIDTH < 39`, so such a type already fails at registration time.
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn round_trip_keeps_the_value() {
        let decimal = Decimal::from_str("1234.567").unwrap();
        let duck = DuckDecimal::<18, 3>::from_decimal(decimal).unwrap();
        assert_eq!(duck.unscaled, 1_234_567);
        assert_eq!(duck.to_decimal().unwrap(), decimal);

        // 负值同样保持符号。
        //
        // Negative values keep their sign too.
        let negative = Decimal::from_str("-0.001").unwrap();
        let duck = DuckDecimal::<18, 3>::from_decimal(negative).unwrap();
        assert_eq!(duck.unscaled, -1);
        assert_eq!(duck.to_decimal().unwrap(), negative);
    }

    #[test]
    fn refining_the_scale_pads_with_zeros() {
        let decimal = Decimal::from_str("1.5").unwrap();
        let duck = DuckDecimal::<10, 4>::from_decimal(decimal).unwrap();
        assert_eq!(duck.unscaled, 15_000);
        assert_eq!(duck.to_decimal().unwrap(), decimal);
    }

    #[test]
    fn coarsening_must_be_exact() {
        let exact = Decimal::from_str("1.2500").unwrap();
        assert_eq!(
            DuckDecimal::<10, 2>::from_decimal(exact).unwrap().unscaled,
            125
        );

        let lossy = Decimal::from_str("1.2345").unwrap();
        let err = DuckDecimal::<10, 2>::from_decimal(lossy).unwrap_err();
        assert!(err.to_string().contains("drop digits"), "{err}");
    }

    #[test]
    fn too_many_digits_for_the_width_is_an_error() {
        let decimal = Decimal::from_str("1234.5").unwrap();
        let err = DuckDecimal::<4, 1>::from_decimal(decimal).unwrap_err();
        assert!(err.to_string().contains("at most 4 digits"), "{err}");

        // 恰好 WIDTH 位是允许的（DECIMAL(4, 1) 装得下 123.4）。
        //
        // Exactly WIDTH digits is allowed: DECIMAL(4, 1) holds 123.4.
        let fits = Decimal::from_str("123.4").unwrap();
        assert_eq!(
            DuckDecimal::<4, 1>::from_decimal(fits).unwrap().unscaled,
            1234
        );
    }

    #[test]
    fn wide_decimals_do_not_fit_rust_decimal() {
        // 2^96 - 1 是 rust_decimal 的尾数上限，这里加 1 就溢出了。
        //
        // 2^96 - 1 is rust_decimal's mantissa limit; one more overflows it.
        let too_wide = DuckDecimal::<38, 0> {
            unscaled: 79_228_162_514_264_337_593_543_950_336,
        };
        let err = too_wide.to_decimal().unwrap_err();
        assert!(err.to_string().contains("rust_decimal"), "{err}");

        // 而 38 位里装得下的值照常转换，只是必须自己在 i128 里算清楚。
        //
        // Values that do fit inside those 38 digits convert normally; the i128 arithmetic is just
        // left to the caller.
        let fits = DuckDecimal::<38, 0> {
            unscaled: 79_228_162_514_264_337_593_543_950_335,
        };
        assert_eq!(
            fits.to_decimal().unwrap(),
            Decimal::from_str("79228162514264337593543950335").unwrap()
        );
    }

    #[test]
    fn scale_above_rust_decimals_limit_is_an_error() {
        // rust_decimal 的标度上限是 28，DuckDB 允许到 WIDTH（最大 38）。
        //
        // rust_decimal's scale limit is 28 while DuckDB allows up to WIDTH (38 at most).
        let duck = DuckDecimal::<38, 30> { unscaled: 1 };
        let err = duck.to_decimal().unwrap_err();
        assert!(err.to_string().contains("scale"), "{err}");
    }

    #[test]
    fn rescaling_past_i128_is_an_error_not_a_panic() {
        // rust_decimal 的最大尾数（2^96 - 1，29 位）配标度 1；补到标度 38 要乘 10^37，
        // 远远超出 i128（约 1.7e38）。
        //
        // rust_decimal's largest mantissa (2^96 - 1, 29 digits) at scale 1: padding it to scale 38
        // multiplies it by 10^37, far beyond i128 (about 1.7e38).
        let decimal = Decimal::from_str("7922816251426433759354395033.5").unwrap();
        assert_eq!(decimal.mantissa(), 79_228_162_514_264_337_593_543_950_335);
        let err = DuckDecimal::<38, 38>::from_decimal(decimal).unwrap_err();
        assert!(err.to_string().contains("overflow"), "{err}");
    }
}
