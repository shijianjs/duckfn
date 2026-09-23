//! `#[duck_*]` 的文档参数（`description` / `comment` / `example`）与 CLI 的 CSV 导出。
//!
//! 覆盖刻意挑出来的易错点：
//!
//! - **A 普通函数**：三列齐全时逐字落在 CSV 里。
//! - **B 没有 metadata**：默认导出跳过它；`--all` 才写出来（三列留空）。
//! - **C CSV 特殊字符**：逗号、双引号、前后空格、非 ASCII、Jekyll 的 `{{ }}` / `{% raw %}`；
//!   源文本里的换行会在写出时压成空格（Markdown 表格与 `read_csv()` 都不接受裸换行）。
//! - **D overload**：多个签名挂到同一个函数集时只出一行，名字是函数集名而不是 Rust 函数名，
//!   元数据按「description/comment 取第一个非空、examples 拼接去重」合并。
//! - **E 多个 example**：CSV 的 `example` 是**一个字符串**，
//!   `generate_md.sh` 只做 `'[' || example.jekyll_format_function() || ']'`，
//!   **不会**按分隔符再拆一次（对照它原生分支的 `list_reduce(lambda x, y : x || ', ' || y)`）。
//!   所以多条示例要用 `", "` 拼成一个字段，渲染出来才是 `[a, b]`，与原生形态一致。
//!
//! 另外用一个可选的 DuckDB 往返测试把「这份 CSV 真的能被 `read_csv()` 读回原值」钉死 ——
//! 转义写错了只有真读一遍才知道。没装 `duckdb` 命令行时该测试打印提示后跳过。
//!
//! Documentation arguments of the `#[duck_*]` macros and the CLI's CSV export. The cases are
//! chosen for the things that actually go wrong: a fully documented function, one with no metadata,
//! CSV special characters, overloads collapsing into one row, and multiple examples. A separate
//! round-trip test (skipped when the `duckdb` CLI is not installed) proves the file still reads back
//! through `read_csv()`.
//!
//! CSV 那部分要 duckfn 的 `cli` feature：
//!
//! ```text
//! cargo test -p duckfn --features cli
//! ```

use duckfn::declared_function_descriptions;

// ============================================================================
// A 普通函数：三列齐全
// ============================================================================

/// 单条示例 + 注释：三列都有值。
///
/// A single example plus a comment: all three columns carry a value.
#[duckfn::duck_scalar_function(
    description = "Doubles an INTEGER",
    comment = "NULL in, NULL out",
    example = "SELECT docs_double_it(21)"
)]
fn docs_double_it(v: Option<i64>) -> Option<i64> {
    v.map(|x| x * 2)
}

// ============================================================================
// E 多个 example
// ============================================================================

/// 多条示例：`examples = [...]` 的顺序会被保留，导出时用 `", "` 拼成一个字段。
///
/// Several examples: the order of `examples = [...]` is preserved and they are joined with `", "`
/// into the single CSV field.
#[duckfn::duck_scalar_function(
    description = "Adds two INTEGERs",
    examples = ["SELECT docs_add_two(1, 2)", "SELECT docs_add_two(3, 4)"]
)]
fn docs_add_two(a: i64, b: i64) -> i64 {
    a + b
}

// ============================================================================
// B 没有 metadata
// ============================================================================

/// 没写文档参数的函数同样会被收集（三个字段为空），但 `is_documented()` 为假、默认导出不写它。
///
/// A function with no documentation argument is collected too (all three fields empty), but
/// `is_documented()` is false and the default export leaves it out.
#[duckfn::duck_scalar_function]
fn docs_undocumented(v: i64) -> i64 {
    v
}

// ============================================================================
// C CSV 特殊字符
// ============================================================================

/// 描述里同时有逗号、双引号与真正的换行；注释两侧带空格；示例里再来一次逗号与双引号。
///
/// The description carries a comma, a double quote and a real newline; the comment is padded with
/// spaces; the example repeats the comma and the quote.
#[duckfn::duck_scalar_function(
    description = "Comma, \"quote\", and\na newline",
    comment = "  padded  ",
    example = "SELECT docs_special('a,b') -- say \"hi\" {{ }} {% raw %}"
)]
fn docs_special() -> i64 {
    0
}

