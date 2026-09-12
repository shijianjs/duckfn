use duckfn::duck_table_function;
use duckfn::{DuckBlob, DuckDate, DuckDecimal, DuckStruct};

use super::table_echo_util::echo_rows;

// ============================================================================
// LIST 的表函数回显（duck_list_scalar_echo.rs 的表函数版本）
//
// 标量版本里 LIST 的 NULL 元素由元素类型决定（读侧用 Option 传播）：
//   Vec<T>         -> 元素不能是 NULL，整个列表变 NULL
//   Vec<Option<T>> -> NULL 元素原样保留
//
// 表函数的 bind 值读取没有 Option 可用（read_by_duck_value_valid 返回 DuckResult），
// 所以 Vec<T> 遇到 NULL 元素会直接报 "Vec<T> value is None"，
// 只有 Vec<Option<T>> 能保留 NULL —— 两条读取路径的语义差异在用例里都会固化。
//
// 出参走 ListVector 的 set_entry/reserve + write_finish，
// 且同一批里混着「有值 / NULL」行，offset 必须按有值行累加。
// ============================================================================

/// LIST(INTEGER) // Vec<i32>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_integer([1, 2, 3]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListIntegerRow {
    pub v: Option<Vec<i32>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_integer(
    v: Option<Vec<i32>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListIntegerRow> {
    echo_rows(v, count, |v| TableEchoListIntegerRow { v })
}

/// LIST(BIGINT) // Vec<i64>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_bigint([1, 2, 3]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListBigintRow {
    pub v: Option<Vec<i64>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_bigint(
    v: Option<Vec<i64>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListBigintRow> {
    echo_rows(v, count, |v| TableEchoListBigintRow { v })
}

/// LIST(DOUBLE) // Vec<f64>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_double([1.5, -2.25]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListDoubleRow {
    pub v: Option<Vec<f64>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_double(
    v: Option<Vec<f64>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListDoubleRow> {
    echo_rows(v, count, |v| TableEchoListDoubleRow { v })
}

/// LIST(BOOLEAN) // Vec<bool>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_bool([true, false]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListBoolRow {
    pub v: Option<Vec<bool>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_bool(
    v: Option<Vec<bool>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListBoolRow> {
    echo_rows(v, count, |v| TableEchoListBoolRow { v })
}

/// LIST(VARCHAR) // Vec<String>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_varchar(['a', 'bb']);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListVarcharRow {
    pub v: Option<Vec<String>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_varchar(
    v: Option<Vec<String>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListVarcharRow> {
    echo_rows(v, count, |v| TableEchoListVarcharRow { v })
}

/// LIST(DATE) // Vec<DuckDate>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_date([DATE '2024-01-02', DATE '1969-12-31']);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListDateRow {
    pub v: Option<Vec<DuckDate>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_date(
    v: Option<Vec<DuckDate>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListDateRow> {
    echo_rows(v, count, |v| TableEchoListDateRow { v })
}

/// LIST(DECIMAL(18,3)) // Vec<DuckDecimal<18, 3>>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_decimal(
///     [1.234::DECIMAL(18,3), (-1.234)::DECIMAL(18,3)]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListDecimalRow {
    pub v: Option<Vec<DuckDecimal<18, 3>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_decimal(
    v: Option<Vec<DuckDecimal<18, 3>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListDecimalRow> {
    echo_rows(v, count, |v| TableEchoListDecimalRow { v })
}

/// LIST(BLOB) // Vec<DuckBlob>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_blob(['\xAA\xBB'::BLOB, ''::BLOB]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListBlobRow {
    pub v: Option<Vec<DuckBlob>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_blob(
    v: Option<Vec<DuckBlob>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListBlobRow> {
    echo_rows(v, count, |v| TableEchoListBlobRow { v })
}

/// LIST(LIST(INTEGER)) // Vec<Vec<i32>>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_nested([[1, 2], [3]]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListNestedRow {
    pub v: Option<Vec<Vec<i32>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_nested(
    v: Option<Vec<Vec<i32>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListNestedRow> {
    echo_rows(v, count, |v| TableEchoListNestedRow { v })
}

/// LIST(INTEGER)，元素可空 // Vec<Option<i32>>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_integer_n([1, NULL, 3]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListIntegerNRow {
    pub v: Option<Vec<Option<i32>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_integer_n(
    v: Option<Vec<Option<i32>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListIntegerNRow> {
    echo_rows(v, count, |v| TableEchoListIntegerNRow { v })
}

/// LIST(VARCHAR)，元素可空 // Vec<Option<String>>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_varchar_n(['a', NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListVarcharNRow {
    pub v: Option<Vec<Option<String>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_varchar_n(
    v: Option<Vec<Option<String>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListVarcharNRow> {
    echo_rows(v, count, |v| TableEchoListVarcharNRow { v })
}

/// LIST(DATE)，元素可空 // Vec<Option<DuckDate>>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_date_n([DATE '2024-01-02', NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListDateNRow {
    pub v: Option<Vec<Option<DuckDate>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_date_n(
    v: Option<Vec<Option<DuckDate>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListDateNRow> {
    echo_rows(v, count, |v| TableEchoListDateNRow { v })
}

/// LIST(LIST(INTEGER))，列表和元素都可空 // Vec<Option<Vec<Option<i32>>>>
/// ```sql
/// SELECT v FROM dfn_table_echo_list_nested_n([[1, NULL], NULL, []]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoListNestedNRow {
    pub v: Option<Vec<Option<Vec<Option<i32>>>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_list_nested_n(
    v: Option<Vec<Option<Vec<Option<i32>>>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoListNestedNRow> {
    echo_rows(v, count, |v| TableEchoListNestedNRow { v })
}
