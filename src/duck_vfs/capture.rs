//! 宿主文件系统（DuckDB 的 VFS）访问的底层半边：连接已经在手，回调期随时取用文件系统。
//!
//! The low-level half of host file system (DuckDB's VFS) access: the connection is already there, so
//! any callback can take the file system. The convenience file helpers live in [`crate::duck_vfs`]
//! too, and the connection itself — captured at registration time and shared with [`crate::sql`] —
//! lives in [`crate::owned_connection`].
//!
//! # 为什么需要它 / Why this exists
//!
//! DuckDB 的 C API 只在部分回调里给出客户端上下文：标量函数有 `bind`/`init` 回调可以调
//! `duckdb_scalar_function_get_client_context`，表函数有 `duckdb_table_function_get_client_context`，
//! `COPY TO` 的四个回调也各有自己的入口。**聚合函数一个都没有** —— 它既没有 bind 回调，
//! 也没有 `duckdb_aggregate_function_get_client_context`，所以 `update` / `finalize` 里
//! 拿不到 `ClientContext`，也就拿不到 `FileSystem`。
//!
//! DuckDB's C API hands out a client context in *some* callbacks: scalar functions can call
//! `duckdb_scalar_function_get_client_context`, table functions
//! `duckdb_table_function_get_client_context`, and each of the four `COPY TO` callbacks has its own
//! entry point. **Aggregate functions have none of them** — no bind callback, no
//! `duckdb_aggregate_function_get_client_context` — so `update` / `finalize` cannot reach a
//! `ClientContext`, and therefore cannot reach a `FileSystem` either.
//!
//! 注册期（扩展加载）是唯一的窗口，但**不能保存那条连接**：quack-rs 的入口点是
//! `access.get_database()` → `duckdb_connect(db)` → 调注册闭包 → `duckdb_disconnect`，
//! 注册返回后连接即失效；而 `duckdb_client_context` 与 `duckdb_file_system` 在 C API 里都是
//! **引用语义**（分别包着 `ClientContext &` 与 `FileSystem &`，`destroy` 只是 `delete` 包装对象），
//! 活不过它们的来源连接。
//!
//! Registration time (extension load) is the only window — but the connection handed to the entry
//! point **must not be kept**: quack-rs' entry point runs `access.get_database()` →
//! `duckdb_connect(db)` → your registration closure → `duckdb_disconnect`, so that connection dies
//! when registration returns. Both `duckdb_client_context` and `duckdb_file_system` are
//! **reference-semantic** in the C API (they wrap `ClientContext &` and `FileSystem &`
//! respectively; `destroy` merely `delete`s the wrapper), so neither outlives its connection.
//!
//! # 机制 / How it works
//!
//! 那条自有长连接（以及谁在什么时候开它）在 [`crate::owned_connection`] 里：注册期打开、存进
//! 进程级静态、永不析构，[`crate::sql`] 也从它取用。本模块只负责**在它之上再取文件系统**：每次
//! 取用（例如聚合函数的 `update` / `finalize`）现取 `ClientContext` → `FileSystem`，句柄随 guard
//! 一起释放，不会逸出。底层读文件走的是**实例级** VFS（`ClientFileSystem` 的 `GetFileSystem()`
//! 返回 `config.file_system`），所以 `httpfs` 注册的 `s3://` / `http(s)://`、内存文件系统都与本地
//! 磁盘一样可用。
//!
//! The owned long-lived connection — and who opens it when — lives in [`crate::owned_connection`]:
//! opened at registration time, stored in a process-level static, never dropped, and also what
//! [`crate::sql`] takes from. This module only adds **the file system on top of it**: every take-up
//! (say an aggregate's `update` / `finalize`) builds a fresh `ClientContext` → `FileSystem` and
//! releases both when the guard drops, so no handle escapes. Reads go through the **instance-level**
//! VFS (`ClientFileSystem::GetFileSystem()` returns `config.file_system`), which is why `s3://` /
//! `http(s)://` from `httpfs` and in-memory file systems work exactly like local disk.
//!
//! # 用法 / Usage
//!
//! ```ignore
//! use duckfn::{DuckResult, duck_error};
//! use duckfn::duck_vfs::{FileOpenOptions, ErrorData};
//!
//! fn file_len(path: &std::ffi::CStr) -> DuckResult<i64> {
//!     duckfn::duck_vfs::with_file_system(|fs| {
//!         let options = FileOpenOptions::read_only();
//!         let handle = fs.open(path, &options).map_err(file_error)?;
//!         let mut buffer = Vec::new();
//!         handle.read_to_end(&mut buffer).map_err(file_error)?;
//!         Ok(buffer.len() as i64)
//!     })
//! }
//!
//! // `ErrorData` 是 quack-rs 的结构化错误（`message()` 取 DuckDB 给的消息），
//! // 也由 `duckfn::duck_vfs` 再导出。
//! // `ErrorData` is quack-rs' structured error (`message()` returns DuckDB's message), also
//! // re-exported by `duckfn::duck_vfs`.
//! fn file_error(error: ErrorData) -> quack_rs::error::ExtensionError {
//!     duck_error(error.message().unwrap_or_else(|| "file system error".to_string()))
//! }
//! ```
//!
//! 需要自己持有 guard 时用 [`file_system`] / [`client_context`]；两者都实现了 `Deref`，
//! 可以直接调底层方法。
//!
//! Use [`file_system`] / [`client_context`] when you want to hold the guard yourself; both
//! implement `Deref`, so the underlying methods are available directly.
//!
//! # 局限 / Limitations
//!
//! - 进程级只有一份，以**首个完成注册的实例**为准。同一进程里存在多个数据库实例时，
//!   用的是先注册那个实例的 VFS。需要按实例隔离时，走「手写适配器 + 把自己的长连接挂在
//!   附加数据里」那条路（`duckfn::DuckExtraInfo` 配合 `quack_rs::query::OwnedConnection`），
//!   本模块不参与。
//! - 自有连接的 `FileOpener` 与查询连接不是同一个，**连接级** `SET` 不保证等价；实例级配置与
//!   secrets 正常生效。
//! - guard 持有连接上的互斥锁：同一线程里嵌套取用会死锁，多线程取用会串行化。要读多个文件
//!   就把读取本身放进同一次 [`with_file_system`]，或把结果缓存进自己的状态（聚合函数推荐在
//!   `finalize` / 每组首行取一次，不要每行都取）。
//! - **不能用在「同一个线程里同步执行 DuckDB」的运行时里**：注册期就在当前线程上打开连接并重入
//!   引擎，那类运行时（单线程 wasm、DuckDB-Wasm 的 Node blocking 绑定）会直接死锁，表现为扩展
//!   LOAD / 注册卡住不动。Node 的 worker 模式、浏览器与原生 CLI 都不受影响 —— 这也是本 crate
//!   把它单独放在 `owned-connection` feature 后面的原因。
//! - 需要 DuckDB 1.5.0+ 与本 crate 的 `owned-connection` feature（它自己依赖 `duckdb-1-5`），
//!   否则整个模块不存在。
//!
//! - There is exactly one process-level entry, and it is the **first instance to finish
//!   registering** that wins. With several database instances in one process, the VFS of that first
//!   instance is used. For per-instance isolation, hand-write an adapter and keep your own
//!   connection in its extra data (`duckfn::DuckExtraInfo` plus
//!   `quack_rs::query::OwnedConnection`); this module stays out of the way.
//! - The owned connection's `FileOpener` is not the query connection's, so **connection-level**
//!   `SET`s are not guaranteed to be equivalent; instance-level configuration and secrets do apply.
//! - The guard holds a mutex on the connection: nesting it on one thread deadlocks and concurrent
//!   take-ups serialize. Read several files inside a single [`with_file_system`] call, or cache the
//!   result in your own state (aggregates should take it once per group — in `finalize` or on the
//!   group's first row — never per row).
//! - **Must not be used in a runtime that executes DuckDB synchronously on one thread**: the
//!   connection is opened, and the engine re-entered, on the current thread during registration,
//!   which deadlocks such a runtime (single-threaded wasm, DuckDB-Wasm's Node blocking bindings) —
//!   the extension LOAD just hangs. Node's worker mode, the browser and the native CLI are
//!   unaffected, which is why this sits behind the crate's `owned-connection` feature.
//! - Requires DuckDB 1.5.0+ and this crate's `owned-connection` feature (which itself needs
//!   `duckdb-1-5`); without them the whole module is absent.

