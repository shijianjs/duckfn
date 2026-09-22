// ============================================================================
// rust_decimal 桥：DuckDecimal<W, S> <-> rust_decimal::Decimal（duckfn 的 `rust_decimal` feature）
//
//   两边的能力并不对等：DuckDB 的 DECIMAL(W, S) 是「i128 未缩放整数 + 标度」，W 最大 38；
//   rust_decimal 的尾数是 96 位（2^96 - 1，约 28~29 位有效数字），标度上限 28。所以
//   DuckDecimal::to_decimal / from_decimal 都会返回 DuckResult：越界、丢位一律报错，不静默截断。
//
//   下面三个函数覆盖三种情况：常见宽度（18, 3）的双向、用 rust_decimal 算一次再写回、
//   以及 DECIMAL(38, 0) 这种宽类型在越界时报错（上面那句是 `dfn_decimal38_to_text` 的用法）。
//
//   Unlike the time bridge, the two sides are not equally capable: DuckDB's DECIMAL(W, S) is "an
//   unscaled i128 plus a scale" with W up to 38, while rust_decimal's mantissa is 96 bits (2^96 - 1,
//   about 28-29 significant digits) with a scale capped at 28. Both `DuckDecimal::to_decimal` and
//   `from_decimal` therefore return a DuckResult: out-of-range and digit-losing conversions are
//   errors, never silent truncation. The three functions below cover the ordinary width (18, 3) in
//   both directions, an actual computation in rust_decimal written back to DuckDB, and a wide
//   DECIMAL(38, 0) whose out-of-range values error out.
// ============================================================================

use duckfn::{DuckDecimal, DuckOptionResult, duck_scalar_function};
use rust_decimal::Decimal;

/// `DECIMAL(18, 3)` 转成 rust_decimal 的规范文本（标度固定为 3，所以 `0` 渲染成 `0.000`）。
///
/// ```sql
/// SELECT dfn_decimal_to_text(1.234::DECIMAL(18,3));   -- 1.234
/// ```
///
/// Renders `DECIMAL(18, 3)` through rust_decimal. The scale is fixed at 3, so `0` renders as
/// `0.000`.
#[duck_scalar_function]
fn dfn_decimal_to_text(d: DuckDecimal<18, 3>) -> DuckOptionResult<String> {
    Ok(Some(d.to_decimal()?.to_string()))
}

/// 用 rust_decimal 算一次（乘 2），再写回 `DECIMAL(18, 3)`：双向都在，且写回前会校验宽度。
///
/// ```sql
/// SELECT dfn_decimal_double(1.234::DECIMAL(18,3));   -- 2.468
/// ```
///
/// Doubles the value in rust_decimal and writes it back to `DECIMAL(18, 3)`: both directions run,
/// and the write-back validates the width.
#[duck_scalar_function]
fn dfn_decimal_double(d: DuckDecimal<18, 3>) -> DuckOptionResult<DuckDecimal<18, 3>> {
    let doubled = d.to_decimal()? * Decimal::TWO;
    Ok(Some(DuckDecimal::from_decimal(doubled)?))
}

/// `DECIMAL(38, 0)` 这种宽度超过 rust_decimal 尾数的类型：装得下就转，装不下报错。
///
/// ```sql
/// SELECT dfn_decimal38_to_text(123::DECIMAL(38,0));  -- 123
/// -- 79228162514264337593543950336（2^96）会报错：超出 rust_decimal 的 96 位尾数
/// ```
///
/// `DECIMAL(38, 0)`, whose width exceeds rust_decimal's mantissa: values that fit convert, values
/// that do not are an error rather than a truncation.
#[duck_scalar_function]
fn dfn_decimal38_to_text(d: DuckDecimal<38, 0>) -> DuckOptionResult<String> {
    Ok(Some(d.to_decimal()?.to_string()))
}
