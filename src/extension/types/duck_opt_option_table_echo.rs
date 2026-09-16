use duckfn::duck_table_function;
use duckfn::{DuckDate, DuckStruct};

use super::duck_struct_scalar_echo::DuckStructSimple;
use super::table_echo_util::echo_rows;

// ============================================================================
// Option<Option<T>> 的表函数回显（duck_opt_option_scalar_echo.rs 的表函数版本）
//
// 双层 Option 与单层等价，这里把它放到「bind 参数 → 行结构体字段 → 输出列」这条
// 与标量函数不同的路径上验证：
//   - bind 阶段按 Value 读，没有 Option 可用，但 Option<Option<T>> 的
//     read_slot_by_duck_value 会把 NULL 变成 Some(None)，因此不会像 Vec<T> 那样
//     报 "Vec<T> value is None"；
//   - 写侧三条路径（leaf / 容器子向量 / struct 子字段）都由 Option<T> 的转发覆盖，
//     奇数行的 NULL 与字段本身的 NULL 都是同一套 write_null。
//
// 行数规则同其它 dfn_table_echo_*：count 缺省 1，偶数行写值、奇数行写 NULL。
// 所有取值都用 CAST(... AS VARCHAR) 包一层，避免驱动层对 LIST 的二次转换影响比对。
// ============================================================================

/// INTEGER // Option<Option<i32>>
/// ```sql
/// SELECT v FROM dfn_table_echo_opt_option_integer(42, count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoOptOptionIntegerRow {
    pub v: Option<Option<i32>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_opt_option_integer(
    v: Option<Option<i32>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoOptOptionIntegerRow> {
    echo_rows(v, count, |v| TableEchoOptOptionIntegerRow { v })
}

/// VARCHAR // Option<Option<String>>
/// ```sql
/// SELECT v FROM dfn_table_echo_opt_option_varchar('abc', count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoOptOptionVarcharRow {
    pub v: Option<Option<String>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_opt_option_varchar(
    v: Option<Option<String>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoOptOptionVarcharRow> {
    echo_rows(v, count, |v| TableEchoOptOptionVarcharRow { v })
}

/// DATE // Option<Option<DuckDate>>
/// ```sql
/// SELECT v FROM dfn_table_echo_opt_option_date(DATE '2024-01-02', count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoOptOptionDateRow {
    pub v: Option<Option<DuckDate>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_opt_option_date(
    v: Option<Option<DuckDate>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoOptOptionDateRow> {
    echo_rows(v, count, |v| TableEchoOptOptionDateRow { v })
}

/// LIST(INTEGER)，元素是双层 Option // Option<Vec<Option<Option<i32>>>>
/// ```sql
/// SELECT v FROM dfn_table_echo_opt_option_list([1, NULL, 3], count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoOptOptionListRow {
    pub v: Option<Vec<Option<Option<i32>>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_opt_option_list(
    v: Option<Vec<Option<Option<i32>>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoOptOptionListRow> {
    echo_rows(v, count, |v| TableEchoOptOptionListRow { v })
}

/// STRUCT(id INTEGER, name VARCHAR) // Option<Option<DuckStructSimple>>
/// ```sql
/// SELECT v FROM dfn_table_echo_opt_option_struct({'id': 1, 'name': 'a'}, count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoOptOptionStructRow {
    pub v: Option<Option<DuckStructSimple>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_opt_option_struct(
    v: Option<Option<DuckStructSimple>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoOptOptionStructRow> {
    echo_rows(v, count, |v| TableEchoOptOptionStructRow { v })
}