use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::sync::MutexGuard;

use quack_rs::client_context::ClientContext;
use quack_rs::file_system::FileSystem;
use quack_rs::query::OwnedConnection;

use crate::owned_connection::take_connection;
use crate::{DuckResult, duck_error};

/// 取一份可用的 DuckDB 文件系统，随 guard 一起释放。
///
/// Takes a usable DuckDB file system; it is released together with the returned guard.
///
/// 返回的 [`DuckFileSystem`] 实现 `Deref<Target = FileSystem>`，可以直接 `open()`；
/// guard 释放时会按「文件系统 → 客户端上下文 → 连接锁」的顺序收尾。
///
/// The returned [`DuckFileSystem`] implements `Deref<Target = FileSystem>`, so `open()` is available
/// directly; dropping the guard releases the file system, then the client context, then the lock.
///
/// # Errors
///
/// 扩展还没注册完（或注册时打开连接失败）时返回可读的错误。
///
/// Returns a readable error when the extension has not finished registering yet, or when capturing
/// failed.
pub fn file_system() -> DuckResult<DuckFileSystem<'static>> {
    let connection = take_connection()?;
    // SAFETY: 连接由静态变量持有且永不析构（见模块文档），因此在 guard 存续期间有效。
    //
    // SAFETY: the connection is owned by a static that is never dropped (see the module docs), so it
    // is valid for as long as the guard lives.
    let context = unsafe { ClientContext::from_connection(connection.as_raw()) }?;
    let fs = FileSystem::from_client_context(&context)
        .ok_or_else(|| duck_error("duckfn: DuckDB did not provide a file system"))?;

    // quack-rs 0.18 起 `FileSystem<'ctx>` 借用它的 `ClientContext`，而这里 `context` 要跟 `fs` 一起
    // 放进**同一个结构体**——那是 Rust 表达不了的自引用。绕过办法：`FileSystem` 包装的只是裸
    // 句柄，句柄本身的存活由下面这个 `'static` 的 guard 保证，于是把裸句柄取出来、用 `from_raw`
    // 重新包一层并选定 `'static`（连接由静态变量保活，见模块文档）。
    // `ManuallyDrop` 让这个借用的临时包装在离开作用域时**不析构**（否则会 destroy 掉句柄）；重新
    // 包装出来的 `FileSystem` 持有同一个句柄，析构时恰好释放一次。
    //
    // Since quack-rs 0.18 `FileSystem<'ctx>` borrows its `ClientContext`, but here `context` must be
    // stored *together with* `fs` in one struct — a self-reference Rust cannot express. The way
    // around it: the `FileSystem` wrapper only holds the raw handle, whose validity is guaranteed by
    // the `'static` guard below, so take that handle out and re-wrap it with `from_raw`, choosing
    // `'static` (the connection is kept alive by a static; see the module docs). `ManuallyDrop` stops
    // the borrowed temporary wrapper from destroying the handle when it goes out of scope; the
    // re-wrapped `FileSystem` owns the same handle and releases it exactly once on drop.
    let fs = ManuallyDrop::new(fs);
    // SAFETY: `fs.as_raw()` is the valid, non-null handle DuckDB just handed out, and it is no longer
    // managed by the `ManuallyDrop` wrapper above. The connection it belongs to stays open as long as
    // `_connection` (a `'static` `MutexGuard`) lives, which is exactly the `'static` chosen here.
    //
    // SAFETY: `fs.as_raw()` is the valid, non-null handle DuckDB just handed out, and the
    // `ManuallyDrop` wrapper above no longer manages it. Its connection stays open for as long as
    // `_connection` (a `'static` `MutexGuard`) lives — exactly the `'static` picked here.
    let fs = unsafe { FileSystem::from_raw(fs.as_raw()) };

    Ok(DuckFileSystem {
        fs,
        context,
        _connection: connection,
    })
}

