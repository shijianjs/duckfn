use duckfn::duck_table_function;
use duckfn::{DuckArray, DuckBlob, DuckDate, DuckDecimal, DuckResult, DuckStruct};

use super::table_echo_util::{echo_rows, option_to_array, option_to_array_with, to_array};

// ============================================================================
// ARRAY 的表函数回显（duck_array_scalar_echo.rs 的表函数版本）
//
// 关键差异：ARRAY 不能作为表函数参数 ——
// src/value_types/duck_array.rs::read_by_duck_value_valid 直接返回
// "Bind value to array type is not supported"。
// 所以下面 ARRAY(_, N) 的入参统一用等价的 LIST，在 bind 阶段做定长校验
// （table_echo_util.rs::to_array），出参才是真正的 ARRAY(N) 列。
// 唯一保留 ARRAY 入参的是 dfn_table_echo_array_integer_param，用来固化那条
// 未实现的读取路径。
//
// 出参写路径：ArrayVector::get_child + 下标 idx * N + i，
// 一批里混着「有值 / NULL」行时，NULL 行只置父向量 validity、不动 child。
// ============================================================================

/// ARRAY(INTEGER, 3)：入参 LIST(INTEGER)，长度必须正好 3
/// ```sql
/// SELECT v FROM dfn_table_echo_array_integer([1, 2, 3]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayIntegerRow {
    pub v: Option<DuckArray<i32, 3>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_integer(
    v: Option<Vec<i32>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayIntegerRow>> {
    let v = option_to_array::<i32, 3>("dfn_table_echo_array_integer", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayIntegerRow { v }))
}

/// ARRAY(BIGINT, 2)：入参 LIST(BIGINT)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_bigint([1, 2]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayBigintRow {
    pub v: Option<DuckArray<i64, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_bigint(
    v: Option<Vec<i64>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayBigintRow>> {
    let v = option_to_array::<i64, 2>("dfn_table_echo_array_bigint", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayBigintRow { v }))
}

/// ARRAY(DOUBLE, 3)：入参 LIST(DOUBLE)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_double([1.5, -2.25, 0.0]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayDoubleRow {
    pub v: Option<DuckArray<f64, 3>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_double(
    v: Option<Vec<f64>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayDoubleRow>> {
    let v = option_to_array::<f64, 3>("dfn_table_echo_array_double", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayDoubleRow { v }))
}

/// ARRAY(BOOLEAN, 2)：入参 LIST(BOOLEAN)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_bool([true, false]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayBoolRow {
    pub v: Option<DuckArray<bool, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_bool(
    v: Option<Vec<bool>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayBoolRow>> {
    let v = option_to_array::<bool, 2>("dfn_table_echo_array_bool", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayBoolRow { v }))
}

/// ARRAY(VARCHAR, 2)：入参 LIST(VARCHAR)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_varchar(['a', 'bb']);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayVarcharRow {
    pub v: Option<DuckArray<String, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_varchar(
    v: Option<Vec<String>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayVarcharRow>> {
    let v = option_to_array::<String, 2>("dfn_table_echo_array_varchar", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayVarcharRow { v }))
}

/// ARRAY(DATE, 2)：入参 LIST(DATE)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_date([DATE '2024-01-02', DATE '1969-12-31']);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayDateRow {
    pub v: Option<DuckArray<DuckDate, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_date(
    v: Option<Vec<DuckDate>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayDateRow>> {
    let v = option_to_array::<DuckDate, 2>("dfn_table_echo_array_date", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayDateRow { v }))
}

/// ARRAY(DECIMAL(18,3), 2)：入参 LIST(DECIMAL(18,3))
/// ```sql
/// SELECT v FROM dfn_table_echo_array_decimal(
///     [1.234::DECIMAL(18,3), (-1.234)::DECIMAL(18,3)]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayDecimalRow {
    pub v: Option<DuckArray<DuckDecimal<18, 3>, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_decimal(
    v: Option<Vec<DuckDecimal<18, 3>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayDecimalRow>> {
    let v = option_to_array::<DuckDecimal<18, 3>, 2>("dfn_table_echo_array_decimal", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayDecimalRow { v }))
}

