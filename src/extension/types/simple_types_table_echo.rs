use duckfn::{duck_table_function, DuckStruct};

use super::table_echo_util::echo_rows;

// ============================================================================
// 简单类型的表函数回显（simple_types_scalar_echo.rs 的表函数版本）
//
// 标量版本：参数按行从向量读（DuckValueType::read_valid），NULL 由 validity 决定；
// 表函数版本：参数在 bind 阶段从 Value 读（read_by_duck_value），
// 结果再按行写进结果向量 —— 读写两侧的机制都不一样，所以要单独测。
//
// 参数统一写成 Option<T>：
//   - 传 NULL 或不传 -> None -> 回显 NULL 行
//   - 传具体值 -> Some(v) -> 回显该值
// 行数规则见 table_echo_util.rs（默认 1 行，奇数行写 NULL）。
// ============================================================================

/// BOOLEAN // bool
/// ```sql
/// SELECT v FROM dfn_table_echo_bool(true);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoBoolRow {
    pub v: Option<bool>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_bool(
    v: Option<bool>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoBoolRow> {
    echo_rows(v, count, |v| TableEchoBoolRow { v })
}

/// TINYINT // i8
/// ```sql
/// SELECT v FROM dfn_table_echo_tinyint(-128::TINYINT);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoTinyintRow {
    pub v: Option<i8>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_tinyint(
    v: Option<i8>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoTinyintRow> {
    echo_rows(v, count, |v| TableEchoTinyintRow { v })
}

/// SMALLINT // i16
/// ```sql
/// SELECT v FROM dfn_table_echo_smallint(-32768::SMALLINT);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoSmallintRow {
    pub v: Option<i16>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_smallint(
    v: Option<i16>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoSmallintRow> {
    echo_rows(v, count, |v| TableEchoSmallintRow { v })
}

/// INTEGER // i32
/// ```sql
/// SELECT v FROM dfn_table_echo_integer(-2147483648);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoIntegerRow {
    pub v: Option<i32>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_integer(
    v: Option<i32>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoIntegerRow> {
    echo_rows(v, count, |v| TableEchoIntegerRow { v })
}

/// BIGINT // i64
/// ```sql
/// SELECT v FROM dfn_table_echo_bigint(-9223372036854775808);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoBigintRow {
    pub v: Option<i64>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_bigint(
    v: Option<i64>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoBigintRow> {
    echo_rows(v, count, |v| TableEchoBigintRow { v })
}

/// HUGEINT // i128
/// ```sql
/// SELECT v FROM dfn_table_echo_hugeint(170141183460469231731687303715884105727);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoHugeintRow {
    pub v: Option<i128>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_hugeint(
    v: Option<i128>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoHugeintRow> {
    echo_rows(v, count, |v| TableEchoHugeintRow { v })
}

/// UTINYINT // u8
/// ```sql
/// SELECT v FROM dfn_table_echo_utinyint(255::UTINYINT);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoUtinyintRow {
    pub v: Option<u8>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_utinyint(
    v: Option<u8>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoUtinyintRow> {
    echo_rows(v, count, |v| TableEchoUtinyintRow { v })
}

/// USMALLINT // u16
/// ```sql
/// SELECT v FROM dfn_table_echo_usmallint(65535::USMALLINT);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoUsmallintRow {
    pub v: Option<u16>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_usmallint(
    v: Option<u16>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoUsmallintRow> {
    echo_rows(v, count, |v| TableEchoUsmallintRow { v })
}

/// UINTEGER // u32
/// ```sql
/// SELECT v FROM dfn_table_echo_uinteger(4294967295::UINTEGER);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoUintegerRow {
    pub v: Option<u32>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_uinteger(
    v: Option<u32>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoUintegerRow> {
    echo_rows(v, count, |v| TableEchoUintegerRow { v })
}

/// UBIGINT // u64
/// ```sql
/// SELECT v FROM dfn_table_echo_ubigint(18446744073709551615::UBIGINT);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoUbigintRow {
    pub v: Option<u64>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_ubigint(
    v: Option<u64>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoUbigintRow> {
    echo_rows(v, count, |v| TableEchoUbigintRow { v })
}

/// UHUGEINT // u128
/// ```sql
/// SELECT v FROM dfn_table_echo_uhugeint(340282366920938463463374607431768211455);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoUhugeintRow {
    pub v: Option<u128>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_uhugeint(
    v: Option<u128>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoUhugeintRow> {
    echo_rows(v, count, |v| TableEchoUhugeintRow { v })
}

/// FLOAT // f32
/// ```sql
/// SELECT v FROM dfn_table_echo_float(1.5::FLOAT);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoFloatRow {
    pub v: Option<f32>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_float(
    v: Option<f32>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoFloatRow> {
    echo_rows(v, count, |v| TableEchoFloatRow { v })
}

/// DOUBLE // f64
/// ```sql
/// SELECT v FROM dfn_table_echo_double(-2.25::DOUBLE);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoDoubleRow {
    pub v: Option<f64>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_double(
    v: Option<f64>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoDoubleRow> {
    echo_rows(v, count, |v| TableEchoDoubleRow { v })
}

/// VARCHAR // String
/// ```sql
/// SELECT v FROM dfn_table_echo_varchar('你好');
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoVarcharRow {
    pub v: Option<String>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_varchar(
    v: Option<String>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoVarcharRow> {
    echo_rows(v, count, |v| TableEchoVarcharRow { v })
}
