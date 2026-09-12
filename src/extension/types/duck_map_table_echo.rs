use duckfn::duck_table_function;
use duckfn::{DuckBlob, DuckDate, DuckDecimal, DuckStruct};
use indexmap::IndexMap;

use super::table_echo_util::echo_rows;

// ============================================================================
// MAP 的表函数回显（duck_map_scalar_echo.rs 的表函数版本）
//
// MAP 物理上是 LIST<STRUCT<key, value>>，所以出参同样走
// ListVector 的 reserve/set_entry + keys/values 两条子向量，
// 一批里混着「有值 / NULL」行时 offset 只能按有值行累加。
//
// 标量版本的 NULL value 语义：
//   IndexMap<K, V>         -> value 不能是 NULL，整个 MAP 变 NULL
//   IndexMap<K, Option<V>> -> NULL value 原样保留
// 表函数的 bind 值读取没有 Option 可用，所以 IndexMap<K, V> 遇到 NULL value
// 会直接报 "Map value cannot be null" —— 用例里会固化这个差异。
//
// key 侧：DuckDB 的 MAP key 不允许为 NULL，两个实现都直接是 IndexMap<K, _>。
// ============================================================================

/// MAP(VARCHAR, INTEGER) // IndexMap<String, i32>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_integer(map(['a', 'b'], [1, 2]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharIntegerRow {
    pub v: Option<IndexMap<String, i32>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_integer(
    v: Option<IndexMap<String, i32>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharIntegerRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharIntegerRow { v })
}

/// MAP(INTEGER, VARCHAR) // IndexMap<i32, String>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_integer_varchar(map([1, 2], ['a', 'b']));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapIntegerVarcharRow {
    pub v: Option<IndexMap<i32, String>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_integer_varchar(
    v: Option<IndexMap<i32, String>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapIntegerVarcharRow> {
    echo_rows(v, count, |v| TableEchoMapIntegerVarcharRow { v })
}

/// MAP(BIGINT, BIGINT) // IndexMap<i64, i64>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_bigint_bigint(
///     map([-1, 1], [(-9223372036854775808), 9223372036854775807]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapBigintBigintRow {
    pub v: Option<IndexMap<i64, i64>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_bigint_bigint(
    v: Option<IndexMap<i64, i64>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapBigintBigintRow> {
    echo_rows(v, count, |v| TableEchoMapBigintBigintRow { v })
}

/// MAP(INTEGER, DOUBLE) // IndexMap<i32, f64>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_integer_double(map([1, 2], [1.5, -2.25]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapIntegerDoubleRow {
    pub v: Option<IndexMap<i32, f64>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_integer_double(
    v: Option<IndexMap<i32, f64>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapIntegerDoubleRow> {
    echo_rows(v, count, |v| TableEchoMapIntegerDoubleRow { v })
}

/// MAP(VARCHAR, DATE) // IndexMap<String, DuckDate>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_date(
///     map(['a'], [DATE '2024-01-02']));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharDateRow {
    pub v: Option<IndexMap<String, DuckDate>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_date(
    v: Option<IndexMap<String, DuckDate>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharDateRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharDateRow { v })
}

/// MAP(VARCHAR, DECIMAL(18,3)) // IndexMap<String, DuckDecimal<18, 3>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_decimal(
///     map(['a'], [1.234::DECIMAL(18,3)]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharDecimalRow {
    pub v: Option<IndexMap<String, DuckDecimal<18, 3>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_decimal(
    v: Option<IndexMap<String, DuckDecimal<18, 3>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharDecimalRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharDecimalRow { v })
}

/// MAP(VARCHAR, BLOB) // IndexMap<String, DuckBlob>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_blob(
///     map(['a'], ['\xAA\xBB'::BLOB]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharBlobRow {
    pub v: Option<IndexMap<String, DuckBlob>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_blob(
    v: Option<IndexMap<String, DuckBlob>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharBlobRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharBlobRow { v })
}

/// MAP(VARCHAR, MAP(VARCHAR, INTEGER)) // IndexMap<String, IndexMap<String, i32>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_nested(
///     map(['a'], [map(['x'], [1])]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapNestedRow {
    pub v: Option<IndexMap<String, IndexMap<String, i32>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_nested(
    v: Option<IndexMap<String, IndexMap<String, i32>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapNestedRow> {
    echo_rows(v, count, |v| TableEchoMapNestedRow { v })
}

/// MAP(VARCHAR, INTEGER)，value 可空 // IndexMap<String, Option<i32>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_integer_n(map(['a', 'b'], [1, NULL]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharIntegerNRow {
    pub v: Option<IndexMap<String, Option<i32>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_integer_n(
    v: Option<IndexMap<String, Option<i32>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharIntegerNRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharIntegerNRow { v })
}

/// MAP(INTEGER, VARCHAR)，value 可空 // IndexMap<i32, Option<String>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_integer_varchar_n(map([1, 2], ['a', NULL]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapIntegerVarcharNRow {
    pub v: Option<IndexMap<i32, Option<String>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_integer_varchar_n(
    v: Option<IndexMap<i32, Option<String>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapIntegerVarcharNRow> {
    echo_rows(v, count, |v| TableEchoMapIntegerVarcharNRow { v })
}

/// MAP(VARCHAR, DATE)，value 可空 // IndexMap<String, Option<DuckDate>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_date_n(
///     map(['a', 'b'], [DATE '2024-01-02', NULL]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharDateNRow {
    pub v: Option<IndexMap<String, Option<DuckDate>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_date_n(
    v: Option<IndexMap<String, Option<DuckDate>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharDateNRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharDateNRow { v })
}

/// MAP(VARCHAR, DECIMAL(18,3))，value 可空
/// // IndexMap<String, Option<DuckDecimal<18, 3>>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_varchar_decimal_n(
///     map(['a', 'b'], [1.234::DECIMAL(18,3), NULL]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoMapVarcharDecimalNRow {
    pub v: Option<IndexMap<String, Option<DuckDecimal<18, 3>>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_map_varchar_decimal_n(
    v: Option<IndexMap<String, Option<DuckDecimal<18, 3>>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapVarcharDecimalNRow> {
    echo_rows(v, count, |v| TableEchoMapVarcharDecimalNRow { v })
}

/// MAP(VARCHAR, MAP(VARCHAR, INTEGER))，value 和嵌套 value 都可空
/// // IndexMap<String, Option<IndexMap<String, Option<i32>>>>
/// ```sql
/// SELECT v FROM dfn_table_echo_map_nested_n(
///     map(['a'], [map(['x'], [1, NULL])]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
#[allow(clippy::type_complexity)]
pub struct TableEchoMapNestedNRow {
    pub v: Option<IndexMap<String, Option<IndexMap<String, Option<i32>>>>>,
}

#[duck_table_function(named_param_from = "count")]
#[allow(clippy::type_complexity)]
fn dfn_table_echo_map_nested_n(
    v: Option<IndexMap<String, Option<IndexMap<String, Option<i32>>>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoMapNestedNRow> {
    echo_rows(v, count, |v| TableEchoMapNestedNRow { v })
}
