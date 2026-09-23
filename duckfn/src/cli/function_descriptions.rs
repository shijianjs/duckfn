//! `function_descriptions` 子命令：把 `#[duck_*]` 上声明的函数文档写成
//! community-extensions 文档页用的 CSV。
//!
//! 这是纯内存操作：读的是宏提交进 inventory 的元数据，不加载扩展、不查 catalog，
//! 因此不需要 DuckDB，也不受插件运行时影响。
//!
//! The `function_descriptions` subcommand: writes the function documentation declared on
//! `#[duck_*]` into the CSV the community-extension doc pages consume. It is a pure in-memory
//! operation over the metadata the macros submitted to inventory — no extension is loaded and the
//! catalog is never queried, so it needs no DuckDB and is unaffected by the extension runtime.

use crate::{FunctionDescription, declared_function_descriptions};
use std::io;
use std::path::{Path, PathBuf};

/// CSV 的表头。`duckdb/community-extensions` 的 `scripts/generate_md.sh` 用 `read_csv()` 读这份
/// 文件，`function` 是它的 JOIN 键（对应 `function_name`），另外三列覆盖同名展示列。
///
/// The CSV header. `scripts/generate_md.sh` in `duckdb/community-extensions` reads this file with
/// `read_csv()`; `function` is the JOIN key (matching `function_name`) and the other three columns
/// override the display columns of the same name.
pub const CSV_HEADER: [&str; 4] = ["function", "description", "comment", "example"];

/// 默认的输出文件名（位于项目根目录的 `target/` 下）。
///
/// The default output file name (under the project root's `target/`).
pub const FILE_NAME: &str = "function_descriptions.csv";

/// `--all` 时的输出文件名。
///
/// The output file name when `--all` is given.
pub const ALL_FILE_NAME: &str = "function_descriptions_all.csv";

/// 一次导出的结果，供 CLI 打一行摘要。
///
/// The outcome of one export, so the CLI can print a one-line summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    /// 写进 CSV 的行数。
    ///
    /// How many rows were written.
    pub written: usize,
    /// 其中没有 `description` 的行数。
    ///
    /// How many of them have no `description`.
    pub without_description: usize,
    /// 因为完全没写文档而被跳过的行数（`--all` 时恒为 0）。
    ///
    /// How many rows were skipped because they carry no documentation at all (always 0 with
    /// `--all`).
    pub skipped: usize,
}

/// 输出路径固定为 `<project_dir>/target/<文件名>`：固定路径便于后续自动化处理。
///
/// The output path is fixed at `<project_dir>/target/<file name>`, which keeps later automation
/// simple.
pub fn output_path(project_dir: &Path, all: bool) -> PathBuf {
    project_dir
        .join("target")
        .join(if all { ALL_FILE_NAME } else { FILE_NAME })
}

/// 收集宏声明的文档并写出 CSV，返回统计。
///
/// `all` 为 `false`（默认）时只写 [`FunctionDescription::is_documented`] 为真的函数，
/// 为 `true` 时把没写文档的也写出来（三列留空），便于当「还差哪些没写」的清单看。
///
/// # Errors
///
/// 建目录或写文件失败时返回 I/O 错误。
///
/// Collects the documentation declared by the macros, writes the CSV and reports what happened.
/// With `all == false` (the default) only functions where [`FunctionDescription::is_documented`]
/// holds are written; with `all == true` the undocumented ones are written too, with empty columns,
/// which makes the file a "still to write" checklist.
///
/// # Errors
///
/// Returns an I/O error when creating the directory or writing the file fails.
pub fn export(project_dir: &Path, all: bool) -> io::Result<Summary> {
    let declared = declared_function_descriptions();
    let declared_rows = declared.len();
    let rows: Vec<FunctionDescription> = if all {
        declared
    } else {
        declared
            .into_iter()
            .filter(FunctionDescription::is_documented)
            .collect()
    };

    let path = output_path(project_dir, all);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write(&path, &rows)?;

    Ok(Summary {
        written: rows.len(),
        without_description: rows.iter().filter(|row| row.description.is_none()).count(),
        skipped: declared_rows - rows.len(),
    })
}

/// 用一个 `csv` crate 的 writer 写出表头 + 每一行。
///
/// 转义（字段含逗号 / 引号 / 换行时的加引号与双写）由 `csv` crate 负责，这里不手拼字符串；
/// 多个示例用 `, ` 拼进同一个 `example` 字段，渲染出来就是 community-extensions 期望的列表形式。
///
/// Writes the header plus one row per function through a `csv` crate writer. Quoting and escaping
/// (for fields containing commas, quotes or newlines) are the crate's job rather than hand-rolled
/// string building; several examples are joined with `", "` into the single `example` field, which
/// is the list form community-extensions expects.
pub fn write(path: &Path, rows: &[FunctionDescription]) -> io::Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(CSV_HEADER)?;
    for row in rows {
        let examples = row.examples.join(", ");
        writer.write_record([
            row.function.as_str(),
            row.description.as_deref().unwrap_or_default(),
            row.comment.as_deref().unwrap_or_default(),
            examples.as_str(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}
