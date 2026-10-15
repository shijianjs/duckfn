// ============================================================================
// 在扩展里执行 SQL：duckfn::sql
//
//   DuckDB 的 C API 只在部分回调里给出客户端上下文，而**客户端上下文里没有连接** —— 没有任何
//   duckdb_client_context_get_connection。所以回调（标量 / 表 / 聚合 / COPY）想跑 SQL，只能靠
//   注册期那条自有长连接，也就是 duckfn 在扩展加载时留下的那条（duckfn::duck_vfs 捕获的同一份）。
//
//   duckfn::sql 把这件事封好：
//     sql::execute(sql)              跑语句，返回影响行数（DDL / SET 是 0）
//     sql::query(sql)                跑查询，返回 DuckQueryResult（schema + 动态行）
//     sql::execute_with(sql, params) 参数绑定版
//     sql::query_with(sql, params)   参数绑定版
//     sql::quote_identifier(name)    表名 / 列名转义（标识符不能用参数占位）
//     sql::quote_literal(text)       字面量转义
//
//   DuckQueryResult 把结果处理那半边也带上了：按列名取单元格（cell）、取单行单列（single_value
//   / single_value_text）、直接变成动态表交给表函数（into_table）。列的**名字与类型都来自 SQL
//   自己**（列的 duckdb_column_logical_type 经 DuckTypeDesc 反推），所以一个表函数的输出 schema
//   可以是调用方在 SQL 里现写的。
//
//   In DuckDB's C API only some callbacks hand out a client context, and **a client context carries
//   no connection** — there is no duckdb_client_context_get_connection. A callback (scalar / table /
//   aggregate / COPY) that wants to run SQL therefore has exactly one window: the owned long-lived
//   connection duckfn keeps from load time (the one duckfn::duck_vfs captures).
//
//   duckfn::sql wraps that up: execute / query for the two shapes, *_with for parameter binding
//   (values are bound, never spliced), and quote_identifier / quote_literal for the identifiers and
//   literals a placeholder cannot carry. DuckQueryResult adds the result-handling half: read a cell by
//   column name, take the single row / single value, or turn the whole thing into a dynamic table.
//   Column names and types come from the result itself, so a table function's output schema can be
//   whatever the caller wrote in SQL.
//
//   前置条件：DuckDB 1.5.0+ 且 duckfn 打开 `owned-connection` feature（它自己依赖 `duckdb-1-5`；
//   本示例 crate 通过 `quack` → `all` 打开）。sql 与 duck_vfs 共用同一条连接与同一把锁：一次
//   调用从头持锁到尾，同一线程里别嵌套取用（会死锁）。
//
//   Requires DuckDB 1.5.0+ and duckfn's `owned-connection` feature (which needs `duckdb-1-5`; this
//   example crate gets it through `quack` → `all`). sql and duck_vfs share one connection and one
//   lock: a call holds it from start to finish, so never nest take-ups on one thread (that deadlocks).
// ============================================================================

use duckfn::{
    DuckDynamicTable, DuckDynamicValue, DuckOptionResult, DuckResult, duck_error,
    duck_scalar_function, duck_table_function, sql,
};

/// 执行一条（或多条，`;` 分隔）SQL 语句，返回影响行数；`CREATE` / `SET` 这类返回 0。
///
/// ```sql
/// SELECT dfn_sql_exec('CREATE OR REPLACE TABLE dfn_sql_demo(i INTEGER)');
/// SELECT dfn_sql_exec('INSERT INTO dfn_sql_demo VALUES (1), (2), (3)');  -- 3
/// ```
///
/// 标了 volatile：DuckDB 不会把常量参数的调用折叠成只执行一次，否则 `SELECT dfn_sql_exec(...)`
/// 可能只在一个分片里执行。
///
/// Runs one or more (`;`-separated) statements and returns the number of rows changed; `CREATE` /
/// `SET` and friends report 0. Marked volatile so DuckDB neither caches nor folds constant-argument
/// calls into a single execution.
#[duck_scalar_function(volatile = true)]
fn dfn_sql_exec(statement: String) -> DuckOptionResult<i64> {
    Ok(Some(
        i64::try_from(sql::execute(&statement)?).unwrap_or(i64::MAX),
    ))
}

/// 跑一条查询，把单行单列的结果渲染成文本；结果是 `NULL` 就回 `NULL`。
///
/// ```sql
/// SELECT dfn_sql_scalar('SELECT 6 * 7');          -- 42
/// SELECT dfn_sql_scalar('SELECT NULL::INTEGER');  -- NULL
/// ```
///
/// 标量函数只能返回固定的 Rust 类型，所以这里用文本兜底；要拿到带类型的结果请用
/// [`dfn_sql_query`]。
///
/// A scalar function can only return a fixed Rust type, so this falls back to text; use
/// [`dfn_sql_query`] for a typed result.
#[duck_scalar_function(volatile = true)]
fn dfn_sql_scalar(statement: String) -> DuckOptionResult<String> {
    sql::query(&statement)?.single_value_text()
}

