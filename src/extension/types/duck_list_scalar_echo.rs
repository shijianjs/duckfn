use duckfn::duck_scalar_function;
use duckfn::{DuckBlob, DuckDate, DuckDecimal};

// ============================================================================
// LIST 类型 echo 函数
//
// 覆盖 duck_list.rs 中的两个实现：
//   - Vec<T>           -> LIST(T)，元素不可为 NULL（遇到 NULL 元素整体返回 NULL）
//   - Vec<Option<T>>   -> LIST(T)，元素可为 NULL
//
// 元素类型分别覆盖简单类型（i32/i64/f64/bool/String）、包装类型
// （DuckDate/DuckDecimal/DuckBlob）以及嵌套 LIST（Vec<Vec<T>>）。
//
// 每个函数同时验证：
//   - 入参：ListVector.get_entry + child_reader -> DuckValueType::read
//   - 出参：ListVector.set_entry/reserve -> DuckValueType::write
//   - 类型：logical_type 是否正确映射为 T[]
// ============================================================================

// ---------------------------------------------------------------------------
// Vec<T>：元素不可为 NULL
// ---------------------------------------------------------------------------

/// LIST(INTEGER) // Vec<i32>
/// ```sql
/// SELECT dfn_echo_list_integer([1, 2, 3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_integer(i: Vec<i32>) -> Vec<i32> {
    i
}

/// LIST(BIGINT) // Vec<i64>
/// ```sql
/// SELECT dfn_echo_list_bigint([1, 2, 3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_bigint(i: Vec<i64>) -> Vec<i64> {
    i
}

/// LIST(DOUBLE) // Vec<f64>
/// ```sql
/// SELECT dfn_echo_list_double([1.5, -2.25]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_double(i: Vec<f64>) -> Vec<f64> {
    i
}

/// LIST(BOOLEAN) // Vec<bool>
/// ```sql
/// SELECT dfn_echo_list_bool([true, false]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_bool(i: Vec<bool>) -> Vec<bool> {
    i
}

/// LIST(VARCHAR) // Vec<String>
/// ```sql
/// SELECT dfn_echo_list_varchar(['a', 'bb']);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_varchar(i: Vec<String>) -> Vec<String> {
    i
}

/// LIST(DATE) // Vec<DuckDate>
/// ```sql
/// SELECT dfn_echo_list_date([DATE '2024-01-02', DATE '1969-12-31']);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_date(i: Vec<DuckDate>) -> Vec<DuckDate> {
    i
}

/// LIST(DECIMAL(18,3)) // Vec<DuckDecimal<18, 3>>
/// ```sql
/// SELECT dfn_echo_list_decimal([1.234::DECIMAL(18,3), (-1.234)::DECIMAL(18,3)]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_decimal(i: Vec<DuckDecimal<18, 3>>) -> Vec<DuckDecimal<18, 3>> {
    i
}

/// LIST(BLOB) // Vec<DuckBlob>
/// ```sql
/// SELECT dfn_echo_list_blob(['\xAA\xBB'::BLOB, ''::BLOB]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_blob(i: Vec<DuckBlob>) -> Vec<DuckBlob> {
    i
}

/// LIST(LIST(INTEGER)) // Vec<Vec<i32>>，嵌套 LIST
/// ```sql
/// SELECT dfn_echo_list_nested([[1, 2], [3]]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_nested(i: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    i
}

// ---------------------------------------------------------------------------
// Vec<Option<T>>：元素可为 NULL
// ---------------------------------------------------------------------------

/// LIST(INTEGER) // Vec<Option<i32>>
/// ```sql
/// SELECT dfn_echo_list_integer_n([1, NULL, 3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_integer_n(i: Vec<Option<i32>>) -> Vec<Option<i32>> {
    i
}

/// LIST(VARCHAR) // Vec<Option<String>>
/// ```sql
/// SELECT dfn_echo_list_varchar_n(['a', NULL]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_varchar_n(i: Vec<Option<String>>) -> Vec<Option<String>> {
    i
}

/// LIST(DATE) // Vec<Option<DuckDate>>
/// ```sql
/// SELECT dfn_echo_list_date_n([DATE '2024-01-02', NULL]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_date_n(i: Vec<Option<DuckDate>>) -> Vec<Option<DuckDate>> {
    i
}

/// LIST(LIST(INTEGER)) // Vec<Option<Vec<Option<i32>>>>，嵌套 LIST 且内外层均可为 NULL
/// ```sql
/// SELECT dfn_echo_list_nested_n([[1, NULL], NULL, []]);
/// ```
#[duck_scalar_function]
fn dfn_echo_list_nested_n(i: Vec<Option<Vec<Option<i32>>>>) -> Vec<Option<Vec<Option<i32>>>> {
    i
}
