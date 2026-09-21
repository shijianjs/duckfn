// ============================================================================
// 宿主文件系统（DuckDB 的 VFS）：在聚合函数里读文件
//
//   DuckDB 的 C API 只在部分回调里交出客户端上下文：标量函数有 bind / init 回调，
//   表函数有 duckdb_table_function_get_client_context，COPY TO 的四个回调各有一个入口；
//   **聚合函数一个都没有** —— 既没有 bind 回调，也没有
//   duckdb_aggregate_function_get_client_context，所以 update / finalize 里拿不到
//   ClientContext，也就拿不到 FileSystem。
//
//   duckfn 因此在扩展加载时（duckfn::register_all_duckfn）就用注册期那份 database 句柄开了
//   一条自有长连接，之后任何回调都能通过 duckfn::with_file_system / duckfn::file_system 取用
//   宿主文件系统：httpfs 注册的 s3:// / http(s)://、内存文件与本地磁盘走同一条通路，
//   而不是退化成只能看本地磁盘的 std::fs。
//
//   函数体里直接调即可，不需要任何额外注册 —— 宏写的聚合与手写适配器都一样；这条能力与
//   函数种类无关，函数级附加数据（extra_info）也不参与。
//
//   In DuckDB's C API only some callbacks hand out a client context: scalar functions have bind /
//   init, table functions have duckdb_table_function_get_client_context, and each of the four
//   COPY TO callbacks has its own entry point. **Aggregate functions have none** — no bind
//   callback, no duckdb_aggregate_function_get_client_context — so update / finalize can reach
//   neither a ClientContext nor a FileSystem.
//
//   duckfn therefore opens an owned, long-lived connection at extension load time
//   (duckfn::register_all_duckfn) from the registration-time database handle, so any later callback
//   can use the host file system through duckfn::with_file_system / duckfn::file_system: httpfs'
//   s3:// / http(s)://, in-memory files and local disk all go the same way, instead of degrading to
//   std::fs, which only ever sees local disk.
//
//   Just call it from the function body — no extra registration, and it works the same for
//   macro-written aggregates and hand-written adapters. The capability is independent of the
//   function kind, and function-level extra data (extra_info) plays no part.
//
// 前置条件：DuckDB 1.5.0+ 且 duckfn 打开 `duckdb-1-5` feature（本示例 crate 已在 Cargo.toml
// 里打开），否则 `with_file_system` 一类入口不存在。
//
// Requires DuckDB 1.5.0+ and duckfn's `duckdb-1-5` feature (this example crate enables it in
// Cargo.toml); otherwise the entry points such as `with_file_system` do not exist.
// ============================================================================

use std::ffi::CString;

use duckfn::{
    DuckAggregateState, DuckBlob, DuckOptionResult, DuckResult, ErrorData, FileOpenOptions,
    duck_aggregate_function, duck_error, duck_scalar_function,
};

/// 文件字节数求和状态：只累计已读到的大小。
///
/// State for summing file sizes: it accumulates nothing but the sizes read so far.
#[derive(Default, Debug, Clone)]
struct FileSizeState {
    /// 已成功读取的字节数合计。
    ///
    /// Total number of bytes read successfully.
    total: i64,
}

impl DuckAggregateState for FileSizeState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
    }

    fn simple_result(&self) -> Self::Output {
        self.total
    }
}

/// 把组内每个路径对应的文件字节数读出来并求和：演示「聚合函数里用宿主文件系统」。
///
/// Sums the byte sizes of the files named by the group's paths: the aggregate-side demo of using
/// the host file system.
///
/// ```sql
/// SELECT dfn_agg_file_size(path) FROM (VALUES ('a.csv'), ('b.csv')) t(path);
/// ```
///
/// 路径按 VARCHAR 逐行传入，每个文件经 DuckDB 的文件系统打开一次，`FileHandle` 随作用域关闭；
/// 读不到文件时整条查询报错，而不是静默跳过这一行。
///
/// Paths arrive row by row as VARCHAR. Every file is opened once through DuckDB's file system and
/// the `FileHandle` closes with its scope. A file that cannot be read fails the query instead of
/// being skipped silently.
#[duck_aggregate_function]
fn dfn_agg_file_size(path: String, state: &mut FileSizeState) -> DuckResult<()> {
    state.total += file_size(&path)?;
    Ok(())
}

