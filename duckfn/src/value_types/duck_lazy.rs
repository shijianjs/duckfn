//! `DuckLazy<T>`：本行内的延迟读取（只读）。
//!
//! `DuckLazy<T>` 在读取时只记录「哪一列、哪一行、行数」这三样东西，真正解析出 `T` 推迟到用户
//! 显式调用 [`DuckLazy::get`] / [`DuckLazy::try_get`] 时。典型场景是聚合函数里「多行不变、
//! 却比被聚合的值复杂十多倍」的配置项参数：
//!
//! ```ignore
//! #[duck_aggregate_function]
//! fn my_agg(cfg: DuckLazy<Config>, v: i64, state: &mut MyState) -> DuckResult<()> {
//!     // 只在第一行解析一次，之后所有行复用解析结果
//!     let cfg = match &state.cfg {
//!         Some(cfg) => cfg,
//!         None => state.cfg.insert(cfg.get()),
//!     };
//!     state.sum += cfg.weight(v);
//!     Ok(())
//! }
//! ```
//!
//! 收益来自「凭证构造是 O(1)」：聚合函数每行都会重建一次参数结构体，eager 参数每行都要完整
//! 解析一遍，而 `DuckLazy<T>` 每行只拷几个字；真正的解析由用户决定只做一次。
//!
//! # 契约（重要）
//!
//! - **只能在本行回调内消费**：凭证指向的是本行所在的向量，它只在产生它的那次回调里有效。
//!   把凭证存进聚合状态、跨 chunk 或跨线程再 `get()` 都是错的；
//! - 这类误用**不会变成未定义行为**：凭证里带着源 reader 的存活令牌（[`ChunkToken`]）与线程 id，
//!   `get()` / `try_get()` 会先校验再解引用 —— 失效时返回错误，`get()` 则 panic，
//!   而 panic 会被适配层的 unwind 包装成**查询报错**；
//! - **只读**：写路径（返回值 / 输出字段）一律 panic，因为输出阶段输入 chunk 可能已经失效；
//! - **bind 参数（`Value` 路径）不支持**：表函数的 bind 值只在 bind 回调里有效，函数体在
//!   bind 返回之后才执行，因此该路径直接报错并给出明确信息。
//!
//! `DuckLazy<T>` and its contract in English: the value is read lazily, only inside the callback
//! that produced it. It only records the column handle plus the row index, so constructing it is
//! O(1) — which is what makes it useful for an aggregate function whose configuration argument is
//! far more expensive to parse than the values it aggregates. The token inside the value keeps a
//! `Weak` to the source reader's liveness token plus the creating thread id, so consuming a stale
//! value reports an error (and `get()` panics, which the adapters turn into a query error) instead
//! of dereferencing a stale vector. Writing is not supported, and the bind/`Value` path is
//! rejected explicitly.

use crate::value_types::duck_value_type::{ChunkToken, DuckValueReader, DuckValueType, DuckValueWriter};
use crate::{DuckResult, duck_error};
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{LogicalType, TypeId, Value};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Weak;
use std::thread::{self, ThreadId};

/// 写路径的错误信息：`DuckLazy<T>` 只读。
///
/// The write-path message: `DuckLazy<T>` is read-only.
const READ_ONLY: &str = "DuckLazy<T> is read-only: it can only be read, never written back";

/// bind/`Value` 路径的错误信息。
///
/// The bind/`Value` path message.
const NO_BIND_PATH: &str =
    "DuckLazy<T> is not supported on the bind/Value path: those values are only valid inside the bind callback";

