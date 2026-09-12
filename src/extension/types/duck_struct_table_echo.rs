use duckfn::duck_table_function;
use duckfn::{DuckArray, DuckOptionArray, DuckResult, DuckStruct};
use indexmap::IndexMap;

use super::duck_struct_scalar_echo::{
    DuckStructAllNullable, DuckStructInner, DuckStructNested, DuckStructNestedOnly,
    DuckStructNullable, DuckStructSimple, DuckStructSimpleTypes, DuckStructWithArray,
    DuckStructWithList, DuckStructWithListN, DuckStructWithMap, DuckStructWrapper,
};
use super::table_echo_util::{echo_rows, option_to_array, to_array};

// ============================================================================
// STRUCT 的表函数回显（duck_struct_scalar_echo.rs 的表函数版本）
//
// 标量版本按行读父向量的 children，参数被折成常量向量时 child 只有 1 个物理
// 元素，会踩到 duckdb#25616（见 duck_struct_scalar_echo.test 的「已知缺陷」）；
// 表函数的参数在 bind 阶段从 Value 读（value.struct_child），
// 不按下标访问子向量，所以这些用例可以直接写单行字面量。
//
// 出参是 STRUCT 列：s_write_columns_batch 逐行写字段，
// s_write_null 需要递归把子字段一并置 NULL（duck_struct.rs / duck_struct_derive.rs），
// 一批里混着「有值 / NULL」行时正好覆盖这条路径。
//
// STRUCT 里带 ARRAY 字段的那一个（dfn_table_echo_struct_with_array）
// 入参换成 LIST 版同形结构：ARRAY 不能作为表函数参数。
// ============================================================================

