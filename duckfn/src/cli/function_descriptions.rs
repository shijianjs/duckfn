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
use std::borrow::Cow;
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
/// 多个示例用 `, ` 拼进同一个 `example` 字段：`generate_md.sh` 只做
/// `'[' || other.example || ']'`，不会按分隔符再拆一次，所以拼成一个字段渲染出来才是 `[a, b]`
/// —— 与它原生分支 `list_reduce(lambda x, y : x || ', ' || y)` 的形态一致。
///
/// 字段里的换行会被压成一个空格，见 [`flatten_newlines`]。
///
/// Writes the header plus one row per function through a `csv` crate writer. Quoting and escaping
/// (for fields containing commas, quotes or newlines) are the crate's job rather than hand-rolled
/// string building; several examples are joined with `", "` into the single `example` field —
/// `generate_md.sh` only does `'[' || other.example || ']'` and never splits the field again, so
/// joining is what renders as `[a, b]`, matching its native
/// `list_reduce(lambda x, y : x || ', ' || y)` branch. Newlines inside a field collapse to a single
/// space, see [`flatten_newlines`].
pub fn write(path: &Path, rows: &[FunctionDescription]) -> io::Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(CSV_HEADER)?;
    for row in rows {
        let examples = row.examples.join(", ");
        let fields = [
            flatten_newlines(&row.function),
            flatten_newlines(row.description.as_deref().unwrap_or_default()),
            flatten_newlines(row.comment.as_deref().unwrap_or_default()),
            flatten_newlines(&examples),
        ];
        writer.write_record(fields.map(|field| field.into_owned()))?;
    }
    writer.flush()?;
    Ok(())
}

/// 把字段里的一段连续换行压成一个空格。
///
/// 两个原因，都是实测出来的：
///
/// 1. `generate_md.sh` 把这几列塞进一张 Markdown 表格，单元格里的换行会把表格行拆断；
/// 2. DuckDB 的 `read_csv()` 会把引号内字段里的 `\n` **读成 `\r\n`**
///    （实测：`length()` 多 1、`contains(x, chr(13))` 为真），所以原样写出去也会被改掉。
///
/// 与其让它变形成不可预期的样子，不如在写出时就压平。
///
/// Collapses a run of newlines inside a field into a single space, for two measured reasons:
/// `generate_md.sh` puts these columns into a Markdown table where a newline breaks the row, and
/// DuckDB's `read_csv()` reads a `\n` inside a quoted field back as `\r\n` (measured: `length()` is
/// one larger and `contains(x, chr(13))` is true). Flattening on the way out beats letting it
/// deform unpredictably on the way back in.
fn flatten_newlines(value: &str) -> Cow<'_, str> {
    if !value.contains(['\r', '\n']) {
        return Cow::Borrowed(value);
    }
    let mut flattened = String::with_capacity(value.len());
    let mut in_break = false;
    for ch in value.chars() {
        if ch == '\r' || ch == '\n' {
            if !in_break {
                flattened.push(' ');
                in_break = true;
            }
        } else {
            flattened.push(ch);
            in_break = false;
        }
    }
    Cow::Owned(flattened)
}