/// 在闭包里用一次文件系统：句柄不逸出，最省心的形态。
///
/// Uses the file system inside a closure: no handle escapes, the least error-prone form.
///
/// 聚合函数推荐在 `finalize`（每组一次）或每组首行调用一次，把结果缓存进自己的状态，
/// 而不是在 `update` 的每一行都取 —— guard 会串行化对这条连接的访问。
///
/// Aggregates should call it once per group — in `finalize`, or on the group's first row — and cache
/// the result in their own state, rather than taking it on every `update` row: the guard serializes
/// access to that connection.
///
/// # Errors
///
/// 同 [`file_system`]；闭包自身返回的错误原样传出。
///
/// Same as [`file_system`]; an error returned by the closure is passed through unchanged.
pub fn with_file_system<R>(f: impl FnOnce(&FileSystem) -> DuckResult<R>) -> DuckResult<R> {
    let guard = file_system()?;
    f(&guard.fs)
}

/// 取一份可用的客户端上下文（连接级配置项、连接 ID）。
///
/// Takes a usable client context (connection-level configuration options, connection ID).
///
/// 注意 `ClientContext::catalog` 需要活跃事务，在空闲连接上会返回 `None`；文件系统请走
/// [`file_system`]。
///
/// Note that `ClientContext::catalog` needs an active transaction and returns `None` on an idle
/// connection; use [`file_system`] for the file system.
///
/// # Errors
///
/// 同 [`file_system`]。
///
/// Same as [`file_system`].
pub fn client_context() -> DuckResult<DuckClientContext> {
    let connection = take_connection()?;
    // SAFETY: 同 `file_system`：连接由静态变量持有且永不析构。
    //
    // SAFETY: as in `file_system` — the connection is owned by a static that is never dropped.
    let context = unsafe { ClientContext::from_connection(connection.as_raw()) }?;
    Ok(DuckClientContext {
        context,
        _connection: connection,
    })
}

