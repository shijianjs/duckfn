//! 注册期捕获的那条**自有长连接**：扩展里任何回调都能借它做事。
//!
//! The **owned long-lived connection** captured at registration time: what any callback in the
//! extension can borrow.
//!
//! # 为什么需要它 / Why it exists
//!
//! DuckDB 的 C API 只在部分回调里交出客户端上下文，而**客户端上下文里没有连接** —— 没有任何
//! `duckdb_client_context_get_connection`。聚合函数的 `update` / `finalize` 就既没有 bind 回调，
//! 也没有 `duckdb_aggregate_function_get_client_context`，想去查一句 SQL 或读一个文件都没有入口。
//!
//! DuckDB's C API hands out a client context in some callbacks only, and **a client context carries
//! no connection**: there is no `duckdb_client_context_get_connection`. An aggregate's `update` /
//! `finalize`, for one, has neither a bind callback nor a
//! `duckdb_aggregate_function_get_client_context`, so it has no way in to run a query or read a file.
//!
//! 注册期（扩展加载）是唯一的窗口，但那条连接**不能保存**：quack-rs 的入口点是
//! `access.get_database()` → `duckdb_connect(db)` → 调注册闭包 → `duckdb_disconnect`，注册一返回
//! 连接即失效。所以 [`capture`] 由 [`Connection::as_raw_database`] 打开**自己的一条**连接
//! （[`OwnedConnection`] 持有数据库实例的 `shared_ptr`，能活过扩展加载），存进进程级静态；静态
//! 永不析构，所以它一直有效。
//!
//! Registration time (extension load) is the only window — but that connection **cannot be kept**:
//! quack-rs' entry point runs `access.get_database()` → `duckdb_connect(db)` → your registration
//! closure → `duckdb_disconnect`, so it dies when registration returns. [`capture`] therefore opens
//! **its own** connection with [`Connection::as_raw_database`] ([`OwnedConnection`] holds a
//! `shared_ptr` to the database instance, so it outlives extension loading) and stores it in a
//! process-level static, which is never dropped.
//!
//! # 它撑着什么 / What it backs
//!
//! - [`crate::sql`]（`sql` feature）：跑 SQL、把结果按动态列读回来。**只用 C API 的稳定区**
//!   （`duckdb_connect` + 预处理 / 绑定 / 取块 + 逻辑类型），所以不需要 `duckdb-1-5`，一份
//!   `C_STRUCT` 二进制能在 DuckDB 1.3 – 2.0 之间通用。
//! - [`crate::duck_vfs`]（`owned-connection` feature）：宿主文件系统。它额外需要 1.5 的客户端上下文
//!   与文件系统入口，因此落在不稳定区。
//!
//! - [`crate::sql`] (the `sql` feature): run SQL and read the result back as dynamic columns. It uses
//!   **only the stable region** of the C API (`duckdb_connect` plus prepare / bind / fetch-chunk and
//!   the logical types), so it does not need `duckdb-1-5` and one `C_STRUCT` binary covers DuckDB
//!   1.3 – 2.0.
//! - [`crate::duck_vfs`] (the `owned-connection` feature): the host file system. It additionally needs
//!   1.5's client context and file-system entry points, so it lives in the unstable region.
//!
//! 那条连接与那把锁是**同一份**：两个能力都开着时，一次取用期间另一方必须等 —— 同一线程里嵌套
//! 取用会死锁。
//!
//! The connection and its lock are **one**: with both capabilities on, one take-up makes the other
//! wait, and nesting take-ups on a single thread deadlocks.
//!
//! # 代价与限制 / Cost and limits
//!
//! - **进程级只有一份，以首个完成注册的实例为准**：同一进程里存在多个数据库实例（或多次 `LOAD`）时，
//!   用的是先注册那个实例的连接。需要按实例隔离时，手写适配器并把自持连接挂进附加数据
//!   （`duckfn::DuckExtraInfo` + `quack_rs::query::OwnedConnection`），本模块不参与。
//! - **每次取用都要拿这条连接上的互斥锁**：取用之间串行化；同一线程里**绝不能嵌套取用**，否则
//!   就地死锁。业务代码 panic 会毒化互斥锁，但连接本身仍然有效，所以这里忽略毒化继续用。
//! - **这条连接活到进程结束**：静态持有数据库实例的 `shared_ptr`，进程不退出就不会释放。
//! - **不能用在「同一个线程里同步执行 DuckDB」的运行时里**：注册期就在当前线程上打开连接并重入
//!   引擎，没有别的线程能推进的运行时因此直接死锁，表现为扩展 `LOAD` / 注册卡住不动。已知一处是
//!   DuckDB-Wasm 的 Node blocking 绑定；Node 的 worker 模式、浏览器与原生 CLI 都正常。
//! - **捕获是 best-effort 的**：[`capture`] 失败只记下原因，注册照样成功（用不上这条连接的扩展
//!   不该因此加载不了），之后每次取用都报出记录下来的原因。
//!
//! - **One process-level entry, the first instance to finish registering wins**: with several database
//!   instances in one process (or several `LOAD`s) it is that instance's connection that is used. For
//!   per-instance isolation, hand-write an adapter and keep your own connection in the function's extra
//!   data (`duckfn::DuckExtraInfo` plus `quack_rs::query::OwnedConnection`); this module stays out.
//! - **Every take-up locks the mutex on that connection**: take-ups serialize, and nesting one on a
//!   single thread **deadlocks immediately**. A panic inside the guard poisons the mutex, but the
//!   connection itself stays valid, so poisoning is deliberately ignored rather than failing every
//!   later take-up.
//! - **The connection lives for the life of the process**: the static holds a `shared_ptr` to the
//!   database instance, so it is never released while the process runs.
//! - **It cannot be used in a runtime that executes DuckDB synchronously on one thread**: the
//!   connection is opened, and the engine re-entered, on the current thread during registration, so a
//!   runtime with no other thread to make progress on deadlocks — the extension `LOAD` hangs. The one
//!   known case is DuckDB-Wasm's Node **blocking** bindings; Node's worker mode, the browser and the
//!   native CLI are all fine.
//! - **Capturing is best-effort**: a failed [`capture`] only records the reason and registration
//!   succeeds anyway (an extension that never needs this connection must not fail to load over it);
//!   every later take-up then reports the recorded reason.