/// 非 ASCII：CSV 是 UTF-8，读回来必须一字不差。
///
/// Non-ASCII: the CSV is UTF-8 and must read back byte for byte.
#[duckfn::duck_scalar_function(
    description = "把 INTEGER 翻倍",
    example = "SELECT docs_unicode()"
)]
fn docs_unicode() -> i64 {
    0
}

// ============================================================================
// D overload：两个签名挂到同一个函数集
// ============================================================================

/// 函数集 `docs_overloaded` 的 INTEGER 签名：提供 description。
///
/// The INTEGER signature of the `docs_overloaded` set: supplies the description.
#[duckfn::duck_scalar_function(
    overloads_name = "docs_overloaded",
    description = "Overloaded: INTEGER input",
    example = "SELECT docs_overloaded(1)"
)]
fn docs_overload_int(v: i64) -> i64 {
    v
}

/// 同一个函数集的 VARCHAR 签名：提供 comment，并且和上面共享同一个 example（导出时必须去重）。
///
/// The VARCHAR signature of the same set: supplies the comment and shares one example with the
/// other signature (which the export must deduplicate).
#[duckfn::duck_scalar_function(
    overloads_name = "docs_overloaded",
    comment = "Also accepts VARCHAR",
    examples = ["SELECT docs_overloaded(1)", "SELECT docs_overloaded('a')"]
)]
fn docs_overload_str(v: String) -> String {
    v
}

// ============================================================================
// 默认导出（只写有文档的）：整份文件逐字节比对
// ============================================================================

/// 默认导出的完整内容，按函数名排序：
///
/// - 只在含 `,` 或 `"` 时才加引号（`csv` crate 的最小转义），字段里的 `"` 双写；
///   所以 `Adds two INTEGERs` 是裸的，而 `NULL in, NULL out` 带引号；
/// - `docs_overloaded` 只占一行，`function` 是函数集名而不是两个 Rust 函数名；
/// - `docs_special` 描述里的换行被压成空格（见 `flatten_newlines`），因此没有任何字段跨物理行；
/// - 没写文档的 `docs_undocumented` 不出现。
///
/// The full content of the default export, sorted by function name: quoting only where a field
/// contains `,` or `"`, with inner quotes doubled (so `Adds two INTEGERs` stays bare and
/// `NULL in, NULL out` is quoted); one single row for the overload set; the newline inside
/// `docs_special`'s description flattened to a space (see `flatten_newlines`), so no field spans a
/// physical line; and no `docs_undocumented`.
#[cfg(feature = "cli")]
const EXPECTED_DEFAULT_CSV: &str = concat!(
    "function,description,comment,example\n",
    "docs_add_two,Adds two INTEGERs,,\"SELECT docs_add_two(1, 2), SELECT docs_add_two(3, 4)\"\n",
    "docs_double_it,Doubles an INTEGER,\"NULL in, NULL out\",SELECT docs_double_it(21)\n",
    "docs_overloaded,Overloaded: INTEGER input,Also accepts VARCHAR,\"SELECT docs_overloaded(1), SELECT docs_overloaded('a')\"\n",
    "docs_special,\"Comma, \"\"quote\"\", and a newline\",  padded  ,\"SELECT docs_special('a,b') -- say \"\"hi\"\" {{ }} {% raw %}\"\n",
    "docs_unicode,把 INTEGER 翻倍,,SELECT docs_unicode()\n",
);

