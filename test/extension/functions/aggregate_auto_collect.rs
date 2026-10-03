// ============================================================================
// auto_collect：把「先收集整列、再一次性计算」的聚合收进宏里
//
// 这个形态在下游（如 duckfn-statrs）非常常见：状态唯一的职责就是把整列 `push` 进 `Vec`，
// finalize 时把切片交给一个纯函数算一个值。完整聚合功能（手写状态结构体 + `DuckAggregateState`
// + 逐行 `push`）在这里全是样板。`auto_collect = true` 让被标注函数**直接就是收尾函数**：
//
//   - `Vec<T>` 参数 = 要收集的列，宏逐行读 `T` 收进内部状态，finalize 时把 `Vec<T>` 交给函数；
//   - `DuckFirst<T>` 参数 = 逐行不变的标量常量，宏用 `DuckLazy` + `DuckLazySlot` 只解析一次，
//     finalize 时把解析出来的 `T` 交给函数（`DuckFirst` 是恒等别名，函数里拿到的就是 `T`）；
//   - 返回值即聚合结果（`-> T` / `-> Option<T>` / `-> DuckOptionResult<T>`）。
//
// 空值传播与原写法完全一致：某行任一「非 Option」列为 NULL 时整行丢弃，不进收集；想保留
// NULL 就把该列写成 `Vec<Option<T>>`，想让标量常量可空就写 `DuckFirst<Option<T>>`。
//
// Collect-then-compute aggregates. The annotated function *is* the finalize handler: `Vec<T>`
// parameters are columns collected across rows, `DuckFirst<T>` parameters are per-query constants
// resolved once, and the return value is the aggregate result. NULL propagation is unchanged: a NULL
// in any non-`Option` column drops the whole row from the collection.
// ============================================================================

use duckfn::{DuckFirst, DuckOptionResult, DuckStruct, duck_aggregate_function};

/// 一个「比被聚合的值复杂得多」的配置项，用 `DuckFirst<Option<ShiftConfig>>` 传给聚合，
/// 演示整条 lazy 通路只在第一行解析一次。
///
/// A configuration far more complex than the values being aggregated, passed through
/// `DuckFirst<Option<ShiftConfig>>` to show the lazy path parsing it exactly once.
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct ShiftConfig {
    /// 缩放系数。
    pub scale: f64,
    /// 平移量。
    pub shift: f64,
}

/// `dfn_agg_range(values)`：极差（max - min），空组返回 NULL（`Option<f64>` 的可空性由类型表达）。
///
/// ```sql
/// SELECT dfn_agg_range(x) FROM (VALUES (1.0), (5.0), (3.0)) t(x);  -- 4.0
/// SELECT dfn_agg_range(x) FROM range(0) t(x);                      -- NULL（空组）
/// ```
#[duck_aggregate_function(
    auto_collect = true,
    description = "Range (max - min) of a DOUBLE column, NULL when no row is non-NULL",
    comment = "The whole column is collected; an empty group yields NULL via the Option<f64> return",
    example = "SELECT dfn_agg_range(x) FROM (VALUES (1.0), (5.0), (3.0)) t(x)"
)]
fn dfn_agg_range(values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    Some(max - min)
}

/// `dfn_agg_covariance(x, y)`：样本协方差（Bessel 修正），两条列按行配对收集；配对后不足
/// 2 行返回 NULL（`DuckOptionResult<f64>` 既可失败又可为 NULL）。任何一列是 NULL 的行整行不进收集。
///
/// ```sql
/// SELECT dfn_agg_covariance(x, y)
/// FROM (VALUES (0.0, -5.0), (3.0, 4.0), (-2.0, 10.0)) t(x, y);  -- -5.5
/// SELECT dfn_agg_covariance(x, y) FROM (VALUES (1.0, 2.0)) t(x, y);  -- NULL（不足 2 行）
/// ```
#[duck_aggregate_function(
    auto_collect = true,
    description = "Sample covariance of two DOUBLE columns (Bessel-corrected), NULL when fewer than two rows are fully non-NULL",
    comment = "A row with a NULL in either column is skipped entirely, keeping the two columns paired",
    example = "SELECT dfn_agg_covariance(x, y) FROM (VALUES (0.0, -5.0), (3.0, 4.0), (-2.0, 10.0)) t(x, y)"
)]
fn dfn_agg_covariance(x: Vec<f64>, y: Vec<f64>) -> DuckOptionResult<f64> {
    let n = x.len();
    if n < 2 {
        return Ok(None);
    }
    let nf = n as f64;
    let mean_x = x.iter().sum::<f64>() / nf;
    let mean_y = y.iter().sum::<f64>() / nf;
    let cov = x
        .iter()
        .zip(y.iter())
        .map(|(a, b)| (a - mean_x) * (b - mean_y))
        .sum::<f64>()
        / (nf - 1.0);
    Ok(Some(cov))
}

