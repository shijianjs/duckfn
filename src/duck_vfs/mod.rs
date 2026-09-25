//! 宿主文件系统（DuckDB 的 VFS）访问，以及基于它的便捷文件读写。
//!
//! Host file system (DuckDB's VFS) access, plus convenience file reads and writes on top of it.
//!
//! 这里的所有读写都经 DuckDB 的**虚拟文件系统**，而不是 `std::fs`：本地磁盘、内存文件系统、
//! `httpfs` 的 `s3://` / `http(s)://` 走同一条通路，`wasm32-unknown-emscripten` 下也能落到宿主
//! 真正的文件系统上。
//!
//! Everything here goes through DuckDB's **virtual file system**, not `std::fs`: local disk,
//! in-memory file systems and `httpfs`' `s3://` / `http(s)://` take the same path, and under
//! `wasm32-unknown-emscripten` it reaches the host's real file system.
//!
//! # 两层接口 / Two layers
//!
//! - **底层**：[`with_file_system`] / [`file_system`] / [`client_context`] 现取 DuckDB 的
//!   [`FileSystem`] / [`ClientContext`]（[`DuckFileSystem`] / [`DuckClientContext`] 是持有连接的
//!   guard），自己决定打开选项与读写节奏。
//! - **便捷层**：[`read`] / [`read_string`] / [`read_lines`] / [`write()`] / [`write_string`] /
//!   [`append_string`] / [`size`] / [`exists`] 等一行式接口，调用方只表达意图（覆盖 / 不许覆盖 /
//!   追加，文本 / 字节，见 [`WriteMode`]）。
//!
//! - **Low level**: [`with_file_system`] / [`file_system`] / [`client_context`] take DuckDB's
//!   [`FileSystem`] / [`ClientContext`] as needed ([`DuckFileSystem`] / [`DuckClientContext`] are
//!   guards that keep the connection alive), leaving open options and I/O pacing to the caller.
//! - **Convenience**: [`read`] / [`read_string`] / [`read_lines`] / [`write()`] / [`write_string`] /
//!   [`append_string`] / [`size`] / [`exists`] and friends — callers state intent (replace / fail if
//!   exists / append, text or bytes; see [`WriteMode`]).
//!
//! # 为什么需要「注册期捕获」/ Why registration time matters
//!
//! DuckDB 的 C API 只在部分回调里交出客户端上下文，**聚合函数一个都没有**（没有 bind 回调，也没有
//! `duckdb_aggregate_function_get_client_context`）；而注册期那条连接是借来的，注册一结束就被断开。
//! 所以 duckfn 在扩展加载时（[`crate::register_all_duckfn`]）打开一条**自有长连接**存进进程级静态，
//! 之后任何回调都能从它现取句柄。详见 [`DuckFileSystem`]。
//!
//! DuckDB's C API hands out a client context in some callbacks only, and **aggregate functions have
//! none** (no bind callback, no `duckdb_aggregate_function_get_client_context`); the connection the
//! entry point receives is borrowed and disconnected as soon as registration returns. duckfn
//! therefore opens an **owned, long-lived connection** at extension load
//! ([`crate::register_all_duckfn`]) and stores it in a process-level static, so any later callback
//! can take fresh handles from it. See [`DuckFileSystem`].
//!
//! # 调用方不必关心的事 / What callers do not have to care about
//!
//! - **覆盖一个更长的旧文件**：C API 没有 truncate（只有 「需要时新建」，映射到 `O_TRUNC` /
//!   `CREATE_ALWAYS` 的那个标志只在 C++ 侧）。[`WriteMode::Replace`] 的语义是「文件内容精确等于这次
//!   写进去的字节」，内部必要时先用一条零行 `COPY ... TO` 清零，再写正文。
//! - **路径转 C 字符串**：含 NUL 字节的路径在入口就被拒绝。
//! - 这些实现细节将来会随 DuckDB C API 的完善而整体替换，**对外行为不变**。
//!
//! - **Overwriting a longer file**: the C API has no truncate (only "create if needed"; the flag
//!   that maps to `O_TRUNC` / `CREATE_ALWAYS` exists on the C++ side only). [`WriteMode::Replace`]
//!   means "the file holds exactly the bytes written this time": internally a longer file is zeroed
//!   with a zero-row `COPY ... TO` first, then the contents are written.
//! - **Paths become C strings**: a path containing a NUL byte is rejected at the entry point.
//! - These internals will be swapped out wholesale once DuckDB's C API grows the pieces; the
//!   observable behaviour stays the same.
//!
//! # 前置条件与限制 / Prerequisites and limits
//!
//! - 需要 DuckDB 1.5.0+ 与本 crate 的 `duckdb-1-5` feature。
//! - 进程级只有一份入口，以**首个完成注册的实例**为准；需要按实例隔离时，手写适配器并自持连接。
//! - **没有删除**：C API 既没有 remove 也没有 move，DuckDB 也没有 `remove_file` 函数。
//! - 每次取用都会拿那条自有连接上的互斥锁：**同一线程别嵌套取用**（会死锁），并发取用之间串行。
//!
//! - Requires DuckDB 1.5.0+ and this crate's `duckdb-1-5` feature.
//! - One process-level entry, first instance to finish registering wins; for per-instance isolation,
//!   hand-write an adapter and keep your own connection.
//! - **No delete**: the C API offers neither remove nor move, and DuckDB has no `remove_file`.
//! - Every take-up locks the mutex on that owned connection: **never nest it on one thread** (that
//!   deadlocks), and concurrent take-ups serialize.

mod capture;
mod file;

// 注册期捕获：由 `register_all_duckfn` 调用，不对用户公开。
//
// Registration-time capture: called by `register_all_duckfn`, not part of the public API.
pub(crate) use capture::capture;

pub use capture::{
    DuckClientContext, DuckFileSystem, client_context, file_system, with_file_system,
};
pub use file::{
    WriteMode, append, append_string, exists, read, read_lines, read_string, read_string_lossy,
    size, write, write_string, write_string_with, write_with,
};

// 出现在上面这些签名与用法里的 quack-rs 类型：从这里再导出，下游不必直接依赖 quack-rs。
//
// The quack-rs types appearing in those signatures and usages, re-exported here so downstream code
// does not have to depend on quack-rs directly.
pub use quack_rs::client_context::ClientContext;
pub use quack_rs::error_data::ErrorData;
pub use quack_rs::file_system::{FileFlag, FileHandle, FileOpenOptions, FileSystem};