/// 收集到的文档按函数名排序，字段与各条属性一一对应。
///
/// The collected entries are sorted by function name and every field matches the attribute that
/// produced it.
#[test]
fn declared_documentation_is_collected() {
    let rows = declared_function_descriptions();
    // 只断言本文件声明的函数，其它测试文件提交的条目不关这里的事。
    //
    // Only the functions declared here are asserted on; submissions from other test files are
    // none of this test's business.
    let find = |name: &str| {
        rows.iter()
            .find(|row| row.function == name)
            .unwrap_or_else(|| panic!("`{name}` is missing from the collected documentation"))
    };

    // A：三列齐全。
    let double_it = find("docs_double_it");
    assert_eq!(double_it.description.as_deref(), Some("Doubles an INTEGER"));
    assert_eq!(double_it.comment.as_deref(), Some("NULL in, NULL out"));
    assert_eq!(
        double_it.examples,
        vec!["SELECT docs_double_it(21)".to_string()]
    );
    assert!(double_it.is_documented());

    // E：多条示例按声明顺序保留；没写 comment 就是 None。
    let add_two = find("docs_add_two");
    assert_eq!(add_two.description.as_deref(), Some("Adds two INTEGERs"));
    assert_eq!(add_two.comment, None);
    assert_eq!(
        add_two.examples,
        vec![
            "SELECT docs_add_two(1, 2)".to_string(),
            "SELECT docs_add_two(3, 4)".to_string()
        ]
    );

    // B：收集到了，但没内容。
    let undocumented = find("docs_undocumented");
    assert!(!undocumented.is_documented());
    assert_eq!(undocumented.description, None);
    assert_eq!(undocumented.comment, None);
    assert!(undocumented.examples.is_empty());

    // C：收集到的还是原样的字符 —— 逗号、引号、真实换行、前后空格、非 ASCII 都还在；
    // 换行只在写 CSV 时才被压平（见 `export_writes_documented_rows_only`）。
    //
    // C: what is collected is still the literal text — comma, quote, a real newline, padding and
    // non-ASCII all intact. The newline is only flattened when the CSV is written.
    let special = find("docs_special");
    assert_eq!(
        special.description.as_deref(),
        Some("Comma, \"quote\", and\na newline")
    );
    assert_eq!(special.comment.as_deref(), Some("  padded  "));
    assert_eq!(find("docs_unicode").description.as_deref(), Some("把 INTEGER 翻倍"));

    // D：两个签名合并成一条，名字是函数集名；Rust 函数名不出现在结果里。
    assert!(
        rows.iter().all(|row| row.function != "docs_overload_int"
            && row.function != "docs_overload_str"),
        "overloads must be reported under the function-set name, not the Rust function names"
    );
    let overloaded: Vec<_> = rows
        .iter()
        .filter(|row| row.function == "docs_overloaded")
        .collect();
    assert_eq!(overloaded.len(), 1, "the overload set must collapse into one row");
    let overloaded = overloaded[0];
    // description 只有 INTEGER 签名写了 → 取第一个非空就是它，与遍历顺序无关。
    // comment 只有 VARCHAR 签名写了。
    assert_eq!(
        overloaded.description.as_deref(),
        Some("Overloaded: INTEGER input")
    );
    assert_eq!(overloaded.comment.as_deref(), Some("Also accepts VARCHAR"));
    // 两个签名各给了一个 example，其中 `SELECT docs_overloaded(1)` 重复 → 只保留一次。
    // 重复项被去掉后，两种遍历顺序都会得到同一个顺序，所以这里可以直接逐项断言。
    assert_eq!(
        overloaded.examples,
        vec![
            "SELECT docs_overloaded(1)".to_string(),
            "SELECT docs_overloaded('a')".to_string()
        ]
    );
}

/// 默认导出：只写有文档的函数，表头是 community-extensions 认的四列。
///
/// The default export: only documented rows, with the header community-extensions expects.
#[cfg(feature = "cli")]
#[test]
fn export_writes_documented_rows_only() {
    let dir = temp_dir("default");
    let summary = duckfn::cli::function_descriptions::export(&dir, false).expect("export must work");
    let path = duckfn::cli::function_descriptions::output_path(&dir, false);
    let text = std::fs::read_to_string(&path).expect("the CSV must be readable");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("function_descriptions.csv")
    );
    assert_eq!(summary.written, 5);
    assert_eq!(summary.without_description, 0);
    assert_eq!(summary.skipped, 1, "the undocumented function must be skipped");

    // 整份文件逐字节比对：转义、排序、合并、跳过、换行压平一次性都验了。
    //
    // Byte-for-byte comparison of the whole file: escaping, ordering, merging, skipping and
    // newline flattening in one assertion.
    assert_eq!(text, EXPECTED_DEFAULT_CSV);
    // 没有字段跨物理行 —— 这正是换行被压平的结果，Markdown 表格与 `read_csv()` 都要求这样。
    //
    // No field spans a physical line, which is what the flattening buys: both the Markdown table
    // and `read_csv()` require it.
    assert_eq!(text.lines().count(), 6, "header + 5 rows, no multi-line field");
    // LF 换行（仓库约定），且文件以换行结束。
    //
    // LF line endings (the repository convention), with a trailing newline.
    assert!(!text.contains('\r'));
    assert!(text.ends_with('\n'));
}

