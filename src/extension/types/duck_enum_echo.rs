use duckfn::{DuckEnum, DuckStruct, duck_scalar_function, duck_table_function};

use super::table_echo_util::echo_rows;

// ============================================================================
// 由 `#[derive(DuckEnum)]` 生成的 ENUM（与手写的 Color 对照，见 custom_type_echo.rs）
//
// ENUM 的规则很固定 —— 逻辑类型带字典、向量里只存下标、bind 阶段给标签 —— 所以这些样板
// 全部交给 derive：
//   - 字典 = 变体声明顺序（可用 `rename_all` / 变体级 `#[duck(rename = ...)]` 调整标签）；
//   - 读：从下标数组取下标 → 变体；写：把变体下标写回向量；
//   - bind 阶段：`Value::as_str()` 取标签再匹配回来。
//
// `#[duck(sql_name = "priority", create_type = true)]` 额外让扩展在加载期执行一次
// `CREATE TYPE IF NOT EXISTS "priority" AS ENUM ('low', 'medium', 'high');` —— 走的正是 SQL 宏
// 那条执行路径（`duckdb_query`）。建好之后 SQL 侧就能直接写 `'high'::priority`，
// 也能把表的列声明成 `priority`；语句是幂等的，重复 `LOAD` 不会报错。
//
// 注意：作为**非可空**函数参数时还需要 `Default`（宏生成的参数结构体会 `derive(Default)`），
// 所以这里照例带上 `#[derive(Default)]` + `#[default]`；可空参数写 `Option<Priority>` 则不需要。
// ============================================================================

/// DuckDB `priority` 类型：`ENUM('low', 'medium', 'high')`，并在加载期建进 catalog。
///
/// The DuckDB `priority` type: `ENUM('low', 'medium', 'high')`, created in the catalog at load
/// time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "lowercase", sql_name = "priority", create_type = true)]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}

/// ```sql
/// SELECT dfn_echo_priority('high'::priority);
/// ```
#[duck_scalar_function]
fn dfn_echo_priority(p: Priority) -> Priority {
    p
}

/// 可空版本：`Option<Priority>` 的 NULL 语义与其它类型一致。
///
/// ```sql
/// SELECT dfn_echo_priority_n(NULL::priority);
/// ```
#[duck_scalar_function]
fn dfn_echo_priority_n(p: Option<Priority>) -> Option<Priority> {
    p
}

/// derive 出来的 ENUM 也能直接当 `STRUCT` 字段。
///
/// A derived ENUM works as a `STRUCT` field as well.
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,
    pub tag: Option<Priority>,
}

/// ```sql
/// SELECT (dfn_echo_ticket({'id': 1, 'priority': 'high'::priority, 'tag': NULL})).priority;
/// ```
#[duck_scalar_function]
fn dfn_echo_ticket(t: Ticket) -> Ticket {
    t
}

/// ```sql
/// SELECT v FROM dfn_table_echo_priority('medium'::priority, count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoPriorityRow {
    pub v: Option<Priority>,
}

/// 表函数：入参走 bind 阶段的 `duckdb_value`（即 derive 生成的标签匹配），出参是 ENUM 列。
///
/// A table function: the argument arrives as a bind-time `duckdb_value` (i.e. the derive's label
/// lookup) and the output column is an ENUM.
#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_priority(
    v: Option<Priority>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoPriorityRow> {
    echo_rows(v, count, |v| TableEchoPriorityRow { v })
}

// ============================================================================
// 命名规则：`rename_all` + 单变体 `#[duck(rename = ...)]`
//
// 这个枚举没有写 `create_type`，所以它**不会**在 catalog 里建类型 —— 默认就是「只做映射」。
// 字典为 `['HTTP', 'GRPC', 'socket']`。
// ============================================================================

/// SCREAMING_SNAKE_CASE 命名 + 单个变体改名。
///
/// SCREAMING_SNAKE_CASE naming plus a per-variant rename.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Channel {
    #[default]
    Http,
    Grpc,
    /// 单个变体覆盖命名规则。
    ///
    /// A single variant overriding the rename rule.
    #[duck(rename = "socket")]
    UnixSocket,
}

/// ```sql
/// SELECT dfn_echo_channel('HTTP');
/// ```
#[duck_scalar_function]
fn dfn_echo_channel(c: Channel) -> Channel {
    c
}

// ============================================================================
// create_type = "print"：加载期只收集建类型的 DDL，不建类型
//
// `"print"` 渲染的是与 `create_type = true` 完全相同的那条语句（同一个
// `duckfn::named_type_ddl`），只是收进队列、不执行；等全部注册项跑完，入口点
// （`duckfn::register_all_duckfn`）把这一批一次性打到 stderr —— 可以先看看宏会生成什么
// SQL、再决定要不要真的建类型。所以下面的 `severity` **不会**出现在 `duckdb_types()` 里。
// ============================================================================

/// 打印模式：`#[duck(create_type = "print")]`。
///
/// 扩展 `LOAD` 结束时会和其它 `"print"` 类型一起打印
/// `CREATE TYPE IF NOT EXISTS "severity" AS ENUM('info', 'warn', 'error');`，但不建类型。
///
/// Print mode: once the extension has loaded, `CREATE TYPE IF NOT EXISTS "severity" AS
/// ENUM('info', 'warn', 'error');` shows up in the batch printed to stderr, without creating the
/// type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "lowercase", sql_name = "severity", create_type = "print")]
pub enum Severity {
    #[default]
    Info,
    Warn,
    Error,
}

/// 打印模式只影响「建不建类型」：枚举作为值类型照常可用（常量字符串会被隐式转换过来）。
///
/// ```sql
/// SELECT dfn_echo_severity('warn');
/// ```
#[duck_scalar_function]
fn dfn_echo_severity(s: Severity) -> Severity {
    s
}