/// 经宿主文件系统取单个文件的字节数。
///
/// Returns one file's size through the host file system.
///
/// 有几点是这个用法自带的：
///
/// - 路径要先转成 `CStr`（DuckDB 的 C API 收 C 字符串），含 NUL 字节的路径直接报错；
/// - `with_file_system` 的 guard 持有那条自有连接上的互斥锁，**别嵌套取用**（会死锁）；
///   要一次读多个文件就把读取放进同一次 `with_file_system`，或改用 `duckfn::file_system()`
///   自己持有 guard；
/// - 多线程聚合时，guard 会串行化对这条连接的访问。这里每个文件只做一次 open + size，
///   持锁时间极短；真正昂贵的读取适合放到 `finalize`（每组一次）并把结果缓存进自己的状态。
///
/// Three things come with this call:
///
/// - the path has to become a `CStr` (DuckDB's C API takes C strings); a path containing a NUL
///   byte is rejected outright;
/// - the `with_file_system` guard holds the mutex on that owned connection, so **never nest it**
///   (that deadlocks); to read several files, do it inside a single `with_file_system`, or hold
///   the guard yourself with `duckfn::file_system()`;
/// - with parallel aggregation the guard serializes access to that connection. Here each file
///   costs one open plus one size call, so the lock is held for a moment; genuinely expensive reads
///   belong in `finalize` (once per group) with the result cached in your own state.
fn file_size(path: &str) -> DuckResult<i64> {
    let c_path = CString::new(path).map_err(|_| {
        duck_error(format!(
            "dfn_agg_file_size: path contains a NUL byte: {path:?}"
        ))
    })?;
    duckfn::with_file_system(|fs| {
        let options = FileOpenOptions::read_only();
        let handle = fs
            .open(&c_path, &options)
            .map_err(|error| read_error(path, error))?;
        let size = handle
            .size()
            .map_err(|error| read_error(path, error))?;
        i64::try_from(size).map_err(|_| {
            duck_error(format!("dfn_agg_file_size: '{path}' is larger than BIGINT"))
        })
    })
}

/// 把文件系统的结构化错误转成查询错误，并带上出错的路径。
///
/// 路径前缀让 sqllogictest 能稳定断言，而不必依赖各平台各自的系统错误文案。
///
/// Turns the file system's structured error into a query error, tagging the path that failed. The
/// prefix keeps sqllogictest assertions stable without depending on each platform's own wording for
/// system errors.
fn read_error(path: &str, error: ErrorData) -> quack_rs::error::ExtensionError {
    duck_error(format!(
        "dfn_agg_file_size: cannot read '{path}': {}",
        error
            .message()
            .unwrap_or_else(|| String::from("unknown file system error"))
    ))
}

// ============================================================================
// 便捷读写（duckfn::file）：Hutool FileUtil 风格
//
//   上面是底层形态：自己 open、自己挑 FileOpenOptions、自己 write_all，句柄和细节都在眼前。
//   日常更常用的是 duckfn::file 这一层：
//
//     dfn_file_write_text(path, text)       覆盖写（旧文件更长也能写对）
//     dfn_file_write_text_new(path, text)   文件已存在就报错
//     dfn_file_append_text(path, text)      追加（不存在则创建）
//     dfn_file_write_bytes(path, blob)      写字节
//     dfn_file_read_text / _bytes / _lines  读文本 / 读字节 / 按行读
//     dfn_file_size / dfn_file_exists       字节数 / 是否存在
//
//   「C API 没有 truncate」「必要时先用 COPY 把旧文件清零」「路径要转 C 字符串」这些细节都在
//   duckfn::file 内部处理，调用方只表达意图（覆盖 / 不许覆盖 / 追加，文本 / 字节）。
//
//   The convenience layer (`duckfn::file`, Hutool `FileUtil` style) sits on top of the raw form
//   above: callers state intent (replace / fail if exists / append, text or bytes) and duckfn::file
//   deals with the rest — the C API's missing truncate, the zeroing COPY, and C-string paths.
//
//   写函数都标了 volatile：DuckDB 不会把常量参数的调用折叠成只执行一次，否则
//   `SELECT dfn_file_write_text('a.txt', 'x')` 可能只在一个分片里执行。
//
//   The writing functions are marked volatile so DuckDB neither caches nor folds constant-argument
//   calls into a single execution.
// ============================================================================

