//! 宿主文件的便捷读写（Hutool `FileUtil` 风格）：[`crate::duck_vfs`] 的便捷层。
//!
//! Convenience helpers for reading and writing host files (in the spirit of Hutool's `FileUtil`):
//! the convenience layer of [`crate::duck_vfs`].
//!
//! 这里只做四件事：**读字节 / 读字符串 / 写字节 / 写字符串**，外加 `append`、`size`、`exists`；
//! 全部经 DuckDB 的虚拟文件系统（VFS），所以本地磁盘、内存文件系统、`httpfs` 的
//! `s3://` / `http(s)://` 走的是同一条通路，而不是只能看本地磁盘的 `std::fs`。
//!
//! Four things only: **read bytes / read text / write bytes / write text**, plus `append`, `size`
//! and `exists`. Everything goes through DuckDB's virtual file system (VFS), so local disk,
//! in-memory file systems and `httpfs`' `s3://` / `http(s)://` all take the same path — not
//! `std::fs`, which only ever sees local disk.
//!
//! # 用法 / Usage
//!
//! ```ignore
//! use duckfn::duck_vfs::{self, WriteMode};
//!
//! duck_vfs::write_string("report.html", "<h1>hi</h1>")?;          // 覆盖写（默认）
//! duck_vfs::append_string("report.html", "\n<!-- tail -->")?;     // 追加
//! duck_vfs::write_string_with("report.html", "x", WriteMode::FailIfExists)?;  // 已存在就报错
//!
//! let text = duck_vfs::read_string("report.html")?;
//! let bytes = duck_vfs::read("report.html")?;
//! let lines = duck_vfs::read_lines("report.html")?;
//! assert!(duck_vfs::exists("report.html"));
//! # let _ = (text, bytes, lines);
//! ```
//!
//! # 调用方不必关心的细节 / Details callers do not need to care about
//!
//! - **覆盖一个更长的旧文件**：DuckDB 的 C API 没有 truncate（`DUCKDB_FILE_FLAG_CREATE` 只映射到
//!   POSIX `O_CREAT` / Windows `OPEN_ALWAYS`，映射到 `O_TRUNC` / `CREATE_ALWAYS` 的
//!   `FILE_FLAGS_FILE_CREATE_NEW` 只在 C++ 侧有），所以直接写会在尾部留下旧内容的残渣。这里在
//!   「旧文件更长」时才先用一条零行的 `COPY ... TO` 把它清零（`COPY` 走 DuckDB 自己的写路径，
//!   用 `OverwriteExistingFile` 真替换目标），再写正文。**行为是「覆盖」，与未来 C API 支持
//!   truncate 后应当完全一致**，届时内部实现可以整体换掉而不影响调用方。
//! - **路径转 C 字符串**：DuckDB 的 C API 收 `CStr`，含 NUL 字节的路径在这里就被拒绝。
//! - **并发**：每次调用都借 duckfn 注册期留下的那条自有连接（详见
//!   `duckfn::duck_vfs::with_file_system` 的文档），同一时刻只有一条线程经由此连接做 I/O，
//!   读多个文件时不要嵌套调用。
//!
//! - **Overwriting a longer file**: DuckDB's C API has no truncate (`DUCKDB_FILE_FLAG_CREATE` only
//!   maps to POSIX `O_CREAT` / Windows `OPEN_ALWAYS`; the flag that maps to `O_TRUNC` /
//!   `CREATE_ALWAYS` is the C++-side `FILE_FLAGS_FILE_CREATE_NEW`), so a plain write leaves a tail
//!   of the old contents behind. Only when the existing file *is longer* does this module zero it
//!   first with a zero-row `COPY ... TO` (`COPY` takes DuckDB's own write path, which replaces the
//!   target through `OverwriteExistingFile`) and then write the contents. The **semantics are
//!   "overwrite"** and should stay identical once the C API grows truncate — the internals can then
//!   be replaced wholesale without callers noticing.
//! - **Paths become C strings**: DuckDB's C API takes `CStr`, so a path containing a NUL byte is
//!   rejected here.
//! - **Concurrency**: every call borrows the owned connection duckfn keeps from registration time
//!   (see `duckfn::duck_vfs::with_file_system`); one thread at a time goes through it, and nested
//!   take-ups deadlock.
//!
//! # 已知限制 / Known limitations
//!
//! - **没有删除**：C API 既没有 remove 也没有 move，DuckDB 也没有 `remove_file` 这类 SQL 函数，
//!   所以这个模块不提供 `delete`（想「清空」可以覆盖成空内容）。
//! - 只写文件内容，不建目录：父目录不存在时由文件系统报错。
//! - 需要 DuckDB 1.5.0+ 与本 crate 的 `duckdb-1-5` feature。
//!
//! - **No delete**: the C API offers neither remove nor move, and DuckDB has no `remove_file` SQL
//!   function, so there is no `delete` here (overwrite with empty contents to "clear" a file).
//! - It writes file contents only — it does not create directories; a missing parent directory is
//!   reported by the file system.
//! - Requires DuckDB 1.5.0+ and this crate's `duckdb-1-5` feature.

