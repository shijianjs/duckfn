//! 在扩展里执行 SQL：把自有连接上的查询变成**动态结果集**。
//!
//! Running SQL from inside an extension: queries on the owned connection as **dynamic result sets**.
//!
//! # 为什么需要这一层 / Why this exists
//!
//! DuckDB 的 C API 只在部分回调里交出客户端上下文，而**客户端上下文里没有连接**：没有任何
//! `duckdb_client_context_get_connection`。所以回调（标量 / 表 / 聚合 / COPY）想跑 SQL，只能靠
//! 注册期那条**自有长连接**（`duckfn` 内部捕获，`duckfn::duck_vfs` 用的也是同一份）。本模块把
//! 「在它上面跑 SQL、再把结果接回来」这整件事封好：不用自己去碰 `duckdb_query` /
//! `duckdb_prepare` / `duckdb_fetch_chunk` 那套句柄，也不必自己拆 `duckdb_result` 的列与块。
//!
//! DuckDB's C API hands out a client context in some callbacks only, and **a client context carries
//! no connection**: there is no `duckdb_client_context_get_connection`. A callback (scalar / table /
//! aggregate / COPY) that wants to run SQL therefore has exactly one window — the **owned long-lived
//! connection** `duckfn` captures at registration time (the same one `duckfn::duck_vfs` uses). This
//! module wraps the whole job: run SQL on it and bring the result back, without touching the
//! `duckdb_query` / `duckdb_prepare` / `duckdb_fetch_chunk` handles or dissecting a `duckdb_result`
//! column by column.
//!
//! # 只要 `sql` feature，不需要 `duckdb-1-5` / Only the `sql` feature — no `duckdb-1-5`
//!
//! 本模块发出的每个调用都在 C API 的**稳定区**（`duckdb_connect` + 预处理 / 绑定 / 取块 + 逻辑
//! 类型），所以它落在 `sql` feature 后面、**不依赖 `duckdb-1-5`**：配
//! `USE_UNSTABLE_C_API=0` + `TARGET_DUCKDB_VERSION=v1.2.0` 打出来的一份 `C_STRUCT` 二进制，
//! DuckDB 1.3 一直到 2.0 都接受；quack-rs 的 ABI 自检也直接返回 `StableOnly`，不做版本比对。
//! **宿主文件系统**才是需要 1.5 的那一半：开 `owned-connection` 会带来本模块，并把
//! `duckfn::duck_vfs` 一并加上。
//!
//! Every call this module makes lives in the **stable** region of the C API (`duckdb_connect` plus
//! prepare / bind / fetch-chunk and the logical types), so it sits behind the `sql` feature and
//! **does not depend on `duckdb-1-5`**: built with `USE_UNSTABLE_C_API=0` and a
//! `TARGET_DUCKDB_VERSION=v1.2.0` floor, the `C_STRUCT` artifact is accepted all the way from DuckDB
//! 1.3 to 2.0, and quack-rs' ABI check short-circuits to `StableOnly`. **The host file system** is
//! the half that needs 1.5: enabling `owned-connection` brings this module and adds
//! `duckfn::duck_vfs` on top.
//!
//! # 两个层次 / Two layers
//!
//! - **执行**：[`execute`] / [`execute_with`] 跑语句、拿影响行数（`CREATE` / `SET` 之类返回 0）；
//!   [`query`] / [`query_with`] 跑查询、拿 [`DuckQueryResult`]。
//! - **结果处理**：[`DuckQueryResult`] 是「schema + 若干动态行」，能按列名取值、取单行单列、
//!   也能直接变成 [`DuckDynamicTable`](crate::DuckDynamicTable) 交给动态表函数。
//!
//! - **Running statements**: [`execute`] / [`execute_with`] return the number of rows changed (0 for
//!   `CREATE` / `SET` and friends); [`query`] / [`query_with`] return a [`DuckQueryResult`].
//! - **Handling results**: a [`DuckQueryResult`] is a schema plus dynamic rows — read a cell by
//!   column name, take the single row / single value, or turn it straight into a
//!   [`DuckDynamicTable`](crate::DuckDynamicTable) for a dynamic table function.
//!
//! # 动态列：结果类型从 SQL 自己来 / Dynamic columns: the types come from SQL itself
//!
//! 查询的列名与列类型都取自结果本身（`duckdb_column_name` / `duckdb_column_logical_type`），
//! 再经 [`DuckTypeDesc::from_logical_type`](crate::DuckTypeDesc::from_logical_type) 变成运行期描述，
//! 所以一个表函数可以这样写：`bind` 里跑一句用户给的 SQL，把列与行原样交给 DuckDB ——
//! 列名、列数、嵌套结构都不必在编译期知道。
//!
//! Column names and types are read off the result itself (`duckdb_column_name` /
//! `duckdb_column_logical_type`) and turned into runtime descriptions by
//! [`DuckTypeDesc::from_logical_type`](crate::DuckTypeDesc::from_logical_type). A table function can
//! therefore run a caller-supplied query in `bind` and hand the columns and rows straight to DuckDB —
//! neither the names, the column count nor the nesting has to be known at compile time.
//!
//! 能表达的形状与动态列通路一致：标量、`DECIMAL`、`LIST`、`STRUCT`、`MAP`，可任意嵌套。`ENUM` /
//! `ARRAY` / `UNION` / `BIT` 没有稳定的重建方式，遇到就报错（错误的列名与类型都写在消息里），
//! 而不是给出一个不完整的描述。
//!
//! The expressible shapes are the ones the dynamic-column path supports: scalars, `DECIMAL`, `LIST`,
//! `STRUCT` and `MAP`, nested arbitrarily. `ENUM` / `ARRAY` / `UNION` / `BIT` have no reliable
//! reconstruction and are an error naming the column (rather than an incomplete description).
//!
//! # 参数：别拼 SQL，绑值 / Parameters: bind values, do not splice SQL
//!
//! 表名、路径、用户输入这类**运行时才知道**的内容有两种处理方式，本模块都给了：
//!
//! - **值**用参数绑定（[`query_with`] / [`execute_with`]）：`?` 位置上绑一个
//!   [`DuckDynamicValue`]，DuckDB 自己按类型收下（内部是 `duckdb_bind_value`），SQL 文本里始终
//!   只有 `?`，注入无从谈起。绑定支持全部标量分支；`LIST` / `STRUCT` / `MAP` 目前不支持——它们
//!   的嵌套 NULL 无法用 `duckdb_value` 表达，请在 SQL 里就地构造。
//! - **标识符**（表名 / 列名）在 SQL 里不能用参数占位，得自己拼进文本，此时用
//!   [`quote_identifier`] 双引号转义；字面量用 [`quote_literal`]。
//!
//! Runtime-only content — table names, paths, user input — has two treatments, both provided here:
//!
//! - **Values** go through parameter binding ([`query_with`] / [`execute_with`]): bind a
//!   [`DuckDynamicValue`] at a `?` placeholder and DuckDB takes it typed (this is
//!   `duckdb_bind_value` under the hood), so the SQL text only ever contains `?` and injection has
//!   nowhere to happen. Every scalar variant is supported; `LIST` / `STRUCT` / `MAP` are not, because
//!   their nested NULLs cannot be expressed as a `duckdb_value` — build those inline in SQL instead.
//! - **Identifiers** (table / column names) cannot be placeholders in SQL and must be spliced into
//!   the text, which is what [`quote_identifier`] escapes (double quotes); literals use
//!   [`quote_literal`].
//!
//! # 代价与限制 / Cost and limits
//!
//! - **结果整份物化进内存**：`query` 把每个 chunk 立刻读成动态行
//!   （[`DuckDynamicRow::read_batch`](crate::DuckDynamicRow::read_batch)）。
//!   这是为了不把 `duckdb_result` 的句柄带出这条连接；需要边读边处理的大结果请改用
//!   「手写适配器 + 自持连接」那条路。
//! - **一次调用从头到尾持锁**，所以长查询会挡住同一把锁上的其它取用；两个 feature 都开时，那把锁也
//!   是 `duckfn::duck_vfs` 的 —— 同一线程里**绝不能嵌套取用**（在 `with_file_system` 闭包里调本模块
//!   会死锁，反之亦然）。并发取用之间串行化。
//! - **前置条件**：注册期捕获成功（扩展正常 `LOAD` 即可）—— 捕获失败时每次调用都报出记录下来的原因。
//!   除了这次捕获，**没有版本要求**：见上面的「只用 `sql` feature」。
//! - **不能在「同一线程里同步执行 DuckDB」的运行时里用**：注册期就在当前线程上开了连接并重入引擎，
//!   已知一处是 DuckDB-Wasm 的 Node blocking 绑定。
//!
//! - **The whole result is materialised in memory**: `query` reads every chunk into dynamic rows
//!   right away ([`DuckDynamicRow::read_batch`](crate::DuckDynamicRow::read_batch)), precisely so no
//!   `duckdb_result` handle escapes that connection. For a large result that must be processed as it
//!   streams, hand-write an adapter and keep your own connection instead.
//! - **A call holds the lock from start to finish**, so a long query blocks every other take-up on
//!   it; with both features on that is `duckfn::duck_vfs`'s lock too, and nesting take-ups on one
//!   thread **deadlocks** (calling this from inside a `with_file_system` closure, and the other way
//!   round). Concurrent take-ups serialize.
//! - **Prerequisites**: a successful registration-time capture (a normal extension `LOAD` is enough)
//!   — when capturing failed, every call reports the recorded reason. Beyond that capture there is
//!   **no version requirement**: see "Only the `sql` feature" above.
//! - **It cannot be used in a runtime that executes DuckDB synchronously on one thread**: the
//!   connection is opened, and the engine re-entered, on the current thread during registration; the
//!   one known case is DuckDB-Wasm's Node blocking bindings.
//!
//! # 用法 / Usage
//!
//! ```ignore
//! use duckfn::sql;
//!
//! // 影响行数（DDL 是 0）
//! let changed = sql::execute("INSERT INTO t VALUES (1), (2)")?;
//!
//! // 查询：schema 与行都来自 SQL 自己
//! let result = sql::query("SELECT 42 AS answer, [1, 2] AS tags")?;
//! assert_eq!(result.len(), 1);
//! assert_eq!(result.cell(0, "answer")?.map(ToString::to_string).as_deref(), Some("42"));
//!
//! // 绑值：表名这类标识符走 quote_identifier
//! let statement = format!("SELECT count(*) FROM {}", sql::quote_identifier("t"));
//! let count = sql::query(&statement)?.single_value()?;
//! ```

