// 示例代码：部分函数/结构体仅用于演示，并未全部注册或调用。
#![allow(dead_code)]

use duckfn::{
    DuckLazy, DuckLazySlot, DuckOptionResult, DuckResult, DuckStruct, DuckValueReader, DuckValueType,
    DuckValueWriter, duck_aggregate_function, duck_scalar_function,
};
use quack_rs::prelude::{LogicalType, TypeId, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// 复杂配置项：解析代价远大于被聚合的值
//
// `DuckLazy<T>` 要解决的场景就是它 —— 聚合函数的配置参数多行不变，却比被聚合的值复杂十多倍。
// 把配置参数写成 `DuckLazy<LazyDemoConfig>` 后，每行只付 O(1) 的凭证构造成本，真正的解析由
// 用户在首行 `get()` 一次、把结果存进聚合状态，后续所有行复用。
//
// 状态里承载解析结果的就是内置的 `DuckLazySlot`：`resolve()` 解析一次并缓存、`combine()` 在并行
// 聚合时把结果搬过来、`get()` 在 `result()` 里取值 —— 「每行读、只解析一次」这条主要路径不用再
// 每个扩展各写一遍三态判断。
//
// 这个类型本身刻意实现成「自带解析计数」的 `DuckValueType`（内部委托给 `#[derive(DuckStruct)]`
// 的 inner），这样 .test 能断言「lazy 组解析 1 次 / eager 组解析 N 次」。
// ============================================================================

/// 配置解析次数（全局计数，.test 用差值断言）。
///
/// 配置 parse count (a global counter; the .test asserts on the delta).
static CONFIG_PARSE_COUNT: AtomicUsize = AtomicUsize::new(0);

/// 配置项的内部形态：多字段 STRUCT（含 LIST），故意做得比 `i64` 复杂得多。
///
/// The inner shape of the configuration: a multi-field STRUCT (including a LIST), deliberately
/// far more complex than the `i64` being aggregated.
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct LazyDemoConfigInner {
    /// 缩放系数。
    pub scale: f64,
    /// 偏移量。
    pub offset: i64,
    /// 逐行权重（元素可空）。
    pub weights: Vec<Option<i64>>,
    /// 标签，纯粹用来把「解析代价」做大。
    pub labels: Vec<String>,
}

/// 包一层只为统计解析次数：`read_valid` 一被调用就 +1。
///
/// A thin wrapper whose only job is counting parses: every `read_valid` call bumps the counter.
///
/// `DuckLazy<LazyDemoConfig>` 不会触发它（这正是收益所在）；只有当用户调用
/// [`DuckLazy::get`] 时，才会经由 `T::read` 真正走到这里。
///
/// `DuckLazy<LazyDemoConfig>` does not trigger it (that is the whole point); only an explicit
/// [`DuckLazy::get`] goes through `T::read` and ends up here.
#[derive(Clone, Debug, Default)]
pub struct LazyDemoConfig(pub LazyDemoConfigInner);

impl DuckValueType for LazyDemoConfig {
    fn type_id() -> TypeId {
        LazyDemoConfigInner::type_id()
    }

    fn logical_type() -> LogicalType {
        LazyDemoConfigInner::logical_type()
    }

    fn create_reader_from_vector(
        vector: libduckdb_sys::duckdb_vector,
        size: usize,
    ) -> DuckValueReader {
        LazyDemoConfigInner::create_reader_from_vector(vector, size)
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        CONFIG_PARSE_COUNT.fetch_add(1, Ordering::Relaxed);
        LazyDemoConfigInner::read_valid(reader, row).map(Self)
    }

    fn create_writer_batch(
        vector: libduckdb_sys::duckdb_vector,
        output_vec: &[Option<&Self>],
    ) -> DuckValueWriter {
        let inner: Vec<Option<&LazyDemoConfigInner>> = output_vec
            .iter()
            .map(|config| config.as_ref().map(|config| &config.0))
            .collect();
        LazyDemoConfigInner::create_writer_batch(vector, &inner)
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        LazyDemoConfigInner::write_valid(writer, idx, &vo.0);
    }

    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        LazyDemoConfigInner::write_null(writer, idx);
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        LazyDemoConfigInner::write_finish(writer);
    }

    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self(LazyDemoConfigInner::read_by_duck_value_valid_simple(value))
    }
}

/// 配置查询入口（`dummy` 只是为了让签名非空，仓库里没有零参标量函数的先例）。
///
/// The counter query entry point (`dummy` only exists because the repo has no zero-argument scalar
/// function precedent).
///
/// ```sql
/// SELECT dfn_lazy_config_parse_count(0);
/// ```
#[duck_scalar_function]
fn dfn_lazy_config_parse_count(dummy: i64) -> i64 {
    let _ = dummy;
    CONFIG_PARSE_COUNT.load(Ordering::Relaxed) as i64
}

