//! `DuckLazySlot<T>`：把 [`DuckLazy<T>`] 解析一次，留在聚合状态里复用。
//!
//! `DuckLazy<T>` 的契约是「凭证只在产生它的那次回调里有效」，所以能跨行、跨 chunk、跨线程活下来的
//! 只能是**解析结果**。而「每行都读、但只真正解析第一行」正是 `DuckLazy` 最主要的使用路径 ——
//! `DuckLazySlot<T>` 把这条路径固化成类型：状态里放一个 `DuckLazySlot<T>` 字段，行处理里
//! [`resolve`](DuckLazySlot::resolve)，`combine` 里 [`combine`](DuckLazySlot::combine)，
//! `result` 里 [`get`](DuckLazySlot::get)。
//!
//! ```rust
//! use duckfn::{DuckLazy, DuckLazySlot, DuckResult};
//!
//! #[derive(Default, Debug, Clone)]
//! struct State {
//!     scale: DuckLazySlot<f64>,
//!     sum: f64,
//! }
//!
//! // 聚合的 update 回调每行调用一次：第一行解析一次，其余行只付 O(1) 的引用计数复制。
//! fn on_row(scale: &DuckLazy<f64>, v: f64, state: &mut State) -> DuckResult<()> {
//!     let scale = state.scale.resolve(scale)?;
//!     state.sum += v * *scale;
//!     Ok(())
//! }
//! ```
//!
//! 槽共有三个状态：
//!
//! | 状态 | 含义 |
//! | --- | --- |
//! | 未解析 | 还没有在任何 `update` 回调里读过，也就是状态刚被 `Default` 创建出来的样子 |
//! | 已解析为 `NULL` | 参数是 `NULL`，只可能由 [`resolve_optional`](DuckLazySlot::resolve_optional) 写入 |
//! | 已解析 | 值放在 `Arc<T>` 里，之后读取与合并都只动引用计数 |
//!
//! # 用法要点
//!
//! - **只在行处理回调里解析**：凭证只在那里有效，`result()` 里既拿不到凭证、也不该再解析。
//! - **合并用 [`combine`](DuckLazySlot::combine)**：并行聚合时 DuckDB 会把多个局部状态并起来，
//!   两边从同一列解析出来的内容必然相同，把对方已解析的值搬过来即可 —— 既不重新解析，也不会碰到
//!   「另一个线程的凭证」这种不成立的东西。
//! - **存 `Arc<T>` 而不是 `T`**：合并一次「比被聚合的值重十多倍」的配置不该深拷一遍。
//! - 参数可空就读 `Option<DuckLazy<T>>` 并调 [`resolve_optional`](DuckLazySlot::resolve_optional)；
//!   非可空参数读 `DuckLazy<T>` 并调 [`resolve`](DuckLazySlot::resolve)，`NULL` 行在读取层就被拦下，
//!   回调根本不会被调用。
//!
//! `DuckLazySlot<T>`: parse a [`DuckLazy<T>`] once and keep it in the aggregate state.
//!
//! A `DuckLazy<T>` token is only valid inside the callback that produced it, so the only thing
//! that can survive across rows, chunks and threads is the *parsed value*. "Read every row, but
//! only really parse the first one" is the main way `DuckLazy` is used, and this type fixes that
//! path into a shape: put a `DuckLazySlot<T>` field in the state, call
//! [`resolve`](DuckLazySlot::resolve) in the row handler, [`combine`](DuckLazySlot::combine) in
//! `simple_combine` and [`get`](DuckLazySlot::get) in `result`.
//!
//! The slot has three states: *unresolved* (nothing has read the column yet, i.e. what `Default`
//! produces), *resolved to `NULL`* (only [`resolve_optional`](DuckLazySlot::resolve_optional)
//! writes this) and *resolved* (the value sits behind an `Arc<T>`, so reading and merging are
//! refcount bumps).
//!
//! Points worth knowing: resolve only inside the row handler (the token is not valid in `result`);
//! merge with [`combine`](DuckLazySlot::combine) instead of re-parsing, since both sides parsed
//! the same column and a `combine` never sees a token to begin with; store `Arc<T>` rather than `T`
//! so merging an expensive configuration is not a deep copy; and pick
//! [`resolve_optional`](DuckLazySlot::resolve_optional) for `Option<DuckLazy<T>>` arguments —
//! a non-nullable argument never reaches the callback on a `NULL` row, because the read layer
//! rejects the whole row first.

use crate::value_types::duck_lazy::DuckLazy;
use crate::value_types::duck_value_type::DuckValueType;
use crate::DuckResult;
use std::fmt;
use std::sync::Arc;