use std::ffi::CString;
use std::str::Utf8Error;

use quack_rs::error::ExtensionError;
use quack_rs::file_system::{FileFlag, FileOpenOptions};

use super::capture::{DuckFileSystem, file_system, with_file_system};
use crate::{DuckResult, duck_error};

/// 写文件时如何对待已存在的文件。
///
/// What to do about an existing file when writing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WriteMode {
    /// 覆盖（默认，同 [`write()`] / [`write_string`]）：旧文件更长也照样写对，结果是「文件内容
    /// 恰好等于这次写进去的字节」。
    ///
    /// Replace (the default, same as [`write()`] / [`write_string`]): even a longer existing file
    /// ends up holding exactly the bytes written this time.
    #[default]
    Replace,
    /// 文件已存在就报错（POSIX `O_EXCL` 语义），不会改动已有内容。
    ///
    /// 存在性由显式检查决定，并额外带上 `EXCLUSIVE_CREATE` 作为并发下的第二道保险：DuckDB 只在
    /// POSIX 本地文件系统上把后者真正落到 `O_EXCL`，Windows 分支不处理它（缺文件时甚至会因为退回
    /// `OPEN_EXISTING` 而报「找不到文件」），所以不能只依赖 flag。
    ///
    /// Fail when the file already exists (POSIX `O_EXCL`), leaving existing contents untouched.
    /// Existence is decided by an explicit check, with `EXCLUSIVE_CREATE` added as a second line of
    /// defence against concurrent creators: DuckDB only turns that flag into a real `O_EXCL` on
    /// POSIX local file systems — the Windows branch ignores it (and even reports "file not found"
    /// for a missing file, because it falls back to `OPEN_EXISTING`) — so the flag alone is not
    /// enough.
    FailIfExists,
    /// 追加到文件末尾；文件不存在则创建。
    ///
    /// Append to the end of the file, creating it when missing.
    Append,
}

/// 零行 `COPY` 语句：把目标文件替换成 0 字节文件。
///
/// 路径以绑定参数传入（`?`），不做 SQL 字符串拼接。
///
/// The zero-row `COPY` statement that replaces the target file with an empty one. The path is
/// bound as a parameter (`?`), never spliced into the SQL text.
const ZERO_FILE_SQL: &str = "COPY (SELECT 1 AS i WHERE false) TO ? (FORMAT csv, HEADER false)";