/// 本行内的延迟读取凭证：逻辑类型与 `T` 完全相同，读取只记录位置，取值时才解析。
///
/// A deferred read of `T` scoped to one row: the logical type equals `T`'s, reading only records
/// the position and the actual parse happens when the value is consumed.
///
/// 用法见模块文档：聚合函数的配置项参数只在第一行 `get()` 一次，其余行只付 O(1) 的
/// 凭证构造成本。注意它**不是**别名 —— `DuckList` / `DuckMap` / `DuckArray` 是别名，
/// `DuckLazy<T>` 是一个独立的新类型，它自己实现 [`DuckValueType`]。
///
/// See the module docs for usage. Note that this is **not** an alias: `DuckList` /
/// `DuckMap` / `DuckArray` are aliases, while `DuckLazy<T>` is a distinct type implementing
/// [`DuckValueType`] itself.
pub struct DuckLazy<T> {
    /// 本行所在列的裸向量（可能是 LIST/STRUCT 的子向量）。
    ///
    /// The raw vector of this row's column (possibly a child vector of a LIST/STRUCT).
    vector: duckdb_vector,
    /// 本行在 `vector` 里的下标。
    ///
    /// This row's index inside `vector`.
    row: usize,
    /// 重建 reader 需要的行数。
    ///
    /// The row count needed to rebuild a reader.
    row_count: usize,
    /// 源 reader 的存活令牌：升级失败即表示凭证已失效。
    ///
    /// The source reader's liveness token: a failed upgrade means the value is stale.
    alive: Weak<ChunkToken>,
    /// 产生凭证的线程，用于拦下跨线程消费。
    ///
    /// The thread that produced the value, used to reject cross-thread consumption.
    thread_id: ThreadId,
    /// 只是标记 `T`，不持有数据。
    ///
    /// Only marks `T`; no data is held.
    _marker: PhantomData<fn() -> T>,
}

impl<T: DuckValueType> DuckLazy<T> {
    /// 解析出 `T`：先校验凭证仍有效，再用记录的向量与行号重建 reader 并读值。
    ///
    /// 凭证已失效（跨回调 / 跨 chunk / 跨线程）或读不出值时 panic —— 适配层的 unwind 包装会把
    /// 它变成一条查询报错。需要优雅处理请用 [`Self::try_get`]。
    ///
    /// Parses `T`: the value first re-checks that it is still valid, then rebuilds a reader from
    /// the recorded vector and row. A stale value (past its callback, past the chunk, or from
    /// another thread) or a value `T` cannot represent panics — the adapter's unwind wrapper turns
    /// that into a query error. Use [`Self::try_get`] to handle it gracefully instead.
    pub fn get(&self) -> T {
        match self.try_get() {
            Ok(value) => value,
            Err(err) => panic!("{}", err.as_str()),
        }
    }

    /// [`Self::get`] 的可失败版本：凭证失效或读不出值时返回 `Err`。
    ///
    /// The fallible flavour of [`Self::get`]: it returns `Err` for a stale value or for a value
    /// `T` cannot represent.
    pub fn try_get(&self) -> DuckResult<T> {
        let reader = self.rebuild_reader()?;
        T::read(&reader, self.row).ok_or_else(|| {
            duck_error(format!(
                "DuckLazy<T> has no value at row {} (T cannot represent NULL); use try_get() to handle it",
                self.row
            ))
        })
    }

    /// 校验凭证有效并重建 reader。
    ///
    /// 顺序很重要：**先校验、后解引用**，否则就退化成了 UB。
    ///
    /// Validates the value and rebuilds the reader. The order matters: validate first, dereference
    /// second — otherwise this would be undefined behaviour again.
    fn rebuild_reader(&self) -> DuckResult<DuckValueReader> {
        if self.thread_id != thread::current().id() {
            return Err(duck_error(format!(
                "DuckLazy<T> can only be consumed on the thread that produced it (row {})",
                self.row
            )));
        }
        if self.alive.upgrade().is_none() {
            return Err(duck_error(format!(
                "DuckLazy<T> is stale: consume it inside the callback that produced it (row {})",
                self.row
            )));
        }
        Ok(T::create_reader_from_vector(self.vector, self.row_count))
    }
}