/// STRUCT(id INTEGER, name VARCHAR) // DuckStructSimple
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_simple({'id': 1, 'name': 'a'});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructSimpleRow {
    pub v: Option<DuckStructSimple>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_simple(
    v: Option<DuckStructSimple>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructSimpleRow> {
    echo_rows(v, count, |v| TableEchoStructSimpleRow { v })
}

/// STRUCT(b BOOLEAN, t TINYINT, s SMALLINT, i INTEGER, l BIGINT, f FLOAT, d DOUBLE, v VARCHAR)
/// // DuckStructSimpleTypes
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_simple_types(
///     {'b': true, 't': 1::TINYINT, 's': 2::SMALLINT, 'i': 3::INTEGER,
///      'l': 4::BIGINT, 'f': 1.5::FLOAT, 'd': 2.5::DOUBLE, 'v': 'x'});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructSimpleTypesRow {
    pub v: Option<DuckStructSimpleTypes>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_simple_types(
    v: Option<DuckStructSimpleTypes>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructSimpleTypesRow> {
    echo_rows(v, count, |v| TableEchoStructSimpleTypesRow { v })
}

/// STRUCT(d DATE, ts TIMESTAMP, dec DECIMAL(18,3), bl BLOB) // DuckStructWrapper
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_wrapper(
///     {'d': DATE '2024-01-02', 'ts': TIMESTAMP '2024-01-02 03:04:05',
///      'dec': 1.234::DECIMAL(18,3), 'bl': '\xAA\xBB'::BLOB});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructWrapperRow {
    pub v: Option<DuckStructWrapper>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_wrapper(
    v: Option<DuckStructWrapper>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructWrapperRow> {
    echo_rows(v, count, |v| TableEchoStructWrapperRow { v })
}

/// STRUCT(id INTEGER, age INTEGER, name VARCHAR) // DuckStructNullable
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_nullable({'id': 1, 'age': NULL, 'name': NULL});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructNullableRow {
    pub v: Option<DuckStructNullable>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_nullable(
    v: Option<DuckStructNullable>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructNullableRow> {
    echo_rows(v, count, |v| TableEchoStructNullableRow { v })
}

/// STRUCT(id INTEGER, name VARCHAR, data INTEGER[]) // DuckStructAllNullable
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_all_nullable({'id': NULL, 'name': NULL, 'data': NULL});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructAllNullableRow {
    pub v: Option<DuckStructAllNullable>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_all_nullable(
    v: Option<DuckStructAllNullable>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructAllNullableRow> {
    echo_rows(v, count, |v| TableEchoStructAllNullableRow { v })
}

/// STRUCT(id INTEGER, data INTEGER[], tags VARCHAR[]) // DuckStructWithList
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_with_list({'id': 1, 'data': [1, 2, 3], 'tags': ['a', 'b']});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructWithListRow {
    pub v: Option<DuckStructWithList>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_with_list(
    v: Option<DuckStructWithList>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructWithListRow> {
    echo_rows(v, count, |v| TableEchoStructWithListRow { v })
}

/// STRUCT(id INTEGER, data INTEGER[])，列表和元素都可空 // DuckStructWithListN
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_with_list_n({'id': 1, 'data': [1, NULL, 3]});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructWithListNRow {
    pub v: Option<DuckStructWithListN>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_with_list_n(
    v: Option<DuckStructWithListN>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructWithListNRow> {
    echo_rows(v, count, |v| TableEchoStructWithListNRow { v })
}

/// STRUCT(id INTEGER, m MAP(VARCHAR, INTEGER)) // DuckStructWithMap
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_with_map({'id': 1, 'm': map(['a', 'b'], [1, 2])});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructWithMapRow {
    pub v: Option<DuckStructWithMap>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_with_map(
    v: Option<DuckStructWithMap>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructWithMapRow> {
    echo_rows(v, count, |v| TableEchoStructWithMapRow { v })
}

/// STRUCT(id INTEGER, arr INTEGER[])：DuckStructWithArray 的可读版本
/// （ARRAY 不能作为表函数参数，所以入参用 LIST 版同形结构）
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructWithArrayInput {
    pub id: i32,
    pub arr: Vec<i32>,
}

/// STRUCT(id INTEGER, arr INTEGER[3])：入参是同形的 LIST 版结构，长度必须正好 3
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_with_array({'id': 1, 'arr': [1, 2, 3]});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructWithArrayRow {
    pub v: Option<DuckStructWithArray>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_with_array(
    v: Option<TableEchoStructWithArrayInput>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoStructWithArrayRow>> {
    let v = match v {
        Some(v) => {
            let arr = to_array::<i32, 3>("dfn_table_echo_struct_with_array", v.arr)?;
            Some(DuckStructWithArray { id: v.id, arr })
        }
        None => None,
    };
    Ok(echo_rows(v, count, |v| TableEchoStructWithArrayRow { v }))
}

/// STRUCT(id INTEGER, inner STRUCT(key VARCHAR, value INTEGER), maybe STRUCT(...))
/// // DuckStructNested
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_nested(
///     {'id': 1, 'inner': {'key': 'a', 'value': 2}, 'maybe': NULL});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructNestedRow {
    pub v: Option<DuckStructNested>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_nested(
    v: Option<DuckStructNested>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructNestedRow> {
    echo_rows(v, count, |v| TableEchoStructNestedRow { v })
}

/// STRUCT(key VARCHAR, value INTEGER) // DuckStructInner
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_inner({'key': 'a', 'value': 2});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructInnerRow {
    pub v: Option<DuckStructInner>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_inner(
    v: Option<DuckStructInner>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructInnerRow> {
    echo_rows(v, count, |v| TableEchoStructInnerRow { v })
}

/// STRUCT(id INTEGER, inner STRUCT(key VARCHAR, value INTEGER)) // DuckStructNestedOnly
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_nested_only(
///     {'id': 1, 'inner': {'key': 'a', 'value': 2}});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructNestedOnlyRow {
    pub v: Option<DuckStructNestedOnly>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_nested_only(
    v: Option<DuckStructNestedOnly>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructNestedOnlyRow> {
    echo_rows(v, count, |v| TableEchoStructNestedOnlyRow { v })
}

/// LIST(STRUCT(id INTEGER, name VARCHAR)) // Vec<DuckStructSimple>
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_list([{'id': 1, 'name': 'a'}, {'id': 2, 'name': 'b'}]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructListRow {
    pub v: Option<Vec<DuckStructSimple>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_list(
    v: Option<Vec<DuckStructSimple>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructListRow> {
    echo_rows(v, count, |v| TableEchoStructListRow { v })
}

/// LIST(STRUCT(id INTEGER, name VARCHAR))，元素可空 // Vec<Option<DuckStructSimple>>
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_list_nullable([{'id': 1, 'name': 'a'}, NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructListNullableRow {
    pub v: Option<Vec<Option<DuckStructSimple>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_list_nullable(
    v: Option<Vec<Option<DuckStructSimple>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructListNullableRow> {
    echo_rows(v, count, |v| TableEchoStructListNullableRow { v })
}

/// MAP(VARCHAR, STRUCT(id INTEGER, name VARCHAR)) // IndexMap<String, DuckStructSimple>
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_map_value(map(['k'], [{'id': 1, 'name': 'a'}]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructMapValueRow {
    pub v: Option<IndexMap<String, DuckStructSimple>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_map_value(
    v: Option<IndexMap<String, DuckStructSimple>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructMapValueRow> {
    echo_rows(v, count, |v| TableEchoStructMapValueRow { v })
}

/// MAP(VARCHAR, STRUCT(...))，value 可空 // IndexMap<String, Option<DuckStructSimple>>
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_map_value_nullable(
///     map(['k'], [NULL::STRUCT(id INTEGER, name VARCHAR)]));
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructMapValueNullableRow {
    pub v: Option<IndexMap<String, Option<DuckStructSimple>>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_map_value_nullable(
    v: Option<IndexMap<String, Option<DuckStructSimple>>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoStructMapValueNullableRow> {
    echo_rows(v, count, |v| TableEchoStructMapValueNullableRow { v })
}

/// ARRAY(STRUCT(id INTEGER, name VARCHAR), 2)：入参是对应的 LIST，长度必须正好 2
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_array([{'id': 1, 'name': 'a'}, {'id': 2, 'name': 'b'}]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructArrayRow {
    pub v: Option<DuckArray<DuckStructSimple, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_array(
    v: Option<Vec<DuckStructSimple>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoStructArrayRow>> {
    let v = option_to_array::<DuckStructSimple, 2>("dfn_table_echo_struct_array", v)?;
    Ok(echo_rows(v, count, |v| TableEchoStructArrayRow { v }))
}

/// ARRAY(STRUCT(id INTEGER, name VARCHAR), 2)，元素可空：入参是对应的 LIST
/// ```sql
/// SELECT v FROM dfn_table_echo_struct_array_nullable([{'id': 1, 'name': 'a'}, NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoStructArrayNullableRow {
    pub v: Option<DuckOptionArray<DuckStructSimple, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_struct_array_nullable(
    v: Option<Vec<Option<DuckStructSimple>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoStructArrayNullableRow>> {
    let v = option_to_array::<Option<DuckStructSimple>, 2>(
        "dfn_table_echo_struct_array_nullable",
        v,
    )?;
    Ok(echo_rows(v, count, |v| TableEchoStructArrayNullableRow {
        v,
    }))
}

