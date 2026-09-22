//! `#[duck_*]` 的文档参数（`description` / `comment` / `example`）与 CSV 导出。
//!
//! 这里只测「源码声明 → inventory → CSV」这一段：不加载扩展、不查 catalog，所以
//! 不需要 DuckDB（catalog 差集那段由 `just docs_csv` 手工验证）。
//!
//! The documentation arguments of the `#[duck_*]` macros (`description` / `comment` / `example`)
//! and the CSV export. This covers only "source declaration → inventory → CSV": no extension is
//! loaded and the catalog is never queried, so DuckDB is not needed (the catalog-diff half is
//! verified by hand through `just docs_csv`).

use duckfn::{
    declared_function_descriptions, duck_scalar_function, write_function_descriptions_csv,
};

/// 单条示例 + 注释：三列都有值。
///
/// A single example plus a comment: all three columns carry a value.
#[duck_scalar_function(
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
#[duck_scalar_function(
    description = "Adds two INTEGERs",
    examples = ["SELECT docs_add_two(1, 2)", "SELECT docs_add_two(3, 4)"]
)]
fn docs_add_two(a: i64, b: i64) -> i64 {
    a + b
}

/// 字段里带逗号与引号：导出时必须按 RFC4180 加引号并双写引号。
///
/// A field containing a comma and a quote: it must be quoted with the inner quote doubled,
/// following RFC 4180.
#[duck_scalar_function(
    description = "Escapes , and \"quotes\"",
    example = "SELECT docs_escaped()"
)]
fn docs_escaped() -> i64 {
    0
}

/// 没写文档参数的函数不出现在 `declared_function_descriptions()` 里。
///
/// A function without documentation arguments never shows up in
/// `declared_function_descriptions()`.
#[duck_scalar_function]
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
    assert_eq!(double_it.examples, vec!["SELECT docs_double_it(21)".to_string()]);

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
    assert_eq!(escaped.description.as_deref(), Some("Escapes , and \"quotes\""));

    assert!(
        rows.iter().all(|row| row.function != "docs_undocumented"),
        "a function without documentation arguments must not be collected"
    );
}

/// 导出的 CSV：表头是 community-extensions 认的四列，逗号与引号按 RFC4180 转义。
///
/// The exported CSV: the header is the four columns community-extensions expects, with commas and
/// quotes escaped following RFC 4180.
#[test]
fn csv_matches_what_community_extensions_reads() {
    let dir = std::env::temp_dir().join(format!("duckfn_doc_test_{}", std::process::id()));
    let path = dir.join("function_descriptions.csv");
    write_function_descriptions_csv(&path).expect("writing the CSV must succeed");

    let text = std::fs::read_to_string(&path).expect("the CSV must be readable");
    let _ = std::fs::remove_dir_all(&dir);

    let mut lines = text.lines();
    // `generate_md.sh` 用 `read_csv()` 读这份文件，列名就是它的 JOIN 与覆盖依据。
    //
    // `generate_md.sh` reads this file with `read_csv()`; the column names are what it joins and
    // overrides on.
    assert_eq!(lines.next(), Some("function,description,comment,example"));

    let body: Vec<&str> = lines.collect();
    // 只在字段里出现 `,` `"` 时才加引号（RFC4180 的最小转义），所以 description 是裸的、
    // 带逗号的 comment 与拼接后的 example 才是带引号的。
    //
    // Quoting happens only when a field contains `,` or `"` (the minimal RFC 4180 escaping), so
    // the description stays bare while the comma-bearing comment and the joined example are
    // quoted.
    assert!(body.contains(&"docs_double_it,Doubles an INTEGER,\"NULL in, NULL out\",SELECT docs_double_it(21)"));
    assert!(body.contains(&"docs_add_two,Adds two INTEGERs,,\"SELECT docs_add_two(1, 2), SELECT docs_add_two(3, 4)\""));
    // 字段里的 `"` 双写。
    //
    // The inner `"` is doubled.
    assert!(body.contains(&"docs_escaped,\"Escapes , and \"\"quotes\"\"\",,SELECT docs_escaped()"));
    // 每行结尾没有多余的空白，行分隔用 LF。
    //
    // No trailing whitespace at the end of a line and LF line endings.
    assert!(!text.contains('\r'));
    assert!(text.ends_with('\n'));
}