// ============================================================================
// 聚合输出：行数 + 加权和 + 配置里的 scale（证明配置真的被用到了）
// ============================================================================

#[derive(Clone, Debug, Default, DuckStruct)]
pub struct LazyConfigAggOutput {
    /// 处理了多少行（跨 chunk 累加，用它证明所有行都过了）。
    pub rows: i64,
    /// 加权和。
    pub weighted_sum: f64,
    /// 配置里的缩放系数（没解析到配置时为 0）。
    pub scale: f64,
}

/// lazy 组的状态：**缓存解析结果**，而不是缓存凭证 —— 承载它的就是 `DuckLazySlot`。
///
/// The lazy group's state: it caches the *parsed value* (through a `DuckLazySlot`), never the token.
#[derive(Default, Debug, Clone)]
pub struct LazyConfigAggState {
    config: DuckLazySlot<LazyDemoConfig>,
    rows: i64,
    weighted_sum: f64,
}

impl duckfn::DuckAggregateState for LazyConfigAggState {
    type Output = LazyConfigAggOutput;

    fn simple_combine(&mut self, other: &Self) {
        // 配置项不必重新解析：两边解析出来的内容一样，直接把另一边的结果搬过来（O(1) 的引用计数复制）。
        //
        // No re-parse is needed: both sides parsed the same configuration, so move the result over
        // (an O(1) refcount bump).
        self.config.combine(&other.config);
        self.rows += other.rows;
        self.weighted_sum += other.weighted_sum;
    }

    fn simple_result(&self) -> Self::Output {
        LazyConfigAggOutput {
            rows: self.rows,
            weighted_sum: self.weighted_sum,
            scale: self.config.get().map_or(0.0, |c| c.0.scale),
        }
    }
}

/// 聚合函数（lazy 版）：配置项参数写成 `DuckLazy<LazyDemoConfig>`，只在第一行解析一次。
///
/// ```sql
/// SELECT rows, weighted_sum, scale FROM dfn_agg_lazy_config_demo(
///     {'scale': 2.0, 'offset': 1, 'weights': [1, 2], 'labels': ['a', 'b']}, v)
/// FROM range(10) t(v);
/// ```
#[duck_aggregate_function]
fn dfn_agg_lazy_config_demo(
    cfg: DuckLazy<LazyDemoConfig>,
    v: i64,
    state: &mut LazyConfigAggState,
) -> DuckResult<()> {
    // 第一行：解析一次；后续行只付 O(1) 的凭证构造成本（凭证本身不被消费）。
    //
    // First row: parse once. Later rows only pay the O(1) token construction and never consume it.
    let config = state.config.resolve(&cfg)?;

    let weight = config
        .0
        .weights
        .get(state.rows as usize % config.0.weights.len().max(1))
        .copied()
        .flatten()
        .unwrap_or(1);
    state.weighted_sum += (v as f64 + config.0.offset as f64) * config.0.scale * weight as f64;
    state.rows += 1;
    Ok(())
}

/// 聚合函数（lazy 版 · 可空参数）：参数写成 `Option<DuckLazy<LazyDemoConfig>>`，
/// 于是 `NULL` 行也会进回调，由 `resolve_optional` 把「这一行没有配置」变成 `None`。
///
/// - 配置还没出现过：`weighted_sum` 按默认配置（scale 0 / offset 0 / 权重 1）累加；
/// - 一旦某一行给了真值，槽里就留下解析结果，之后的 `NULL` 行也继续用它。
///
/// The nullable flavour: the argument is `Option<DuckLazy<LazyDemoConfig>>`, so `NULL` rows reach
/// the callback too and `resolve_optional` turns "no configuration on this row" into `None`.
///
/// ```sql
/// SELECT rows, scale FROM dfn_agg_lazy_config_demo_n(
///     CASE WHEN v = 0 THEN {'scale': 2.0, 'offset': 1, 'weights': [1, 2], 'labels': []} END, v)
/// FROM range(3) t(v);
/// ```
#[duck_aggregate_function]
fn dfn_agg_lazy_config_demo_n(
    cfg: Option<DuckLazy<LazyDemoConfig>>,
    v: i64,
    state: &mut LazyConfigAggState,
) -> DuckResult<()> {
    let config = state.config.resolve_optional(cfg.as_ref())?;

    let weight = config
        .as_ref()
        .and_then(|config| {
            config
                .0
                .weights
                .get(state.rows as usize % config.0.weights.len().max(1))
                .copied()
                .flatten()
        })
        .unwrap_or(1);
    let scale = config.as_ref().map_or(0.0, |config| config.0.scale);
    let offset = config.as_ref().map_or(0, |config| config.0.offset);
    state.weighted_sum += (v as f64 + offset as f64) * scale * weight as f64;
    state.rows += 1;
    Ok(())
}