/// 读取整个文件的字节。
///
/// Reads the whole file as bytes.
///
/// # Errors
///
/// 文件不存在、打不开或读失败时返回带路径的查询错误。
///
/// Returns a query error carrying the path when the file is missing, cannot be opened, or fails to
/// read.
pub fn read(path: &str) -> DuckResult<Vec<u8>> {
    let c_path = path_c_string("read", path)?;
    with_file_system(|file_system| {
        let handle = file_system
            .open(&c_path, &FileOpenOptions::read_only())
            .map_err(|error| file_error("read", path, error))?;
        let mut buffer = Vec::new();
        handle
            .read_to_end(&mut buffer)
            .map_err(|error| file_error("read", path, error))?;
        Ok(buffer)
    })
}

/// 按 UTF-8 读取整个文件。
///
/// Reads the whole file as UTF-8 text.
///
/// # Errors
///
/// 除了 [`read`] 的错误，内容不是合法 UTF-8 时也报错（给出第一个非法字节的位置）；
/// 想容忍脏字节请用 [`read_string_lossy`]，想要原始字节请用 [`read`]。
///
/// Besides [`read`]'s errors, invalid UTF-8 is an error too (reporting the first invalid byte);
/// use [`read_string_lossy`] to tolerate it or [`read`] for raw bytes.
pub fn read_string(path: &str) -> DuckResult<String> {
    let bytes = read(path)?;
    String::from_utf8(bytes).map_err(|error| utf8_error(path, error.utf8_error()))
}

/// 按 UTF-8 读取整个文件，非法字节替换成 `U+FFFD`（同 `String::from_utf8_lossy`）。
///
/// Reads the whole file as UTF-8, replacing invalid bytes with `U+FFFD` (as
/// `String::from_utf8_lossy` does).
///
/// # Errors
///
/// 同 [`read`]。
///
/// Same as [`read`].
pub fn read_string_lossy(path: &str) -> DuckResult<String> {
    Ok(String::from_utf8_lossy(&read(path)?).into_owned())
}

/// 按行读取 UTF-8 文本。
///
/// Reads UTF-8 text line by line.
///
/// 规则：以 `\n` 分行，行尾的 `\r` 去掉（兼容 CRLF），末尾的换行不产生一行空行，
/// 空文件得到空 `Vec`。
///
/// Rules: split on `\n`, strip a trailing `\r` (so CRLF works), a trailing newline does not
/// produce an empty last line, and an empty file yields an empty `Vec`.
///
/// # Errors
///
/// 同 [`read_string`]。
///
/// Same as [`read_string`].
pub fn read_lines(path: &str) -> DuckResult<Vec<String>> {
    Ok(split_lines(&read_string(path)?))
}

/// 把字节写入文件，覆盖已有内容（同 [`WriteMode::Replace`]）。
///
/// Writes bytes to the file, replacing existing contents (same as [`WriteMode::Replace`]).
///
/// # Errors
///
/// 路径非法、打不开文件或写失败时返回带路径的查询错误。
///
/// Returns a query error carrying the path when the path is invalid, the file cannot be opened, or
/// the write fails.
pub fn write(path: &str, bytes: &[u8]) -> DuckResult<()> {
    write_with(path, bytes, WriteMode::Replace)
}

/// 把 UTF-8 文本写入文件，覆盖已有内容（同 [`WriteMode::Replace`]）。
///
/// Writes UTF-8 text to the file, replacing existing contents (same as [`WriteMode::Replace`]).
///
/// # Errors
///
/// 同 [`write()`]。
///
/// Same as [`write()`].
pub fn write_string(path: &str, text: &str) -> DuckResult<()> {
    write(path, text.as_bytes())
}