/// `--all`：文件名带 `_all` 后缀，没写文档的函数也在里面（三列留空）。
///
/// `--all`: the file name carries the `_all` suffix and undocumented functions are included with
/// empty fields.
#[cfg(feature = "cli")]
#[test]
fn export_all_includes_undocumented_rows() {
    let dir = temp_dir("all");
    let summary = duckfn::cli::function_descriptions::export(&dir, true).expect("export must work");
    let path = duckfn::cli::function_descriptions::output_path(&dir, true);
    let text = std::fs::read_to_string(&path).expect("the CSV must be readable");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("function_descriptions_all.csv")
    );
    assert_eq!(summary.written, 6);
    assert_eq!(summary.without_description, 1);
    assert_eq!(summary.skipped, 0);

    // `docs_undocumented` 排序上正好在 `docs_unicode` 前面，插进去即可。
    //
    // `docs_undocumented` sorts right before `docs_unicode`, so inserting it is enough.
    let expected = EXPECTED_DEFAULT_CSV.replace(
        "docs_unicode,",
        "docs_undocumented,,,\ndocs_unicode,",
    );
    assert_eq!(text, expected);
    assert_eq!(text.lines().count(), 7, "header + 6 rows, no multi-line field");
}

// ============================================================================
// 往返：这份 CSV 真的能被 DuckDB 读回来
// ============================================================================