/// 文件系统 guard：`Deref` 到 quack-rs 的 [`FileSystem`]，drop 时按声明顺序释放各句柄。
///
/// File-system guard: `Deref`s to quack-rs' [`FileSystem`]; dropping it releases the handles in
/// declaration order.
///
/// 字段顺序即析构顺序（`fs` → `context` → `connection`）：先放开对连接内文件的包装，再删客户端
/// 上下文包装，最后解锁。类型自动是 `!Send + !Sync`（内含两个引用语义的句柄），跨线程误用会
/// 在编译期被挡住。
///
/// Field order *is* drop order (`fs` → `context` → `connection`): release the file-system wrapper
/// first, then the client-context wrapper, then the lock. The type is automatically
/// `!Send + !Sync` (it holds two reference-semantic handles), so misuse across threads is a compile
/// error.
pub struct DuckFileSystem<'ctx> {
    fs: FileSystem<'ctx>,
    context: ClientContext,
    // 仅为持有而存在（名字带下划线是给 dead_code lint 看的）：它保住连接锁，从而保证
    // 上面两个引用语义的句柄在其存续期间有效。析构顺序即声明顺序。
    //
    // Held for its own sake (the leading underscore is for the dead-code lint): it keeps the
    // connection lock, which is what makes the two reference-semantic handles above valid for as
    // long as they live. Drop order is declaration order.
    _connection: MutexGuard<'static, OwnedConnection>,
}

impl DuckFileSystem<'_> {
    /// 返回底层连接在 DuckDB 里的连接 ID，便于日志里区分。
    ///
    /// Returns the connection ID of the underlying connection, handy for logs.
    #[must_use]
    pub fn connection_id(&self) -> u64 {
        self.context.connection_id()
    }

    /// 内部用：那条自有连接（[`crate::duck_vfs`] 的写工具借它跑一条 `COPY` 来清零文件）。
    ///
    /// Internal: the owned connection (the write helpers in [`crate::duck_vfs`] borrow it to zero a
    /// file with `COPY`). Not public: running SQL from a callback is a decision each caller should
    /// take deliberately — [`crate::sql`] is that decision, made once.
    pub(crate) fn connection(&self) -> &OwnedConnection {
        &self._connection
    }
}

impl<'ctx> Deref for DuckFileSystem<'ctx> {
    type Target = FileSystem<'ctx>;

    fn deref(&self) -> &Self::Target {
        &self.fs
    }
}

impl std::fmt::Debug for DuckFileSystem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 底层句柄的 Debug 只打印指针，这里附上连接 ID 更有用。
        //
        // The handles' own Debug impls only print pointers; the connection ID is more useful.
        f.debug_struct("DuckFileSystem")
            .field("connection_id", &self.connection_id())
            .finish_non_exhaustive()
    }
}

/// 客户端上下文 guard：`Deref` 到 quack-rs 的 [`ClientContext`]。
///
/// Client-context guard: `Deref`s to quack-rs' [`ClientContext`].
pub struct DuckClientContext {
    context: ClientContext,
    // 同 `DuckFileSystem`：只为持锁而存在。
    //
    // Same as `DuckFileSystem`: held only to keep the lock.
    _connection: MutexGuard<'static, OwnedConnection>,
}

impl std::fmt::Debug for DuckClientContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuckClientContext")
            .field("connection_id", &self.context.connection_id())
            .finish_non_exhaustive()
    }
}

impl Deref for DuckClientContext {
    type Target = ClientContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::{client_context, file_system};

    // 这些测试只覆盖纯 Rust 逻辑：真正的句柄需要 DuckDB 运行时（见 `test/sql` 里的
    // sqllogictest），单测不去碰 FFI。
    //
    // These tests cover the pure-Rust logic only: real handles need the DuckDB runtime (see the
    // sqllogictest suite); unit tests stay away from FFI.

    #[test]
    fn taking_the_file_system_before_registration_is_an_error() {
        // 不假设全局捕获状态：无论另一个测试是否记录过捕获失败，错误文案都以这一段开头。
        //
        // No assumption about global capture state: whichever branch the "not ready" error takes, the
        // message starts like this.
        let error = file_system().expect_err("nothing was captured yet");
        assert!(error.as_str().contains("not available"), "{error}");
        assert!(client_context().is_err());
    }
}
