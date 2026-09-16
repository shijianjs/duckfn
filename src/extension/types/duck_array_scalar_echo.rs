use duckfn::duck_scalar_function;
use duckfn::{DuckArray, DuckBlob, DuckDate, DuckDecimal};

// ============================================================================
// ARRAY 类型 echo 函数
//
// 覆盖 duck_array.rs 的实现（注意它是固定长度的定长数组，长度 N 是类型的一部分）：
//   - T[N]（别名 DuckArray<T, N>）：元素类型写 T 时元素不可为 NULL（遇到 NULL 元素整体返回 NULL），
//     元素类型写 Option<T> 时元素可为 NULL —— 可空性由元素类型表达，没有单独的别名
//
// 元素类型分别覆盖简单类型（i32/i64/f64/bool/String）、包装类型
// （DuckDate/DuckDecimal/DuckBlob）以及嵌套 ARRAY（[T[N]; M] -> T[N][M]）。
//
// 与 LIST 的关键差异：
//   - LIST 的行是变长的，写回时需要 set_entry/reserve/set_size；
//   - ARRAY 的行是定长的，子向量里第 row 行的元素固定落在 row*N .. row*N+N，
//     因此读写都用 `row * N + i` 直接定位，没有 offset 累加。
//
// 每个函数同时验证：
//   - 入参：ArrayVector.get_child -> DuckValueType::read
//   - 出参：ArrayVector.get_child -> DuckValueType::write，类型映射为 T[N]
// ============================================================================

// ---------------------------------------------------------------------------
// DuckArray<T, N>：T[N]，元素不可为 NULL
// ---------------------------------------------------------------------------

/// ARRAY(INTEGER, 3) // [i32; 3]
/// ```sql
/// SELECT dfn_echo_array_integer([1, 2, 3]::INTEGER[3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_integer(i: DuckArray<i32, 3>) -> DuckArray<i32, 3> {
    i
}

/// ARRAY(BIGINT, 2) // [i64; 2]
/// ```sql
/// SELECT dfn_echo_array_bigint([1, 2]::BIGINT[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_bigint(i: DuckArray<i64, 2>) -> DuckArray<i64, 2> {
    i
}

/// ARRAY(DOUBLE, 3) // [f64; 3]
/// ```sql
/// SELECT dfn_echo_array_double([1.5, -2.25, 0.0]::DOUBLE[3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_double(i: DuckArray<f64, 3>) -> DuckArray<f64, 3> {
    i
}

/// ARRAY(BOOLEAN, 2) // [bool; 2]
/// ```sql
/// SELECT dfn_echo_array_bool([true, false]::BOOLEAN[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_bool(i: DuckArray<bool, 2>) -> DuckArray<bool, 2> {
    i
}

/// ARRAY(VARCHAR, 2) // [String; 2]
/// ```sql
/// SELECT dfn_echo_array_varchar(['a', 'bb']::VARCHAR[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_varchar(i: DuckArray<String, 2>) -> DuckArray<String, 2> {
    i
}

/// ARRAY(DATE, 2) // [DuckDate; 2]
/// ```sql
/// SELECT dfn_echo_array_date([DATE '2024-01-02', DATE '1969-12-31']::DATE[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_date(i: DuckArray<DuckDate, 2>) -> DuckArray<DuckDate, 2> {
    i
}

/// ARRAY(DECIMAL(18,3), 2) // [DuckDecimal<18, 3>; 2]
/// ```sql
/// SELECT dfn_echo_array_decimal([1.234, -1.234]::DECIMAL(18,3)[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_decimal(
    i: DuckArray<DuckDecimal<18, 3>, 2>,
) -> DuckArray<DuckDecimal<18, 3>, 2> {
    i
}

/// ARRAY(BLOB, 2) // [DuckBlob; 2]
/// ```sql
/// SELECT dfn_echo_array_blob(['\xAA\xBB'::BLOB, ''::BLOB]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_blob(i: DuckArray<DuckBlob, 2>) -> DuckArray<DuckBlob, 2> {
    i
}

/// ARRAY(ARRAY(INTEGER, 2), 2) // [[i32; 2]; 2]，嵌套定长数组
/// ```sql
/// SELECT dfn_echo_array_nested([[1, 2], [3, 4]]::INTEGER[2][2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_nested(i: DuckArray<DuckArray<i32, 2>, 2>) -> DuckArray<DuckArray<i32, 2>, 2> {
    i
}

// ---------------------------------------------------------------------------
// [Option<T>; N]：T[N]，元素可为 NULL
// ---------------------------------------------------------------------------

/// ARRAY(INTEGER, 3) // [Option<i32>; 3]
/// ```sql
/// SELECT dfn_echo_array_integer_n([1, NULL, 3]::INTEGER[3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_integer_n(i: [Option<i32>; 3]) -> [Option<i32>; 3] {
    i
}

/// ARRAY(VARCHAR, 2) // [Option<String>; 2]
/// ```sql
/// SELECT dfn_echo_array_varchar_n(['a', NULL]::VARCHAR[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_varchar_n(i: [Option<String>; 2]) -> [Option<String>; 2] {
    i
}

/// ARRAY(DATE, 2) // [Option<DuckDate>; 2]
/// ```sql
/// SELECT dfn_echo_array_date_n([DATE '2024-01-02', NULL]::DATE[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_date_n(i: [Option<DuckDate>; 2]) -> [Option<DuckDate>; 2] {
    i
}

/// ARRAY(ARRAY(INTEGER, 2), 2) // [Option<[Option<i32>; 2]>; 2]，外层/内层元素都可为 NULL
/// ```sql
/// SELECT dfn_echo_array_nested_n([[1, NULL], NULL]::INTEGER[2][2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_array_nested_n(
    i: [Option<[Option<i32>; 2]>; 2],
) -> [Option<[Option<i32>; 2]>; 2] {
    i
}