/// 按给定策略写字节（覆盖 / 已存在就报错 / 追加）。
///
/// Writes bytes according to the given mode (replace / fail if exists / append).
///
/// # Errors
///
/// 路径非法、打不开文件或写失败时返回带路径的查询错误；`WriteMode::FailIfExists` 下文件已存在
/// 也是错误。
///
/// Returns a query error carrying the path when the path is invalid, the file cannot be opened, or
/// the write fails; with `WriteMode::FailIfExists` an existing file is an error too.
pub fn write_with(path: &str, bytes: &[u8], mode: WriteMode) -> DuckResult<()> {
    let c_path = path_c_string("write", path)?;
    let file_system = file_system()?;
    match mode {
        WriteMode::FailIfExists => {
            // 用手里这个 guard 做检查（不能再调 `exists`：那会二次上锁 → 死锁）。
            //
            // Probe through the guard we already hold (calling `exists` here would take the lock a
            // second time and deadlock).
            if file_system
                .open(&c_path, &FileOpenOptions::read_only())
                .is_ok()
            {
                return Err(duck_error(format!(
                    "duckfn::duck_vfs::write: '{path}' already exists and WriteMode::FailIfExists was \
                     requested; delete it or write to another path"
                )));
            }
            let options = FileOpenOptions::new();
            options.set_flag(FileFlag::Write, true);
            options.set_flag(FileFlag::Create, true);
            options.set_flag(FileFlag::CreateNew, true);
            write_through(&file_system, &c_path, path, bytes, &options)
        }
        WriteMode::Append => {
            let options = FileOpenOptions::new();
            options.set_flag(FileFlag::Write, true);
            options.set_flag(FileFlag::Create, true);
            options.set_flag(FileFlag::Append, true);
            write_through(&file_system, &c_path, path, bytes, &options)
        }
        WriteMode::Replace => {
            // 旧文件更长时才需要清零：不长于新内容时，从偏移 0 写 N 字节得到的就是
            // max(旧长度, N) = N 字节，直接写即可。
            //
            // Zeroing is only needed when the existing file is longer: otherwise writing N bytes
            // from offset 0 leaves exactly max(existing, N) = N bytes.
            if existing_len(&file_system, &c_path, path)? > bytes.len() as u64 {
                zero_file(&file_system, path)?;
            }
            write_through(
                &file_system,
                &c_path,
                path,
                bytes,
                &FileOpenOptions::write_create(),
            )
        }
    }
}

/// 按给定策略写 UTF-8 文本。
///
/// Writes UTF-8 text according to the given mode.
///
/// # Errors
///
/// 同 [`write_with`]。
///
/// Same as [`write_with`].
pub fn write_string_with(path: &str, text: &str, mode: WriteMode) -> DuckResult<()> {
    write_with(path, text.as_bytes(), mode)
}

/// 追加字节到文件末尾；文件不存在则创建。
///
/// Appends bytes to the end of the file, creating it when missing.
///
/// # Errors
///
/// 同 [`write()`]。
///
/// Same as [`write()`].
pub fn append(path: &str, bytes: &[u8]) -> DuckResult<()> {
    write_with(path, bytes, WriteMode::Append)
}

/// 追加 UTF-8 文本到文件末尾；文件不存在则创建。
///
/// Appends UTF-8 text to the end of the file, creating it when missing.
///
/// # Errors
///
/// 同 [`write()`]。
///
/// Same as [`write()`].
pub fn append_string(path: &str, text: &str) -> DuckResult<()> {
    write_with(path, text.as_bytes(), WriteMode::Append)
}

/// 文件字节数。
///
/// The size of the file in bytes.
///
/// # Errors
///
/// 文件不存在、打不开或取长度失败时返回带路径的查询错误。
///
/// Returns a query error carrying the path when the file is missing, cannot be opened, or its size
/// cannot be read.
pub fn size(path: &str) -> DuckResult<u64> {
    let c_path = path_c_string("size", path)?;
    with_file_system(|file_system| {
        let handle = file_system
            .open(&c_path, &FileOpenOptions::read_only())
            .map_err(|error| file_error("size", path, error))?;
        handle
            .size()
            .map_err(|error| file_error("size", path, error))
    })
}