use crate::{DuckDynamicValue, DuckResult, duck_error};
use quack_rs::error::ExtensionError;

mod bind;
mod result;

pub use result::DuckQueryResult;

use crate::owned_connection::with_connection;

/// 执行 SQL 语句（可含多条，用 `;` 分隔），返回影响行数。
///
/// Runs SQL statements (several, separated by `;`, are allowed) and returns the number of rows
/// changed.
///
/// 语句按顺序全部执行；返回的是**产出结果的那一条**的影响行数 —— 第一条产出行的语句，若都不产出
/// 行则是最后一条（`CREATE` / `SET` 之类因此返回 0）。某条失败即返回错误，但它之前的语句已经执行
/// 并提交（除非在显式事务里）。详见 quack-rs `query::query` 的说明。
///
/// Every statement runs, in order; the count is that of **the statement whose result is returned** —
/// the first one that produces rows, or else the last (`CREATE` / `SET` and friends therefore report
/// 0). A failing statement fails the call, but the statements before it have already run and
/// committed (outside an explicit transaction). See quack-rs' `query::query` for the details.
///
/// # Errors
///
/// 还没捕获到连接、语句失败、或 SQL 含内嵌 NUL 字节时返回错误（消息带 `duckfn::sql::execute:`
/// 前缀与 DuckDB 自己给的原因）。
///
/// Returns an error when no connection has been captured, when a statement fails, or when the SQL
/// contains an interior NUL byte (the message carries a `duckfn::sql::execute:` prefix and DuckDB's
/// own reason).
pub fn execute(sql: &str) -> DuckResult<u64> {
    with_connection(|connection| {
        connection
            .execute(sql)
            .map_err(|error| query_error("execute", error))
    })
}