// ============================================================================
// 对照组（eager）：同样的逻辑，但入参是 eager 类型 —— 每行都会完整解析一次配置
// ============================================================================

#[derive(Default, Debug, Clone)]
pub struct EagerConfigAggState {
    rows: i64,
    weighted_sum: f64,
    scale: f64,
}

impl duckfn::DuckAggregateState for EagerConfigAggState {
    type Output = LazyConfigAggOutput;

    fn simple_combine(&mut self, other: &Self) {
        self.rows += other.rows;
        self.weighted_sum += other.weighted_sum;
        if self.scale == 0.0 {
            self.scale = other.scale;
        }
    }

    fn simple_result(&self) -> Self::Output {
        LazyConfigAggOutput {
            rows: self.rows,
            weighted_sum: self.weighted_sum,
            scale: self.scale,
        }
    }
}

/// 聚合函数（eager 版）：入参写 `LazyDemoConfig`，于是每行都要完整解析一次配置。
///
/// The eager counterpart: the argument is written as `LazyDemoConfig`, so every row parses the
/// configuration in full. Its only purpose is to show that the lazy group's assertion is not
/// vacuous.
///
/// ```sql
/// SELECT rows, weighted_sum, scale FROM dfn_agg_eager_config_demo(
///     {'scale': 2.0, 'offset': 1, 'weights': [1, 2], 'labels': ['a', 'b']}, v)
/// FROM range(10) t(v);
/// ```
#[duck_aggregate_function]
fn dfn_agg_eager_config_demo(
    cfg: LazyDemoConfig,
    v: i64,
    state: &mut EagerConfigAggState,
) -> DuckResult<()> {
    let weight = cfg
        .0
        .weights
        .get(state.rows as usize % cfg.0.weights.len().max(1))
        .copied()
        .flatten()
        .unwrap_or(1);
    state.weighted_sum += (v as f64 + cfg.0.offset as f64) * cfg.0.scale * weight as f64;
    state.rows += 1;
    state.scale = cfg.0.scale;
    Ok(())
}

// ============================================================================
// 反面用例：把凭证留在状态里，等到 finalize 才消费
//
// 那时产生凭证的回调早就返回、源 reader 也早就释放了。守卫会给出明确的查询报错
// （"DuckLazy<T> is stale: ..."），而不是解引用一块失效的向量。
//
// 这类错误用 `DuckLazySlot` 根本写不出来：它的接口只接受 `&DuckLazy<T>`，凭证进不了状态。
// ============================================================================

#[derive(Default, Debug, Clone)]
pub struct LazyMisuseState {
    config: Option<DuckLazy<LazyDemoConfig>>,
    rows: i64,
}

impl duckfn::DuckAggregateState for LazyMisuseState {
    type Output = f64;

    fn simple_combine(&mut self, other: &Self) {
        // 错误做法之二：连凭证一起合并过去（DuckDB 合并状态后，finalize 拿到的可能是这一份）。
        //
        // A second mistake: the token is merged across states too (DuckDB may finalise this one).
        if self.config.is_none() {
            self.config = other.config.clone();
        }
        self.rows += other.rows;
    }

    fn simple_result(&self) -> Self::Output {
        0.0
    }

    /// 故意在 `c_finalize` 里消费凭证：那时源 chunk 早就释放了，应当报错。
    ///
    /// Deliberately consumes the token inside `c_finalize`: the source chunk is long gone by then,
    /// so this must fail.
    fn result(&self) -> DuckOptionResult<Self::Output> {
        let config = self.config.as_ref().expect("第一行一定存下了凭证");
        Ok(Some(config.get().0.scale))
    }
}

/// 聚合函数（误用版）：凭证跨回调消费 —— 用 `.test` 固化「误用 = 查询报错，而不是 UB」。
///
/// ```sql
/// SELECT dfn_agg_bad_lazy_use({'scale': 2.0, 'offset': 1, 'weights': [1], 'labels': []}, v)
/// FROM range(3) t(v);
/// -- ERROR: DuckLazy<T> is stale: ...
/// ```
#[duck_aggregate_function]
fn dfn_agg_bad_lazy_use(
    cfg: DuckLazy<LazyDemoConfig>,
    v: i64,
    state: &mut LazyMisuseState,
) -> DuckResult<()> {
    let _ = v;
    // 错误做法：只存凭证、不消费它。
    if state.config.is_none() {
        state.config = Some(cfg);
    }
    state.rows += 1;
    Ok(())
}