/// 文件是否存在（能否以只读方式打开）。
///
/// Whether the file exists (whether it can be opened read-only).
///
/// 打不开就当作不存在 —— 包括路径含 NUL 字节、权限不足、远端不可达，以及扩展还没完成注册
/// （那时文件系统本来就取不到）。需要区分具体原因时请改用 [`size`] 或 [`read`]。
///
/// Anything that cannot be opened counts as absent — a path with a NUL byte, insufficient
/// permissions, an unreachable remote, or the extension not having finished registering (there is
/// no file system yet). Use [`size`] or [`read`] when the reason matters.
#[must_use]
pub fn exists(path: &str) -> bool {
    let Ok(c_path) = path_c_string("exists", path) else {
        return false;
    };
    matches!(
        with_file_system(|file_system| Ok(file_system
            .open(&c_path, &FileOpenOptions::read_only())
            .is_ok())),
        Ok(true)
    )
}

/// 用给定选项打开文件并整段写入。
///
/// Opens the file with the given options and writes `bytes` in one go.
fn write_through(
    file_system: &DuckFileSystem,
    c_path: &CString,
    path: &str,
    bytes: &[u8],
    options: &FileOpenOptions,
) -> DuckResult<()> {
    let handle = file_system
        .open(c_path, options)
        .map_err(|error| file_error("write", path, error))?;
    handle
        .write_all(bytes)
        .map_err(|error| file_error("write", path, error))?;
    // 句柄随作用域关闭（DuckDB 的 close 会落盘），因此不需要显式 close / sync。
    //
    // The handle closes with its scope (DuckDB's close flushes), so no explicit close / sync.
    Ok(())
}

/// 取已有文件的长度；打不开时按 0 处理（真正的失败会在随后打开写模式时暴露）。
///
/// The existing file's length, or 0 when it cannot be opened (a real failure surfaces when the
/// write handle is opened afterwards).
fn existing_len(file_system: &DuckFileSystem, c_path: &CString, path: &str) -> DuckResult<u64> {
    match file_system.open(c_path, &FileOpenOptions::read_only()) {
        Ok(handle) => handle
            .size()
            .map_err(|error| file_error("write", path, error)),
        Err(_) => Ok(0),
    }
}

/// 用内置 `COPY` 把文件清零，绕开 C API 没有 truncate 这件事。
///
/// Zeros the file with the built-in `COPY`, working around the C API having no truncate.
///
/// `COPY ... TO` 走 DuckDB 自己的写路径（`OverwriteExistingFile` → POSIX `O_CREAT|O_TRUNC` /
/// Windows `CREATE_ALWAYS`），并且是「写临时文件再替换目标」，所以清零之后写正文得到的长度就
/// 精确等于正文长度。
///
/// `COPY ... TO` takes DuckDB's own write path (`OverwriteExistingFile` → POSIX `O_CREAT|O_TRUNC`
/// / Windows `CREATE_ALWAYS`) and replaces the target after writing a temporary file, so writing
/// the contents afterwards yields exactly the contents' length.
fn zero_file(file_system: &DuckFileSystem, path: &str) -> DuckResult<()> {
    let explanation = || {
        format!(
            "duckfn::duck_vfs: cannot replace '{path}': the file already exists and is longer than the \
             new contents, and zeroing it with COPY failed"
        )
    };
    let connection = file_system.connection();
    let statement = connection
        .prepare(ZERO_FILE_SQL)
        .map_err(|error| duck_error(format!("{}: {}", explanation(), error.as_str())))?;
    statement
        .bind_str(1, path)
        .map_err(|error| duck_error(format!("{}: {}", explanation(), error.as_str())))?;
    statement
        .execute()
        .map(|_| ())
        .map_err(|error| duck_error(format!("{}: {}", explanation(), error.as_str())))
}