/// 先绑一个 `BIGINT` 参数再跑查询：值走 `?` 占位，不拼进 SQL 文本。
///
/// ```sql
/// SELECT dfn_sql_scalar_arg('SELECT ?::BIGINT * 2', 21);  -- 42
/// ```
///
/// 这就是「参数从外面来」时的正确写法：`?` 在 SQL 里始终只是一个占位，值由
/// `DuckDynamicValue` 绑上去，注入无从谈起。
///
/// The right shape when the value comes from outside: `?` stays a placeholder in the SQL text and
/// the value is bound as a `DuckDynamicValue`, so there is nothing to inject.
#[duck_scalar_function(volatile = true)]
fn dfn_sql_scalar_arg(statement: String, value: i64) -> DuckOptionResult<String> {
    sql::query_with(&statement, &[DuckDynamicValue::BigInt(value)])?.single_value_text()
}

/// 数一张表有多少行：表名在运行期才知道，用 `quote_identifier` 转义后拼进 SQL。
///
/// ```sql
/// SELECT dfn_sql_table_count('dfn_sql_demo');   -- 3
/// SELECT dfn_sql_table_count('a"b');            -- 名字带引号也拼不坏 SQL
/// ```
///
/// SQL 里的标识符不能用 `?`，只能拼文本，所以这是必须自己守住的一处边界：`quote_identifier`
/// 把内部的 `"` 双写、首尾补引号，形如 `a"b` 的名字也跳不出这个引号。
///
/// Identifiers cannot be `?` placeholders and must be spliced, which is the one boundary the caller
/// has to hold: `quote_identifier` doubles inner `"` and wraps both ends, so a name like `a"b` cannot
/// break out.
#[duck_scalar_function(volatile = true)]
fn dfn_sql_table_count(table_name: String) -> DuckOptionResult<i64> {
    let statement = format!("SELECT count(*) FROM {}", sql::quote_identifier(&table_name));
    match sql::query(&statement)?.single_value()? {
        Some(DuckDynamicValue::BigInt(count)) => Ok(Some(count)),
        Some(other) => Err(duck_error(format!(
            "dfn_sql_table_count: count(*) came back as {} instead of BIGINT",
            other.type_id().sql_name()
        ))),
        None => Ok(None),
    }
}

/// 表是否存在：表名（外部来的字符串值）走参数绑定，不拼进 SQL 文本。
///
/// ```sql
/// SELECT dfn_sql_has_table('dfn_sql_demo');  -- true
/// SELECT dfn_sql_has_table('nope');          -- false
/// ```
///
/// 这里的 `information_schema.tables` 是自己写死的，直接写；只有**外部来的**内容才需要转义或
/// 绑定。值优先绑定（本函数就是），标识符才用 `sql::quote_identifier`（见
/// [`dfn_sql_table_count`]），而两边都用不上的极少见位置（`SET` 的取值、`INSTALL '...'`）才轮到
/// `sql::quote_literal`。
///
/// `information_schema.tables` here is written out in our own source, so it is spelled directly; only
/// external content needs escaping or binding. Values are bound first (this function does),
/// identifiers use `quote_identifier` (see [`dfn_sql_table_count`]), and the few places neither fits
/// (the value of a `SET`, `INSTALL '...'`) fall back to `quote_literal`.
#[duck_scalar_function(volatile = true)]
fn dfn_sql_has_table(table_name: String) -> DuckOptionResult<bool> {
    let statement = "SELECT count(*) FROM information_schema.tables WHERE table_name = ?";
    let count =
        sql::query_with(statement, &[DuckDynamicValue::Varchar(table_name)])?.single_value()?;
    Ok(Some(count == Some(DuckDynamicValue::BigInt(1))))
}

/// 把一条查询的结果整份回给 SQL：列名、列类型、行都来自被执行的语句本身。
///
/// ```sql
/// SELECT * FROM dfn_sql_query('SELECT 42 AS answer, [1, 2] AS tags');
/// DESCRIBE SELECT * FROM dfn_sql_query('SELECT count(*) AS n FROM dfn_sql_demo');
/// ```
///
/// 这是动态列通路最直接的用法：`bind` 阶段跑一句调用方给的 SQL，把 `DuckQueryResult` 变成
/// `DuckDynamicTable` 交回去 —— 列名、列数、`LIST` / `STRUCT` / `MAP` 的嵌套形状都不必在编译期
/// 知道。
///
/// The most direct use of the dynamic-column path: run the caller's query during bind and hand the
/// `DuckQueryResult` back as a `DuckDynamicTable` — names, column count and nesting are all unknown at
/// compile time.
///
/// `ENUM` / `ARRAY` / `UNION` / `BIT` 这几类列没有稳定的重建方式，跑到就报错（消息里带列名）。
///
/// Columns of type `ENUM` / `ARRAY` / `UNION` / `BIT` have no reliable reconstruction and fail with a
/// message naming the column.
#[duck_table_function(dynamic_columns = true)]
fn dfn_sql_query(statement: String) -> DuckResult<DuckDynamicTable> {
    Ok(sql::query(&statement)?.into_table())
}