impl<T> Clone for DuckLazy<T> {
    /// 复制凭证本身（不触发解析）；`T` 不需要 `Clone`。
    ///
    /// Copies the token itself (no parsing is triggered); `T` does not need to be `Clone`.
    fn clone(&self) -> Self {
        Self {
            vector: self.vector,
            row: self.row,
            row_count: self.row_count,
            alive: self.alive.clone(),
            thread_id: self.thread_id,
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for DuckLazy<T> {
    /// 只打印位置与是否仍有效，不触发解析。
    ///
    /// Prints the position and liveness only; no parsing is triggered.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DuckLazy")
            .field("row", &self.row)
            .field("row_count", &self.row_count)
            .field("alive", &(self.alive.upgrade().is_some()))
            .finish()
    }
}

impl<T> Default for DuckLazy<T> {
    /// 「未初始化」凭证：永远失效，`get()` / `try_get()` 会明确报错。
    ///
    /// 宏生成的参数结构体会 `#[derive(Default)]`，所以这个实现是必需的。
    ///
    /// An uninitialised value: it is never valid, so `get()` / `try_get()` report a clear error.
    /// The macro-generated argument struct derives `Default`, which is why this is required.
    fn default() -> Self {
        Self {
            vector: std::ptr::null_mut(),
            row: 0,
            row_count: 0,
            alive: Weak::new(),
            thread_id: thread::current().id(),
            _marker: PhantomData,
        }
    }
}

// 裸向量句柄本身不是 Send/Sync，但 `DuckLazy<T>` 的消费有运行时守卫：`rebuild_reader` 会先确认
// 源 reader 仍然存活、且当前线程就是产生凭证的线程，之后才会解引用。也就是说只有「同线程 +
// 源 chunk 仍在回调内」这一种组合能真正读到向量，而那正是安全的组合。
//
// The raw vector handle is not `Send`/`Sync` by itself, but consuming a `DuckLazy<T>` is guarded at
// runtime: `rebuild_reader` first checks that the source reader is still alive and that the current
// thread is the one that produced the value, and only then dereferences the vector. The only
// combination that can actually read is "same thread + source chunk still inside its callback",
// which is exactly the sound one.
unsafe impl<T> Send for DuckLazy<T> {}
unsafe impl<T> Sync for DuckLazy<T> {}

// 裸指针由 DuckDB FFI 提供，此处直接解引用
#[allow(clippy::not_unsafe_ptr_arg_deref)]
impl<T: DuckValueType> DuckValueType for DuckLazy<T> {
    fn type_id() -> TypeId {
        T::type_id()
    }

    /// 逻辑类型与 `T` 完全相同：`DuckLazy` 只是把「解析时机」推后，不改变 SQL 侧的类型。
    ///
    /// The logical type equals `T`'s exactly: `DuckLazy` only defers *when* the value is parsed, it
    /// does not change the SQL-side type.
    fn logical_type() -> LogicalType {
        T::logical_type()
    }

    /// 只记录位置，不做任何解析；NULL 单元格由 [`DuckValueType::read`] 在外层判掉（返回 `None`），
    /// 因此可空参数写 `Option<DuckLazy<T>>` 即可，语义与其它类型一致。
    ///
    /// Only records the position; nothing is parsed. A NULL cell is filtered out by
    /// [`DuckValueType::read`] (it returns `None`), so a nullable argument is written
    /// `Option<DuckLazy<T>>` — the semantics match every other type.
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self {
            vector: reader.c_duckdb_vector,
            row,
            row_count: reader.vector_reader.row_count(),
            alive: reader.alive_weak(),
            thread_id: thread::current().id(),
            _marker: PhantomData,
        })
    }

    /// 只读：写路径一律 panic。
    ///
    /// Read-only: the write path always panics.
    fn write_valid(_writer: &mut DuckValueWriter, _idx: usize, _vo: &Self) {
        panic!("{READ_ONLY}");
    }

    /// 只读：NULL 行同样 panic（`DuckLazy<T>` 不能作为输出列）。
    ///
    /// Read-only: NULL rows panic as well (`DuckLazy<T>` cannot be an output column).
    fn write_null(_writer: &mut DuckValueWriter, _idx: usize) {
        panic!("{READ_ONLY}");
    }

    /// bind/`Value` 路径显式拒绝：那里的值只在 bind 回调内有效，而函数体在 bind 返回之后才跑。
    ///
    /// The bind/`Value` path is rejected explicitly: those values are only valid inside the bind
    /// callback, while the function body runs after bind has returned.
    fn read_by_duck_value_valid(_value: &Value) -> DuckResult<Self> {
        Err(duck_error(NO_BIND_PATH))
    }

    /// 同 [`Self::read_by_duck_value_valid`]，永远不会被默认路径调用到（已直接拒绝）。
    ///
    /// Same as [`Self::read_by_duck_value_valid`]; the default path never reaches it because that
    /// one is rejected directly.
    fn read_by_duck_value_valid_simple(_value: &Value) -> Self {
        panic!("{NO_BIND_PATH}");
    }
}