/// 把路径转成 DuckDB 的 C 字符串，顺带挡掉含 NUL 字节的路径。
///
/// Converts the path into DuckDB's C string, rejecting paths that contain a NUL byte.
fn path_c_string(operation: &str, path: &str) -> DuckResult<CString> {
    CString::new(path).map_err(|_| {
        duck_error(format!(
            "duckfn::duck_vfs::{operation}: the path contains a NUL byte: {path:?}"
        ))
    })
}

/// 文件系统返回的结构化错误 → 带「操作 + 路径」的查询错误。
///
/// Turns the file system's structured error into a query error tagged with the operation and path.
fn file_error(operation: &str, path: &str, error: quack_rs::error_data::ErrorData) -> ExtensionError {
    duck_error(format!(
        "duckfn::duck_vfs::{operation}: '{path}': {}",
        error
            .message()
            .unwrap_or_else(|| String::from("unknown file system error"))
    ))
}

/// 非法 UTF-8 → 带位置提示的查询错误。
///
/// Turns invalid UTF-8 into a query error that names the offending byte.
fn utf8_error(path: &str, error: Utf8Error) -> ExtensionError {
    duck_error(format!(
        "duckfn::duck_vfs::read_string: '{path}' is not valid UTF-8 (invalid byte at offset {}); use \
         read_string_lossy to replace it or read for raw bytes",
        error.valid_up_to()
    ))
}

/// [`read_lines`] 的纯逻辑部分：按 `\n` 分行、去掉行尾 `\r`、忽略末尾空行。
///
/// The pure part of [`read_lines`]: split on `\n`, strip a trailing `\r`, drop a trailing empty
/// line.
fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{WriteMode, path_c_string, split_lines, utf8_error};

    // 只测纯逻辑：真正读写的部分要 DuckDB 运行时（见 `test/sql/functions/file_system.test`），
    // 单测不去碰 FFI。
    //
    // Pure logic only: real I/O needs the DuckDB runtime (see
    // `test/sql/functions/file_system.test`), so unit tests stay away from FFI.

    #[test]
    fn write_mode_defaults_to_replace() {
        assert_eq!(WriteMode::default(), WriteMode::Replace);
        assert_ne!(WriteMode::Append, WriteMode::FailIfExists);
    }

    #[test]
    fn path_with_nul_byte_is_rejected() {
        let error = path_c_string("read", "a\0b").expect_err("NUL must be rejected");
        assert!(error.as_str().contains("duckfn::duck_vfs::read"), "{error}");
        assert!(error.as_str().contains("NUL"), "{error}");
        assert!(path_c_string("read", "plain/path.txt").is_ok());
    }

    #[test]
    fn invalid_utf8_error_points_at_the_byte() {
        // 走 Vec 而不是字面量数组：编译器对「一眼可见非法」的字面量会直接报 lint。
        //
        // Go through a Vec rather than a literal array: the compiler lints literals it can see are
        // invalid up front.
        let bytes: Vec<u8> = vec![b'a', 0xFF];
        let error = std::str::from_utf8(&bytes).expect_err("must not be UTF-8");
        let message = utf8_error("x.txt", error);
        assert!(message.as_str().contains("'x.txt'"), "{message}");
        assert!(message.as_str().contains("offset 1"), "{message}");
    }

    #[test]
    fn lines_split_on_lf_and_tolerate_crlf() {
        assert!(split_lines("").is_empty());
        assert_eq!(split_lines("a"), ["a"]);
        assert_eq!(split_lines("a\n"), ["a"]);
        assert_eq!(split_lines("a\nb"), ["a", "b"]);
        assert_eq!(split_lines("a\nb\n"), ["a", "b"]);
        assert_eq!(split_lines("a\r\nb\r\n"), ["a", "b"]);
        // 只有换行 = 一个空行；连续换行之间的空行保留。
        //
        // A lone newline is one empty line; empty lines in the middle are kept.
        assert_eq!(split_lines("\n"), [""]);
        assert_eq!(split_lines("a\n\nb"), ["a", "", "b"]);
    }
}