/// 读 UTF-8 文本；非法字节报错（想容忍脏字节用 `dfn_file_read_text_lossy`）。
/// ```sql
/// SELECT dfn_file_read_text('report.html');
/// ```
#[duck_scalar_function]
fn dfn_file_read_text(path: String) -> DuckOptionResult<String> {
    Ok(Some(duckfn::file::read_string(&path)?))
}

/// 读原始字节（BLOB）。
/// ```sql
/// SELECT dfn_file_read_bytes('report.html');
/// ```
#[duck_scalar_function]
fn dfn_file_read_bytes(path: String) -> DuckOptionResult<DuckBlob> {
    Ok(Some(DuckBlob {
        value: duckfn::file::read(&path)?,
    }))
}

/// 读 UTF-8 文本，非法字节换成 `U+FFFD`。
/// ```sql
/// SELECT dfn_file_read_text_lossy('mixed.bin');
/// ```
#[duck_scalar_function]
fn dfn_file_read_text_lossy(path: String) -> DuckOptionResult<String> {
    Ok(Some(duckfn::file::read_string_lossy(&path)?))
}

/// 按行读文本：`\n` 分行、行尾 `\r` 去掉、末尾换行不产生空行。
/// ```sql
/// SELECT dfn_file_read_lines('notes.txt');
/// ```
#[duck_scalar_function]
fn dfn_file_read_lines(path: String) -> DuckOptionResult<Vec<String>> {
    Ok(Some(duckfn::file::read_lines(&path)?))
}

/// 覆盖写 UTF-8 文本，返回写入的字节数。
/// ```sql
/// SELECT dfn_file_write_text('report.html', '<h1>hi</h1>');
/// ```
#[duck_scalar_function(volatile = true)]
fn dfn_file_write_text(path: String, text: String) -> DuckOptionResult<i64> {
    duckfn::file::write_string(&path, &text)?;
    Ok(Some(text.len() as i64))
}

/// 覆盖写字节（BLOB），返回写入的字节数。
/// ```sql
/// SELECT dfn_file_write_bytes('a.bin', '\xFF\xFE'::BLOB);
/// ```
#[duck_scalar_function(volatile = true)]
fn dfn_file_write_bytes(path: String, data: DuckBlob) -> DuckOptionResult<i64> {
    duckfn::file::write(&path, &data.value)?;
    Ok(Some(data.value.len() as i64))
}

/// 只在文件不存在时写文本；已存在则报错且不改动原文件。
/// ```sql
/// SELECT dfn_file_write_text_new('once.txt', 'first');
/// ```
#[duck_scalar_function(volatile = true)]
fn dfn_file_write_text_new(path: String, text: String) -> DuckOptionResult<i64> {
    duckfn::file::write_string_with(&path, &text, duckfn::file::WriteMode::FailIfExists)?;
    Ok(Some(text.len() as i64))
}

/// 追加 UTF-8 文本（不存在则创建），返回本次写入的字节数。
/// ```sql
/// SELECT dfn_file_append_text('log.txt', 'line' || chr(10));
/// ```
#[duck_scalar_function(volatile = true)]
fn dfn_file_append_text(path: String, text: String) -> DuckOptionResult<i64> {
    duckfn::file::append_string(&path, &text)?;
    Ok(Some(text.len() as i64))
}

/// 文件字节数；文件不存在或打不开时报错。
/// ```sql
/// SELECT dfn_file_size('report.html');
/// ```
#[duck_scalar_function]
fn dfn_file_size(path: String) -> DuckOptionResult<i64> {
    Ok(Some(
        i64::try_from(duckfn::file::size(&path)?).unwrap_or(i64::MAX),
    ))
}

/// 文件是否存在（能否只读打开；打不开也算不存在，包括扩展未完成注册时）。
/// ```sql
/// SELECT dfn_file_exists('report.html');
/// ```
#[duck_scalar_function]
fn dfn_file_exists(path: String) -> bool {
    duckfn::file::exists(&path)
}