/// 把导出的 CSV 交给 `duckdb` 的 `read_csv()`，确认值原样回来、JOIN 键对得上、
/// `generate_md.sh` 那两段 SQL 的语义成立：
///
/// - 用 `function_name == other.function` 能查到行；
/// - `'[' || other.example || ']'` 给出 `[a, b]`（与原生 `list_reduce(x || ', ' || y)` 一致）；
/// - 空字段被读成 `NULL`（不是空串）—— `generate_md.sh` 那边 `'[' || NULL || ']'` 就是 NULL，
///   页面留空，正是「没写示例」想要的效果。
///
/// Feeds the exported CSV to `duckdb`'s `read_csv()` and checks that values come back unchanged,
/// the JOIN key matches, and the semantics of the two `generate_md.sh` statements hold.
///
/// 没装 `duckdb` 命令行时打印提示并跳过。
///
/// Prints a notice and returns when the `duckdb` CLI is not installed.
#[cfg(feature = "cli")]
#[test]
fn csv_round_trips_through_duckdb() {
    let Some(duckdb) = duckdb_binary() else {
        eprintln!("skipping csv_round_trips_through_duckdb: no `duckdb` CLI on PATH");
        return;
    };

    let dir = temp_dir("roundtrip");
    duckfn::cli::function_descriptions::export(&dir, true).expect("export must work");
    let path = duckfn::cli::function_descriptions::output_path(&dir, true);
    let source = format!("read_csv('{}')", sql_path(&path));
    let select = |expr: &str, function: &str| {
        duckdb_scalar(
            &duckdb,
            &format!("SELECT {expr} FROM {source} WHERE function = '{function}'"),
        )
    };

    // A + C：含逗号与双引号的描述、两侧带空格的注释、非 ASCII，读回来一字不差。
    assert_eq!(
        select("description", "docs_special"),
        "Comma, \"quote\", and a newline"
    );
    assert_eq!(select("comment", "docs_special"), "  padded  ");
    assert_eq!(
        select("example", "docs_special"),
        "SELECT docs_special('a,b') -- say \"hi\" {{ }} {% raw %}"
    );
    assert_eq!(select("description", "docs_unicode"), "把 INTEGER 翻倍");

    // C：源文本里的换行没有落到 CSV 里 —— 裸换行会被 `read_csv()` 读成 `\r\n`（实测），
    // 而且会拆断 `generate_md.sh` 生成的 Markdown 表格，所以写出时就压成空格了。
    //
    // The newline in the source text never reaches the CSV: `read_csv()` reads a bare `\n` back as
    // `\r\n` (measured) and it would break the Markdown table `generate_md.sh` builds, so it is
    // flattened to a space on the way out.
    assert_eq!(select("contains(description, chr(10))", "docs_special"), "false");
    assert_eq!(select("contains(description, chr(13))", "docs_special"), "false");

    // E：多条示例拼成一个字段，`generate_md.sh` 包上方括号后与原生列表形态一致。
    assert_eq!(
        select("example", "docs_add_two"),
        "SELECT docs_add_two(1, 2), SELECT docs_add_two(3, 4)"
    );
    assert_eq!(
        select("'[' || example || ']'", "docs_add_two"),
        "[SELECT docs_add_two(1, 2), SELECT docs_add_two(3, 4)]"
    );

    // D：重载只占一行，`function` 是函数集名。
    assert_eq!(
        duckdb_scalar(
            &duckdb,
            &format!("SELECT count(*) FROM {source} WHERE function = 'docs_overloaded'"),
        ),
        "1"
    );
    assert_eq!(
        select("description", "docs_overloaded"),
        "Overloaded: INTEGER input"
    );

    // B：`--all` 里的空字段被读成 NULL；`docs_overloaded` 的 comment 有值、example 有值。
    assert_eq!(select("comment IS NULL", "docs_undocumented"), "true");
    assert_eq!(select("comment IS NULL", "docs_double_it"), "false");

    // `function` 列就是 JOIN 键：函数名与排序都对。
    assert_eq!(
        duckdb_scalar(
            &duckdb,
            &format!(
                "SELECT string_agg(function, ',') FROM (SELECT function FROM {source} ORDER BY 1)"
            ),
        ),
        "docs_add_two,docs_double_it,docs_overloaded,docs_special,docs_undocumented,docs_unicode"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 每个测试用各自的临时目录，避免互相覆盖。
///
/// Each test gets its own temporary directory so they cannot overwrite one another.
#[cfg(feature = "cli")]
fn temp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("duckfn_doc_test_{label}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// `duckdb` 命令行：`DUCKDB` 环境变量优先（与 Justfile 的约定一致），否则 PATH 里的 `duckdb`。
///
/// The `duckdb` CLI: the `DUCKDB` environment variable wins (same convention as the Justfile),
/// otherwise `duckdb` from `PATH`.
#[cfg(feature = "cli")]
fn duckdb_binary() -> Option<String> {
    let candidate = std::env::var("DUCKDB").unwrap_or_else(|_| "duckdb".to_string());
    let ok = std::process::Command::new(&candidate)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    ok.then_some(candidate)
}

/// 跑一条只返回单个值的 SQL，返回去掉行尾换行的原始输出。
///
/// Runs a query returning a single value and returns the raw output without the trailing newline.
/// Raw output (rather than parsing) is what makes the multi-line description comparable exactly.
#[cfg(feature = "cli")]
fn duckdb_scalar(duckdb: &str, sql: &str) -> String {
    let output = std::process::Command::new(duckdb)
        .args(["-noheader", "-list", "-c", sql])
        .output()
        .expect("running duckdb");
    assert!(
        output.status.success(),
        "duckdb failed: {}\nSQL: {sql}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("duckdb writes UTF-8")
        .trim_end_matches(['\r', '\n'])
        .to_string()
}

/// DuckDB 的 SQL 字符串字面量里用正斜杠更省事，单引号要双写。
///
/// Forward slashes keep the path simple inside a SQL string literal, and single quotes are doubled.
#[cfg(feature = "cli")]
fn sql_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/").replace('\'', "''")
}