/// 槽的内部状态。
///
/// The slot's internal state.
///
/// 单独拆一个枚举，是为了让 `Default` / `Clone` / `Debug` 都不带上 `T: Default` / `T: Clone` /
/// `T: Debug` 之类的额外约束：状态类型只保证 `T: DuckValueType`，它虽然蕴含 `Clone + Debug`，
/// 却**不**蕴含 `Default`。
///
/// A separate enum keeps `Default` / `Clone` / `Debug` free of extra bounds such as `T: Default`
/// / `T: Clone` / `T: Debug`: `T` is only guaranteed to be a `DuckValueType`, which implies
/// `Clone + Debug` but **not** `Default`.
enum SlotInner<T> {
    /// 还没有在任何 `update` 回调里解析过。
    ///
    /// Not parsed inside an update callback yet.
    Unresolved,
    /// 已解析，且该参数是 `NULL`（只由 [`DuckLazySlot::resolve_optional`] 写入）。
    ///
    /// Parsed, and the argument was `NULL` (written only by [`DuckLazySlot::resolve_optional`]).
    Null,
    /// 已解析：值放在 `Arc` 里，合并与读取都只动引用计数。
    ///
    /// Parsed: the value sits behind an `Arc`, so merging and reading are refcount bumps only.
    Value(Arc<T>),
}

impl<T> Clone for SlotInner<T> {
    /// 复制内部状态：`Value` 只做一次引用计数递增，不深拷 `T`（因此不要求 `T: Clone`）。
    ///
    /// Copies the inner state: `Value` bumps a refcount instead of deep-copying `T` (which is why
    /// `T: Clone` is not required).
    fn clone(&self) -> Self {
        match self {
            SlotInner::Unresolved => SlotInner::Unresolved,
            SlotInner::Null => SlotInner::Null,
            SlotInner::Value(value) => SlotInner::Value(Arc::clone(value)),
        }
    }
}

/// 聚合状态里的「`DuckLazy<T>` 槽」：只解析一次，之后所有行、所有合并都复用它。
///
/// The "`DuckLazy<T>` slot" of an aggregate state: parsed once and reused by every later row and
/// every merge.
///
/// 典型用法（完整示例见 `duckfn-quack/src/extension/demo/lazy_config_demo.rs`）：
///
/// ```ignore
/// #[derive(Default, Debug, Clone)]
/// struct WeightedState {
///     cfg: DuckLazySlot<Config>,
///     sum: f64,
/// }
///
/// #[duck_aggregate_function]
/// fn dfn_agg_weighted(cfg: DuckLazy<Config>, v: i64, state: &mut WeightedState) -> DuckResult<()> {
///     let cfg = state.cfg.resolve(&cfg)?;   // 第一行解析一次，其余行 O(1)
///     state.sum += cfg.weight(v);
///     Ok(())
/// }
///
/// impl DuckAggregateState for WeightedState {
///     type Output = f64;
///
///     fn simple_combine(&mut self, other: &Self) {
///         self.cfg.combine(&other.cfg);     // 只搬运已解析的值，不重新解析
///         self.sum += other.sum;
///     }
///
///     fn simple_result(&self) -> f64 {
///         self.sum
///     }
/// }
/// ```
pub struct DuckLazySlot<T> {
    /// 当前状态。
    ///
    /// The current state.
    inner: SlotInner<T>,
}

impl<T> Default for DuckLazySlot<T> {
    /// 新建一个未解析的槽（聚合状态 `Default` 出来时就是它）。
    ///
    /// A fresh, unresolved slot (which is what a `Default`-constructed aggregate state starts as).
    fn default() -> Self {
        Self {
            inner: SlotInner::Unresolved,
        }
    }
}

impl<T> Clone for DuckLazySlot<T> {
    /// 复制槽本身：已解析的值只做一次引用计数递增（不要求 `T: Clone`，也不深拷）。
    ///
    /// Copies the slot: a resolved value is a refcount bump (`T: Clone` is not required and nothing
    /// is deep-copied).
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> fmt::Debug for DuckLazySlot<T> {
    /// 只打印状态，不把配置 / 列表整个倒出来（解析结果可能很大）。
    ///
    /// Prints the state only: the parsed value may be large and is never dumped.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match self.inner {
            SlotInner::Unresolved => "Unresolved",
            SlotInner::Null => "NULL",
            SlotInner::Value(_) => "Resolved",
        };
        f.write_str("DuckLazySlot(")?;
        f.write_str(state)?;
        f.write_str(")")
    }
}