/// ARRAY(BLOB, 2)：入参 LIST(BLOB)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_blob(['\xAA\xBB'::BLOB, ''::BLOB]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayBlobRow {
    pub v: Option<DuckArray<DuckBlob, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_blob(
    v: Option<Vec<DuckBlob>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayBlobRow>> {
    let v = option_to_array::<DuckBlob, 2>("dfn_table_echo_array_blob", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayBlobRow { v }))
}

/// ARRAY(INTEGER, 2) 的 ARRAY，即 INTEGER[2][2]：入参 LIST(LIST(INTEGER))
/// ```sql
/// SELECT v FROM dfn_table_echo_array_nested([[1, 2], [3, 4]]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayNestedRow {
    pub v: Option<DuckArray<DuckArray<i32, 2>, 2>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_nested(
    v: Option<Vec<Vec<i32>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayNestedRow>> {
    // 内层 LIST(INTEGER) 先转成 ARRAY(INTEGER, 2)，外层再做一次定长校验
    let v = option_to_array_with("dfn_table_echo_array_nested", v, |item| {
        to_array::<i32, 2>("dfn_table_echo_array_nested", item)
    })?;
    Ok(echo_rows(v, count, |v| TableEchoArrayNestedRow { v }))
}

/// ARRAY(INTEGER, 3)，元素可空：入参 LIST(INTEGER)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_integer_n([1, NULL, 3]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayIntegerNRow {
    pub v: Option<[Option<i32>; 3]>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_integer_n(
    v: Option<Vec<Option<i32>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayIntegerNRow>> {
    let v = option_to_array::<Option<i32>, 3>("dfn_table_echo_array_integer_n", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayIntegerNRow { v }))
}

/// ARRAY(VARCHAR, 2)，元素可空：入参 LIST(VARCHAR)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_varchar_n(['a', NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayVarcharNRow {
    pub v: Option<[Option<String>; 2]>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_varchar_n(
    v: Option<Vec<Option<String>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayVarcharNRow>> {
    let v = option_to_array::<Option<String>, 2>("dfn_table_echo_array_varchar_n", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayVarcharNRow { v }))
}

/// ARRAY(DATE, 2)，元素可空：入参 LIST(DATE)
/// ```sql
/// SELECT v FROM dfn_table_echo_array_date_n([DATE '2024-01-02', NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayDateNRow {
    pub v: Option<[Option<DuckDate>; 2]>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_date_n(
    v: Option<Vec<Option<DuckDate>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayDateNRow>> {
    let v = option_to_array::<Option<DuckDate>, 2>("dfn_table_echo_array_date_n", v)?;
    Ok(echo_rows(v, count, |v| TableEchoArrayDateNRow { v }))
}

/// ARRAY(INTEGER, 2) 的 ARRAY，元素可空：入参 LIST(LIST(INTEGER))
/// ```sql
/// SELECT v FROM dfn_table_echo_array_nested_n([[1, NULL], NULL]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayNestedNRow {
    pub v: Option<[Option<[Option<i32>; 2]>; 2]>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_nested_n(
    v: Option<Vec<Option<Vec<Option<i32>>>>>,
    count: Option<i64>,
) -> DuckResult<impl Iterator<Item = TableEchoArrayNestedNRow>> {
    // 内层 LIST(INTEGER) 先转成 ARRAY(INTEGER, 2)，外层再做一次定长校验
    let v = option_to_array_with("dfn_table_echo_array_nested_n", v, |item| match item {
        Some(item) => Ok(Some(to_array::<Option<i32>, 2>(
            "dfn_table_echo_array_nested_n",
            item,
        )?)),
        None => Ok(None),
    })?;
    Ok(echo_rows(v, count, |v| TableEchoArrayNestedNRow { v }))
}

/// ARRAY(INTEGER, 3) 作为 **参数**：读取路径未实现
///
/// duck_array.rs::read_by_duck_value_valid 返回
/// "Bind value to array type is not supported"，
/// 所以非 NULL 的 ARRAY 入参一定在 bind 阶段失败；
/// Value 本身是 NULL 时不进读取路径，可以正常回显 NULL 行。
/// ```sql
/// SELECT v FROM dfn_table_echo_array_integer_param(NULL::INTEGER[3]);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoArrayIntegerParamRow {
    pub v: Option<DuckArray<i32, 3>>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_array_integer_param(
    v: Option<DuckArray<i32, 3>>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoArrayIntegerParamRow> {
    echo_rows(v, count, |v| TableEchoArrayIntegerParamRow { v })
}