use std::sync::{Mutex, MutexGuard, OnceLock};

use quack_rs::connection::Connection;
use quack_rs::error::ExtensionError;
use quack_rs::query::OwnedConnection;

use crate::{DuckResult, duck_error};

/// 注册期打开的自有长连接；进程级唯一，且静态永不析构。
///
/// The owned long-lived connection opened at registration time; one per process, never dropped
/// (statics do not run `Drop`), which is exactly why the handles derived from it stay valid.
static CONNECTION: OnceLock<Mutex<OwnedConnection>> = OnceLock::new();

/// 捕获失败的原因，供后续调用报出可读的错误。
///
/// Why capturing failed, so later calls can report a readable error.
static CAPTURE_ERROR: OnceLock<String> = OnceLock::new();

/// 注册期捕获：打开自有长连接并保存；[`crate::register_all_duckfn`] 会调用它，用户不需要直接调。
///
/// Capture at registration time: opens the owned long-lived connection and stores it.
/// [`crate::register_all_duckfn`] calls this; users never call it directly.
///
/// best-effort：失败只记下原因（后续取用时才会报错），绝不阻断扩展注册 —— 用不上这条连接的扩展
/// 不该因为这一项失败而加载不了。`OnceLock::set` 天然幂等，重复注册（同一进程多个实例）时首个成功
/// 的实例胜出。
///
/// Best-effort: a failure is only recorded (and surfaces when something takes the connection);
/// registration never fails because of it — an extension that never needs it must not fail to load
/// over this. `OnceLock::set` is naturally idempotent, so when several instances register in one
/// process the first one to succeed wins.
pub(crate) fn capture(connection: &Connection) {
    if CONNECTION.get().is_some() {
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
            let _ = CONNECTION.set(Mutex::new(owned));
        }
        Err(error) => {
            let _ = CAPTURE_ERROR.set(error.as_str().to_string());
        }
    }
}

/// 借出那条自有长连接，在闭包里跑完再释放它的锁。
///
/// Runs the closure with the owned long-lived connection, releasing its lock afterwards.
///
/// 这是 [`crate::sql`] 的入口；[`crate::duck_vfs`] 的 guard 也拿同一把锁，所以两者**互斥**：
/// 在一个 `with_file_system` 闭包里跑 SQL 会死锁，反之亦然。
///
/// This is [`crate::sql`]'s entry point; [`crate::duck_vfs`]'s guards take the same lock, so the two
/// are **mutually exclusive**: running SQL inside a `with_file_system` closure deadlocks, and the
/// other way round too.
///
/// # Errors
///
/// 扩展还没注册完（或注册时打开连接失败）时返回可读错误；闭包自身的错误原样传出。
///
/// Returns a readable error when the extension has not finished registering yet (or opening the
/// connection failed); the closure's own error is passed through unchanged.
pub(crate) fn with_connection<R>(
    f: impl FnOnce(&OwnedConnection) -> DuckResult<R>,
) -> DuckResult<R> {
    let connection = take_connection()?;
    f(&connection)
}

/// 取连接锁（guard 自己保住连接，析构时解锁）；未捕获时报可读错误。
///
/// Takes the connection lock (the guard keeps the connection alive and releases the lock when
/// dropped); reports a readable error when nothing was captured.
///
/// # Errors
///
/// 扩展还没注册完，或注册期打开连接失败时返回错误。
///
/// Returns an error when the extension has not finished registering yet, or when opening the
/// connection during registration failed.
pub(crate) fn take_connection() -> DuckResult<MutexGuard<'static, OwnedConnection>> {
    let mutex = CONNECTION.get().ok_or_else(not_ready_error)?;
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
fn not_ready_error() -> ExtensionError {
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
                "duckfn: the registration-time connection is not available (the extension has not \
                 finished registering)",
            )
        },
        |reason| {
            format!(
                "duckfn: the registration-time connection is not available (opening a connection \
                 during registration failed: {reason})"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{CONNECTION, not_ready_message, take_connection, with_connection};

    // 这些测试只覆盖纯 Rust 逻辑：真正的连接需要 DuckDB 运行时（见 `test/sql` 里的 sqllogictest），
    // 单测不去碰 FFI。
    //
    // These tests cover the pure-Rust logic only: a real connection needs the DuckDB runtime (see the
    // sqllogictest suite); unit tests stay away from FFI.

    #[test]
    fn taking_the_connection_before_registration_is_an_error() {
        // 不假设全局捕获状态：无论另一个测试是否记录过捕获失败，错误文案都以这一段开头。
        //
        // No assumption about global capture state: whichever branch `not_ready_error` takes, the
        // message starts like this.
        assert!(CONNECTION.get().is_none(), "no DuckDB runtime in unit tests");
        let error = take_connection().expect_err("nothing was captured yet");
        assert!(error.as_str().contains("not available"), "{error}");
        assert!(with_connection(|_| Ok(())).is_err());
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