/// 绑定参数后执行语句，返回影响行数；语义同 [`execute`]。
///
/// Executes statements with bound parameters, returning the number of rows changed; the semantics
/// match [`execute`].
///
/// `params` 依次绑到语句的 `?` 位置上，数量必须与语句的参数个数一致。**优先用它而不是把值拼进
/// SQL 文本**：值来自外部（表名、路径、用户输入）时，拼接就是注入。
///
/// `params` are bound to the statement's `?` placeholders in order; the count must match the
/// statement's parameter count. **Prefer this over splicing values into the SQL text**: when the
/// value comes from outside (a table name, a path, user input), splicing is an injection bug.
///
/// # Errors
///
/// 除 [`execute`] 的错误外：参数个数不匹配、某个值绑定失败（含无法绑定的嵌套值）时返回错误。
///
/// Besides [`execute`]'s errors: a parameter-count mismatch, or a value that cannot be bound
/// (including nested values, which are not bindable).
pub fn execute_with(sql: &str, params: &[DuckDynamicValue]) -> DuckResult<u64> {
    with_connection(|connection| {
        let statement = connection
            .prepare(sql)
            .map_err(|error| query_error("execute", error))?;
        bind::bind_all(&statement, params)?;
        statement
            .execute()
            .map(|result| result.rows_changed())
            .map_err(|error| query_error("execute", error))
    })
}

