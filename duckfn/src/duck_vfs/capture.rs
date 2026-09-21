//! 宿主文件系统（DuckDB 的 VFS）访问的底层半边：注册期捕获、回调期随时取用。
//!
//! The low-level half of host file system (DuckDB's VFS) access: captured at registration time,
//! usable from any callback. The convenience file helpers live in [`crate::duck_vfs`] too.
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
//! [`crate::register_all_duckfn`] 在跑注册项之前调用 [`capture`]，用
//! [`Connection::as_raw_database`] 打开一条**自有长连接**（[`OwnedConnection`] 持有数据库实例的
//! `shared_ptr`，能活过扩展加载）并存进进程级静态变量；静态变量永不析构，所以这条连接与它
//! [ClientData](https://duckdb.org) 里的客户端文件系统一直有效。
//!
//! [`crate::register_all_duckfn`] calls [`capture`] before running the registration items. It opens
//! an **owned, long-lived connection** with [`Connection::as_raw_database`] ([`OwnedConnection`]
//! holds a `shared_ptr` to the database instance, so it outlives extension loading) and stores it in
//! a process-level static. Statics are never dropped, so that connection — and the client file
//! system inside its `ClientData` — stays valid for the rest of the process.
//!
//! 取用时（例如聚合函数的 `update` / `finalize`）从这条连接现取 `ClientContext` → `FileSystem`，
//! 句柄随 guard 一起释放，不会逸出。底层读文件走的是**实例级** VFS（`ClientFileSystem` 的
//! `GetFileSystem()` 返回 `config.file_system`），所以 `httpfs` 注册的 `s3://` / `http(s)://`、
//! 内存文件系统都与本地磁盘一样可用。
//!
//! At use time (say an aggregate's `update` / `finalize`) the guard takes a fresh `ClientContext` →
//! `FileSystem` from that connection and releases both when dropped, so no handle escapes. Reads go
//! through the **instance-level** VFS (`ClientFileSystem::GetFileSystem()` returns
//! `config.file_system`), which is why `s3://` / `http(s)://` from `httpfs` and in-memory file
//! systems work exactly like local disk.
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
//! - 需要 DuckDB 1.5.0+ 与本 crate 的 `duckdb-1-5` feature，否则整个模块不存在。
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
//! - Requires DuckDB 1.5.0+ and this crate's `duckdb-1-5` feature; without them the whole module is
//!   absent.

use std::ops::Deref;
use std::sync::{Mutex, MutexGuard, OnceLock};

use quack_rs::client_context::ClientContext;
use quack_rs::connection::Connection;
use quack_rs::file_system::FileSystem;
use quack_rs::query::OwnedConnection;

use crate::{DuckResult, duck_error};

/// 注册期打开的自有长连接；进程级唯一，且静态永不析构。
///
/// The owned long-lived connection opened at registration time; one per process, never dropped
/// (statics do not run `Drop`), which is exactly why the handles derived from it stay valid.
static VFS_CONNECTION: OnceLock<Mutex<OwnedConnection>> = OnceLock::new();

/// 捕获失败的原因，供后续调用报出可读的错误。
///
/// Why capturing failed, so later calls can report a readable error.
static CAPTURE_ERROR: OnceLock<String> = OnceLock::new();

