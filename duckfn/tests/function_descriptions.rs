//! `#[duck_*]` 的文档参数（`description` / `comment` / `example`）与 CLI 的 CSV 导出。
//!
//! 这里只测「源码声明 → inventory → CSV」这一段：不加载扩展、不查 catalog，所以不需要
//! DuckDB。CSV 那部分要 duckfn 的 `cli` feature：
//!
//! ```text
//! cargo test -p duckfn --features cli
//! ```
//!
//! The documentation arguments of the `#[duck_*]` macros (`description` / `comment` / `example`)
//! and the CLI's CSV export. This covers only "source declaration → inventory → CSV": no extension
//! is loaded and the catalog is never queried, so DuckDB is not needed. The CSV half needs duckfn's
//! `cli` feature (`cargo test -p duckfn --features cli`).

use duckfn::declared_function_descriptions;

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

/// 多条示例：`examples = [...]` 的顺序会被保留。
///
/// Several examples: the order of `examples = [...]` is preserved.
#[duckfn::duck_scalar_function(
    description = "Adds two INTEGERs",
    examples = ["SELECT docs_add_two(1, 2)", "SELECT docs_add_two(3, 4)"]
)]
fn docs_add_two(a: i64, b: i64) -> i64 {
    a + b
}

/// 字段里带逗号与引号：导出时必须按 CSV 规则加引号并双写引号。
///
/// A field containing a comma and a quote: it must be quoted with the inner quote doubled.
#[duckfn::duck_scalar_function(
    description = "Escapes , and \"quotes\"",
    example = "SELECT docs_escaped()"
)]
fn docs_escaped() -> i64 {
    0
}

/// 没写文档参数的函数同样会被收集（三个字段为空），但 `is_documented()` 为假。
///
/// A function without documentation arguments is collected too (all three fields empty), but
/// `is_documented()` is false for it.
#[duckfn::duck_scalar_function]
fn docs_undocumented(v: i64) -> i64 {
    v
}

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

    let double_it = find("docs_double_it");
    assert_eq!(double_it.description.as_deref(), Some("Doubles an INTEGER"));
    assert_eq!(double_it.comment.as_deref(), Some("NULL in, NULL out"));
    assert_eq!(
        double_it.examples,
        vec!["SELECT docs_double_it(21)".to_string()]
    );
    assert!(double_it.is_documented());

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

    let escaped = find("docs_escaped");
    assert_eq!(
        escaped.description.as_deref(),
        Some("Escapes , and \"quotes\"")
    );

    // 没写文档的函数也在结果里（`--all` 需要它的名字），只是没内容。
    //
    // A function without documentation is in the result too (`--all` needs its name) — just empty.
    let undocumented = find("docs_undocumented");
    assert!(!undocumented.is_documented());
    assert_eq!(undocumented.description, None);
    assert_eq!(undocumented.comment, None);
    assert!(undocumented.examples.is_empty());
}

/// 默认导出（不带 `--all`）：只写有文档的行，表头是 community-extensions 认的四列，
/// 逗号与引号按 CSV 规则转义。
///
/// The default export (no `--all`): only documented rows, with the header community-extensions
/// expects and commas/quotes escaped following CSV rules.
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
    assert_eq!(summary.written, 3);
    assert_eq!(summary.without_description, 0);
    assert_eq!(summary.skipped, 1, "the undocumented function must be skipped");

    let mut lines = text.lines();
    // `generate_md.sh` 用 `read_csv()` 读这份文件，列名就是它的 JOIN 与覆盖依据。
    //
    // `generate_md.sh` reads this file with `read_csv()`; the column names are what it joins and
    // overrides on.
    assert_eq!(lines.next(), Some("function,description,comment,example"));

    let body: Vec<&str> = lines.collect();
    // 只在字段里出现 `,` `"` 时才加引号（csv crate 的最小转义），所以 description 是裸的、
    // 带逗号的 comment 与拼接后的 example 才是带引号的。
    //
    // Quoting happens only when a field contains `,` or `"`, so the description stays bare while
    // the comma-bearing comment and the joined example are quoted.
    assert!(body.contains(
        &"docs_double_it,Doubles an INTEGER,\"NULL in, NULL out\",SELECT docs_double_it(21)"
    ));
    assert!(body.contains(
        &"docs_add_two,Adds two INTEGERs,,\"SELECT docs_add_two(1, 2), SELECT docs_add_two(3, 4)\""
    ));
    // 字段里的 `"` 双写。
    //
    // The inner `"` is doubled.
    assert!(body.contains(&"docs_escaped,\"Escapes , and \"\"quotes\"\"\",,SELECT docs_escaped()"));
    assert!(
        body.iter().all(|line| !line.starts_with("docs_undocumented")),
        "the undocumented function must not be exported without --all"
    );
    assert!(!text.contains('\r'));
    assert!(text.ends_with('\n'));
}

/// `--all`：文件名带 `_all` 后缀，没写文档的函数也在里面（字段留空）。
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
    assert_eq!(summary.written, 4);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.without_description, 1);
    assert!(text.lines().any(|line| line == "docs_undocumented,,,"));
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