/// 执行一条查询，把结果整份读成动态结果集。
///
/// Runs one query and reads the whole result into a dynamic result set.
///
/// 列名与列类型取自结果本身，行按同一份描述读成 [`DuckDynamicRow`](crate::DuckDynamicRow)，
/// 因此 `LIST` / `STRUCT` / `MAP` / `DECIMAL` 都能原样带回来（见模块文档的「动态列」）。
///
/// Column names and types come from the result itself and the rows are read against that same
/// description into [`DuckDynamicRow`](crate::DuckDynamicRow), so `LIST` / `STRUCT` / `MAP` /
/// `DECIMAL` come back as they are (see "Dynamic columns" in the module docs).
///
/// # Errors
///
/// 还没捕获到连接、查询失败、SQL 含内嵌 NUL 字节、或某列的类型无法用动态列表达（`ENUM` /
/// `ARRAY` / `UNION` / `BIT`）时返回错误。
///
/// Returns an error when no connection has been captured, when the query fails, when the SQL
/// contains an interior NUL byte, or when a column's type cannot be expressed as a dynamic column
/// (`ENUM` / `ARRAY` / `UNION` / `BIT`).
pub fn query(sql: &str) -> DuckResult<DuckQueryResult> {
    with_connection(|connection| {
        let result = connection
            .query(sql)
            .map_err(|error| query_error("query", error))?;
        DuckQueryResult::read(result)
    })
}

/// 绑定参数后执行查询；语义同 [`query`]，参数规则同 [`execute_with`]。
///
/// Runs one query with bound parameters; the semantics match [`query`] and the parameter rules match
/// [`execute_with`].
///
/// 语句必须是**恰好一条**（预处理语句不支持多条），这一点与 [`query`] 不同。
///
/// The statement must be **exactly one** (a prepared statement cannot hold several), which is where
/// this differs from [`query`].
///
/// # Errors
///
/// 同 [`query`]，外加参数个数不匹配、值无法绑定时返回错误。
///
/// As [`query`], plus a parameter-count mismatch or an unbindable value.
pub fn query_with(sql: &str, params: &[DuckDynamicValue]) -> DuckResult<DuckQueryResult> {
    with_connection(|connection| {
        let statement = connection
            .prepare(sql)
            .map_err(|error| query_error("query", error))?;
        bind::bind_all(&statement, params)?;
        let result = statement
            .execute()
            .map_err(|error| query_error("query", error))?;
        DuckQueryResult::read(result)
    })
}

