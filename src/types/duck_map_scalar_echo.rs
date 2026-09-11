use duckfn::duck_scalar_function;
use duckfn::{DuckBlob, DuckDate, DuckDecimal};
use indexmap::IndexMap;

// ============================================================================
// MAP 类型 echo 函数
//
// 覆盖 duck_map.rs 中的两个实现：
//   - IndexMap<K, V>         -> MAP(K, V)，value 不可为 NULL（遇到 NULL value 整体返回 NULL）
//   - IndexMap<K, Option<V>> -> MAP(K, V)，value 可为 NULL
//
// 关于 key：DuckDB 的 MAP key 不允许为 NULL，所以两个实现都直接是 IndexMap<K, _>
// （K 本身不是 Option<K>），不存在 key 可空的情况。
//
// 与 LIST 的关系：MAP 在物理布局上就是 LIST<STRUCT<key, value>>，
// 所以实现里同样用 ListVector::reserve / set_entry / set_size 管理 entry，
// 只是子向量被拆成 keys / values 两条；读取时用 MapVector::get_entry 拿到
// {offset, length}，再分别到两个子向量里按同样下标取值。
//
// 每个函数同时验证：
//   - 入参：MapVector.get_entry + keys/values -> DuckValueType::read
//   - 出参：ListVector.set_entry/reserve + keys/values -> DuckValueType::write
//   - 类型：logical_type 是否正确映射为 MAP(K, V)
// ============================================================================

// ---------------------------------------------------------------------------
// IndexMap<K, V>：value 不可为 NULL
// ---------------------------------------------------------------------------

/// MAP(VARCHAR, INTEGER) // IndexMap<String, i32>
/// ```sql
/// SELECT dfn_echo_map_varchar_integer(map(['a', 'b'], [1, 2]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_integer(i: IndexMap<String, i32>) -> IndexMap<String, i32> {
    i
}

/// MAP(INTEGER, VARCHAR) // IndexMap<i32, String>
/// ```sql
/// SELECT dfn_echo_map_integer_varchar(map([1, 2], ['a', 'b']));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_integer_varchar(i: IndexMap<i32, String>) -> IndexMap<i32, String> {
    i
}

/// MAP(BIGINT, BIGINT) // IndexMap<i64, i64>
/// ```sql
/// SELECT dfn_echo_map_bigint_bigint(map([1::BIGINT], [9223372036854775807::BIGINT]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_bigint_bigint(i: IndexMap<i64, i64>) -> IndexMap<i64, i64> {
    i
}

/// MAP(INTEGER, DOUBLE) // IndexMap<i32, f64>
///
/// 注意：float 不能做 key —— duck_map.rs 的 impl 要求 `K: Hash + Eq`，
/// 而 f32/f64 都没有实现 Eq，所以只能放在 value 位置。
/// ```sql
/// SELECT dfn_echo_map_integer_double(map([1, 2], [1.5, -2.25]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_integer_double(i: IndexMap<i32, f64>) -> IndexMap<i32, f64> {
    i
}

/// MAP(VARCHAR, DATE) // IndexMap<String, DuckDate>
/// ```sql
/// SELECT dfn_echo_map_varchar_date(map(['d'], [DATE '2024-01-02']));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_date(i: IndexMap<String, DuckDate>) -> IndexMap<String, DuckDate> {
    i
}

/// MAP(VARCHAR, DECIMAL(18,3)) // IndexMap<String, DuckDecimal<18, 3>>
/// ```sql
/// SELECT dfn_echo_map_varchar_decimal(map(['d'], [1.234::DECIMAL(18,3)]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_decimal(
    i: IndexMap<String, DuckDecimal<18, 3>>,
) -> IndexMap<String, DuckDecimal<18, 3>> {
    i
}

/// MAP(VARCHAR, BLOB) // IndexMap<String, DuckBlob>
/// ```sql
/// SELECT dfn_echo_map_varchar_blob(map(['b'], ['\xAA\xBB'::BLOB]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_blob(i: IndexMap<String, DuckBlob>) -> IndexMap<String, DuckBlob> {
    i
}

/// MAP(VARCHAR, MAP(VARCHAR, INTEGER)) // IndexMap<String, IndexMap<String, i32>>，嵌套 MAP
/// ```sql
/// SELECT dfn_echo_map_nested(map(['o'], [map(['i'], [1])]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_nested(
    i: IndexMap<String, IndexMap<String, i32>>,
) -> IndexMap<String, IndexMap<String, i32>> {
    i
}

// ---------------------------------------------------------------------------
// IndexMap<K, Option<V>>：value 可为 NULL
// ---------------------------------------------------------------------------

/// MAP(VARCHAR, INTEGER) // IndexMap<String, Option<i32>>
/// ```sql
/// SELECT dfn_echo_map_varchar_integer_n(map(['a', 'b'], [1, NULL]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_integer_n(
    i: IndexMap<String, Option<i32>>,
) -> IndexMap<String, Option<i32>> {
    i
}

/// MAP(INTEGER, VARCHAR) // IndexMap<i32, Option<String>>
/// ```sql
/// SELECT dfn_echo_map_integer_varchar_n(map([1, 2], ['a', NULL]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_integer_varchar_n(
    i: IndexMap<i32, Option<String>>,
) -> IndexMap<i32, Option<String>> {
    i
}

/// MAP(VARCHAR, DATE) // IndexMap<String, Option<DuckDate>>
/// ```sql
/// SELECT dfn_echo_map_varchar_date_n(map(['d'], [NULL::DATE]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_date_n(
    i: IndexMap<String, Option<DuckDate>>,
) -> IndexMap<String, Option<DuckDate>> {
    i
}

/// MAP(VARCHAR, DECIMAL(18,3)) // IndexMap<String, Option<DuckDecimal<18, 3>>>
/// ```sql
/// SELECT dfn_echo_map_varchar_decimal_n(map(['d'], [NULL::DECIMAL(18,3)]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_varchar_decimal_n(
    i: IndexMap<String, Option<DuckDecimal<18, 3>>>,
) -> IndexMap<String, Option<DuckDecimal<18, 3>>> {
    i
}

/// MAP(VARCHAR, MAP(VARCHAR, INTEGER))
/// // IndexMap<String, Option<IndexMap<String, Option<i32>>>>，嵌套 MAP 且各层 value 可为 NULL
/// ```sql
/// SELECT dfn_echo_map_nested_n(map(['o'], [map(['i'], [NULL])]));
/// ```
#[duck_scalar_function]
fn dfn_echo_map_nested_n(
    i: IndexMap<String, Option<IndexMap<String, Option<i32>>>>,
) -> IndexMap<String, Option<IndexMap<String, Option<i32>>>> {
    i
}