impl<T> DuckLazySlot<T> {
    /// 新建一个未解析的槽，等价于 `Default::default()`。
    ///
    /// A new, unresolved slot; equivalent to `Default::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// 槽里是否已经有解析结果 —— 结果可以是值，也可以是 `NULL`。
    ///
    /// Whether the slot already holds a parse result — that result may be a value or `NULL`.
    pub fn is_resolved(&self) -> bool {
        !matches!(self.inner, SlotInner::Unresolved)
    }

    /// 取已解析的值；未解析、或「已解析为 `NULL`」都返回 `None`。
    ///
    /// 返回 `Arc` 而不是引用：调用方常常要在持有它的同时继续修改状态里的其它字段。
    ///
    /// Reads the parsed value; both "unresolved" and "resolved to `NULL`" yield `None`.
    ///
    /// It returns an `Arc` rather than a reference because callers usually want to keep it around
    /// while mutating other fields of the state.
    pub fn get(&self) -> Option<Arc<T>> {
        match &self.inner {
            SlotInner::Value(value) => Some(Arc::clone(value)),
            SlotInner::Unresolved | SlotInner::Null => None,
        }
    }

    /// 把 `other` 已解析的结果搬过来，供聚合的 `simple_combine` / `combine` 使用。
    ///
    /// 自身已经有解析结果时保持不变 —— 两个状态解析的是同一列，内容必然相同。`other` 未解析时
    /// 自身也不变，因此这个方法**不会**被「没读过参数的那一方」清空。
    ///
    /// Moves `other`'s parse result over; used from an aggregate's `simple_combine` / `combine`.
    ///
    /// It keeps its own result when it already has one — both states parsed the same column, so the
    /// contents are identical. An unresolved `other` leaves it untouched either way, so a side that
    /// never read the argument can never wipe the other.
    pub fn combine(&mut self, other: &Self) {
        if matches!(self.inner, SlotInner::Unresolved) {
            self.inner = other.inner.clone();
        }
    }
}

impl<T: DuckValueType> DuckLazySlot<T> {
    /// 解析参数并缓存：只有第一次真正解析，之后每次都是一次引用计数递增。
    ///
    /// 参数非可空时用这个（`DuckLazy<T>`）：单元格是 `NULL` 的行在读取层就被拦下，回调不会执行。
    /// 若之前被 [`Self::resolve_optional`] 标记成 `NULL`、而这一行有真值，则以真值为准。
    ///
    /// 解析失败时返回原始错误（凭证失效、或 `T` 表达不了该值），并且**不**改变槽的状态，
    /// 所以下一行还会再试一次。
    ///
    /// Parses the argument and caches it: only the first call really parses, every later one is a
    /// refcount bump.
    ///
    /// Use it for a non-nullable argument (`DuckLazy<T>`): rows whose cell is `NULL` are rejected by
    /// the read layer, so the callback never runs for them. If [`Self::resolve_optional`] previously
    /// marked the slot as `NULL` and this row carries a real value, the real value wins.
    ///
    /// A failed parse returns the original error (a stale token, or a value `T` cannot represent)
    /// and leaves the slot untouched, so the next row tries again.
    pub fn resolve(&mut self, lazy: &DuckLazy<T>) -> DuckResult<Arc<T>> {
        if let SlotInner::Value(value) = &self.inner {
            return Ok(Arc::clone(value));
        }
        let value = Arc::new(lazy.try_get()?);
        self.inner = SlotInner::Value(Arc::clone(&value));
        Ok(value)
    }

    /// [`Self::resolve`] 的可空版本：`None` 表示这一行的参数是 `NULL`。
    ///
    /// - 已经解析出值时不会被后来的 `NULL` 覆盖（同一列在各行之间内容一致，先出现的真值说了算）；
    /// - 整列都是 `NULL` 时槽停在「已解析为 `NULL`」，[`Self::get`] 返回 `None`，`combine` 也会把
    ///   这个状态搬过去。
    ///
    /// The nullable flavour of [`Self::resolve`]: `None` means the argument was `NULL` on this row.
    ///
    /// A resolved value is never overwritten by a later `NULL` (the column holds the same content on
    /// every row, so the first real value wins); when the whole column is `NULL` the slot settles on
    /// "resolved to `NULL`", [`Self::get`] returns `None`, and `combine` moves that state over too.
    pub fn resolve_optional(&mut self, lazy: Option<&DuckLazy<T>>) -> DuckResult<Option<Arc<T>>> {
        match lazy {
            Some(lazy) => self.resolve(lazy).map(Some),
            None => {
                if matches!(self.inner, SlotInner::Unresolved) {
                    self.inner = SlotInner::Null;
                }
                Ok(self.get())
            }
        }
    }
}