/// 注册期捕获：打开自有长连接并保存；`register_all_duckfn` 会调用它，用户不需要直接调。
///
/// Capture at registration time: opens the owned long-lived connection and stores it.
/// [`crate::register_all_duckfn`] calls this; users never call it directly.
///
/// best-effort：失败只记下原因（后续取用时才会报错），绝不阻断扩展注册 —— 用不上文件系统的扩展
/// 不该因为这一项失败而加载不了。`OnceLock::set` 天然幂等，重复注册（同一进程多个实例）时
/// 首个成功的实例胜出。
///
/// Best-effort: a failure is only recorded (and surfaces when something asks for the file system);
/// registration never fails because of it — an extension that never touches the file system must not
/// fail to load over this. `OnceLock::set` is naturally idempotent, so when several instances
/// register in one process the first one to succeed wins.
pub(crate) fn capture(connection: &Connection) {
    if VFS_CONNECTION.get().is_some() {
        return;
    }
    // SAFETY: `connection` 由注册闭包传入，句柄在闭包期间有效；`as_raw_database` 返回的
    // `duckdb_database` 由 DuckDB 管理与实例同生命周期，正是 `OwnedConnection::open` 要求的输入。
    //
    // SAFETY: `connection` comes from the registration closure, so its handle is valid for the
    // duration of that closure; the `duckdb_database` returned by `as_raw_database` is DuckDB-managed
    // and lives as long as the instance — exactly what `OwnedConnection::open` requires.
    match unsafe { OwnedConnection::open(connection.as_raw_database()) } {
        Ok(owned) => {
            // 已经被另一个实例抢先注册时，`owned` 会随 `Err` 返回并被 drop（即断开连接）。
            //
            // When another instance won the race the value comes back inside `Err` and is dropped
            // right away (which disconnects it).
            let _ = VFS_CONNECTION.set(Mutex::new(owned));
        }
        Err(error) => {
            let _ = CAPTURE_ERROR.set(error.as_str().to_string());
        }
    }
}

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
pub fn file_system() -> DuckResult<DuckFileSystem> {
    let connection = take_connection()?;
    // SAFETY: 连接由静态变量持有且永不析构（见模块文档），因此在 guard 存续期间有效。
    //
    // SAFETY: the connection is owned by a static that is never dropped (see the module docs), so it
    // is valid for as long as the guard lives.
    let context = unsafe { ClientContext::from_connection(connection.as_raw()) }?;
    let fs = FileSystem::from_client_context(&context)
        .ok_or_else(|| duck_error("duckfn: DuckDB did not provide a file system"))?;
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
pub struct DuckFileSystem {
    fs: FileSystem,
    context: ClientContext,
    // 仅为持有而存在（名字带下划线是给 dead_code lint 看的）：它保住连接锁，从而保证
    // 上面两个引用语义的句柄在其存续期间有效。析构顺序即声明顺序。
    //
    // Held for its own sake (the leading underscore is for the dead-code lint): it keeps the
    // connection lock, which is what makes the two reference-semantic handles above valid for as
    // long as they live. Drop order is declaration order.
    _connection: MutexGuard<'static, OwnedConnection>,
}

impl DuckFileSystem {
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
    /// take deliberately.
    pub(crate) fn connection(&self) -> &OwnedConnection {
        &self._connection
    }
}

impl Deref for DuckFileSystem {
    type Target = FileSystem;

    fn deref(&self) -> &Self::Target {
        &self.fs
    }
}

impl std::fmt::Debug for DuckFileSystem {
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

/// 取连接锁；未捕获时给出可读错误。
///
/// Takes the connection lock; reports a readable error when nothing was captured.
fn take_connection() -> DuckResult<MutexGuard<'static, OwnedConnection>> {
    let mutex = VFS_CONNECTION.get().ok_or_else(not_ready_error)?;
    // 业务代码在 guard 里 panic 会毒化互斥锁，但连接本身（以及它引用的数据库实例）依然有效，
    // 所以这里选择忽略毒化继续用，而不是让之后每次取用都失败。
    //
    // A panic inside the guard poisons the mutex, but the connection — and the database instance it
    // references — is still fine, so poisoning is deliberately ignored instead of failing every
    // later take-up.
    Ok(mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// 构造「还没准备好」的错误，区分「注册失败」与「尚未注册」两种情况。
///
/// Builds the "not ready" error, distinguishing "capture failed" from "not registered yet".
fn not_ready_error() -> quack_rs::error::ExtensionError {
    duck_error(not_ready_message(CAPTURE_ERROR.get().map(String::as_str)))
}

/// [`not_ready_error`] 的纯逻辑部分：拿得到捕获失败原因就用它，否则说「尚未注册」。
///
/// The pure part of [`not_ready_error`]: report the captured failure when there is one, otherwise
/// say "not registered yet". Kept free of global state so it can be tested deterministically.
fn not_ready_message(capture_error: Option<&str>) -> String {
    capture_error.map_or_else(
        || {
            String::from(
                "duckfn: the DuckDB file system is not available (the extension has not finished \
                 registering)",
            )
        },
        |reason| {
            format!(
                "duckfn: the DuckDB file system is not available (opening a connection during \
                 registration failed: {reason})"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{VFS_CONNECTION, client_context, file_system, not_ready_message};

    // 这些测试只覆盖纯 Rust 逻辑：真正的句柄需要 DuckDB 运行时（见 `test/sql` 里的
    // sqllogictest），单测不去碰 FFI。
    //
    // These tests cover the pure-Rust logic only: real handles need the DuckDB runtime (see the
    // sqllogictest suite); unit tests stay away from FFI.

    #[test]
    fn taking_the_file_system_before_registration_is_an_error() {
        // 不假设全局捕获状态：无论另一个测试是否记录过捕获失败，错误文案都以这段开头。
        //
        // No assumption about global capture state: whichever branch `not_ready_error` takes, the
        // message starts like this.
        assert!(VFS_CONNECTION.get().is_none(), "no DuckDB runtime in unit tests");
        let error = file_system().expect_err("nothing was captured yet");
        assert!(error.as_str().contains("not available"), "{error}");
        assert!(client_context().is_err());
    }

    #[test]
    fn not_ready_message_separates_not_registered_yet_from_a_capture_failure() {
        let never_registered = not_ready_message(None);
        assert!(
            never_registered.contains("has not finished registering"),
            "{never_registered}"
        );

        let failed = not_ready_message(Some("duckdb_connect failed"));
        assert!(failed.contains("duckdb_connect failed"), "{failed}");
        assert!(failed.contains("registration failed"), "{failed}");
    }
}