/// `dfn_agg_weighted_mean(values, weights)`：加权平均。两列都收集；权重和为 0 时返回 NULL。
///
/// ```sql
/// SELECT dfn_agg_weighted_mean(v, w)
/// FROM (VALUES (1.0, 1.0), (2.0, 3.0)) t(v, w);  -- (1*1 + 2*3) / 4 = 1.75
/// ```
#[duck_aggregate_function(
    auto_collect = true,
    description = "Weighted mean of a DOUBLE column by a weight column, NULL when the total weight is zero",
    comment = "Both columns are collected row by row; a NULL in either drops the whole row",
    example = "SELECT dfn_agg_weighted_mean(v, w) FROM (VALUES (1.0, 1.0), (2.0, 3.0)) t(v, w)"
)]
fn dfn_agg_weighted_mean(values: Vec<f64>, weights: Vec<f64>) -> Option<f64> {
    let total: f64 = weights.iter().sum();
    if total == 0.0 {
        return None;
    }
    let weighted: f64 = values
        .iter()
        .zip(weights.iter())
        .map(|(v, w)| v * w)
        .sum();
    Some(weighted / total)
}

/// `dfn_agg_scaled(values, factor)`：求和后乘以标量常量 `factor`。`factor` 逐行相同，用
/// `DuckFirst<f64>`（非可空）声明 —— 宏只在第一行解析一次；空组时 `factor` 无从解析，结果为 NULL。
///
/// ```sql
/// SELECT dfn_agg_scaled(x, 2.0) FROM (VALUES (1.0), (2.0), (3.0)) t(x);  -- 12.0
/// SELECT dfn_agg_scaled(x, 2.0) FROM range(0) t(x);                      -- NULL（空组）
/// ```
#[duck_aggregate_function(
    auto_collect = true,
    description = "Sum of a DOUBLE column times a constant factor, NULL when no row is non-NULL",
    comment = "The factor is a per-query constant resolved once via DuckFirst, not collected per row",
    example = "SELECT dfn_agg_scaled(x, 2.0) FROM (VALUES (1.0), (2.0), (3.0)) t(x)"
)]
fn dfn_agg_scaled(values: Vec<f64>, factor: DuckFirst<f64>) -> f64 {
    values.iter().sum::<f64>() * factor
}

/// `dfn_agg_affine(values, cfg)`：对整列做仿射 `v * scale + shift` 后求和。配置项是 STRUCT，
/// 用 `DuckFirst<Option<ShiftConfig>>` 传入并只解析一次；配置缺省（NULL）时取单位变换。
///
/// ```sql
/// SELECT dfn_agg_affine(x, {'scale': 2.0, 'shift': 1.0})
/// FROM (VALUES (1.0), (2.0), (3.0)) t(x);  -- (1*2+1)+(2*2+1)+(3*2+1) = 15
/// ```
#[duck_aggregate_function(
    auto_collect = true,
    description = "Sum of a DOUBLE column after an affine transform (scale, shift) passed as a STRUCT constant",
    comment = "The complex STRUCT constant is parsed once via DuckFirst; a NULL constant means identity",
    example = "SELECT dfn_agg_affine(x, {'scale': 2.0, 'shift': 1.0}) FROM (VALUES (1.0), (2.0), (3.0)) t(x)"
)]
fn dfn_agg_affine(values: Vec<f64>, cfg: DuckFirst<Option<ShiftConfig>>) -> f64 {
    let (scale, shift) = cfg.map(|c| (c.scale, c.shift)).unwrap_or((1.0, 0.0));
    values.iter().map(|v| v * scale + shift).sum()
}

/// `dfn_agg_count_present(values)`：统计非 NULL 值个数。`values` 写成 `Vec<Option<f64>>`，
/// 于是 NULL 值不整行丢弃、而是以 `None` 进入收集，函数自己数。
///
/// ```sql
/// SELECT dfn_agg_count_present(x) FROM (VALUES (1.0), (NULL), (3.0)) t(x);  -- 2
/// ```
#[duck_aggregate_function(
    auto_collect = true,
    description = "Count of non-NULL values in a DOUBLE column, keeping NULLs inside the collected vector",
    comment = "A Vec<Option<f64>> parameter receives NULL values as None instead of dropping the row",
    example = "SELECT dfn_agg_count_present(x) FROM (VALUES (1.0), (NULL), (3.0)) t(x)"
)]
fn dfn_agg_count_present(values: Vec<Option<f64>>) -> i64 {
    values.iter().filter(|v| v.is_some()).count() as i64
}