/// 把一个标识符（表名、列名）转义成可以安全拼进 SQL 双引号形式。
///
/// Escapes an identifier (table or column name) into a safely spliced double-quoted SQL form.
///
/// 内部的 `"` 双写（SQL 标准），首尾各加一个 `"` —— 因此 `dfn_t` → `"dfn_t"`，
/// 形如 `a"b` 的名字也拼不出额外的语法。SQL 里的标识符不能用参数占位，所以「运行时才知道表名」
/// 只能拼文本，转义就是这一处的安全边界：
///
/// Inner `"` are doubled (the SQL standard) and both ends get one — so `dfn_t` → `"dfn_t"`, and a
/// name like `a"b` cannot break out of the quoting. Identifiers cannot be placeholders, so a
/// runtime-only table name has to be spliced; this is the escaping boundary for exactly that.
///
/// ```ignore
/// let sql = format!("SELECT count(*) FROM {}", duckfn::sql::quote_identifier(table));
/// ```
#[must_use]
pub fn quote_identifier(name: &str) -> String {
    quote_with(name, '"')
}

/// 把一个字符串转义成可以安全拼进 SQL 的单引号字面量。
///
/// Escapes a string into a safely spliced single-quoted SQL literal.
///
/// 内部的 `'` 双写（SQL 标准），首尾各加一个 `'`。**能绑值就绑值**（[`query_with`] /
/// [`execute_with`]），它只用于参数占位用不上的位置（例如 `SET` 的取值、`INSTALL '...'`）。
/// DuckDB 的普通字符串字面量不把反斜杠当转义（那是 `E'...'` 的语义），所以双写 `'` 就够。
///
/// Inner `'` are doubled (the SQL standard) and both ends get one. **Bind the value when you can**
/// ([`query_with`] / [`execute_with`]); this is for the places a placeholder cannot reach (the value
/// of a `SET`, `INSTALL '...'`, ...). DuckDB's plain string literals do not treat a backslash as an
/// escape (that is `E'...'`), so doubling `'` is enough.
#[must_use]
pub fn quote_literal(text: &str) -> String {
    quote_with(text, '\'')
}

/// [`quote_identifier`] / [`quote_literal`] 的共同实现：首尾加引号、内部同字符双写。
///
/// The shared implementation of [`quote_identifier`] / [`quote_literal`]: wrap in the quote and
/// double it inside.
fn quote_with(text: &str, quote: char) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for character in text.chars() {
        if character == quote {
            out.push(quote);
        }
        out.push(character);
    }
    out.push(quote);
    out
}

/// 给底层错误加上「这是 `duckfn::sql` 在做哪一步」的前缀，便于 sqllogictest 稳定断言。
///
/// Prefixes an underlying error with which `duckfn::sql` step produced it, so sqllogictest
/// assertions stay stable.
pub(crate) fn query_error(operation: &str, error: ExtensionError) -> ExtensionError {
    duck_error(format!("duckfn::sql::{operation}: {}", error.as_str()))
}

#[cfg(test)]
mod tests {
    use super::{quote_identifier, quote_literal, query_error};
    use crate::duck_error;

    // 只测纯逻辑：真正跑 SQL 需要 DuckDB 运行时（见 `test/sql/functions/sql_exec.test`），
    // 单测不去碰 FFI。
    //
    // Pure logic only: actually running SQL needs the DuckDB runtime (see
    // `test/sql/functions/sql_exec.test`), so unit tests stay away from FFI.

    #[test]
    fn identifiers_are_double_quoted_with_inner_quotes_doubled() {
        assert_eq!(quote_identifier("dfn_t"), "\"dfn_t\"");
        assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_identifier(""), "\"\"");
        // 单引号在标识符里不特殊，原样保留。
        //
        // A single quote is not special inside an identifier and stays as it is.
        assert_eq!(quote_identifier("a'b"), "\"a'b\"");
    }

    #[test]
    fn literals_are_single_quoted_with_inner_quotes_doubled() {
        assert_eq!(quote_literal("hi"), "'hi'");
        assert_eq!(quote_literal("it's"), "'it''s'");
        assert_eq!(quote_literal(""), "''");
        // 双引号在字面量里不特殊，原样保留。
        //
        // A double quote is not special inside a literal and stays as it is.
        assert_eq!(quote_literal("a\"b"), "'a\"b'");
    }

    #[test]
    fn query_errors_carry_the_operation_as_a_prefix() {
        let message = query_error("query", duck_error("Parser Error: boom"));
        assert_eq!(message.as_str(), "duckfn::sql::query: Parser Error: boom");
    }
}
