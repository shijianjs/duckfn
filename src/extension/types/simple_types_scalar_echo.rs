use duckfn::duck_scalar_function;

// ============================================================================
// 简单类型 echo 函数
//
// 覆盖 simple_types.rs 中已实现的全部 DuckValueType：
// bool / i8 / i16 / i32 / i64 / i128 / u8 / u16 / u32 / u64 / u128 / f32 / f64 / String
//
// 每个函数同时验证：
//   - 入参：VectorReader.read_* -> DuckValueType::read
//   - 出参：DuckValueType::write -> VectorWriter.write_*
//   - 类型：type_id / logical_type 映射是否正确
// ============================================================================

/// TypeId::Boolean
/// ```sql
/// SELECT dfn_echo_bool(i) FROM (VALUES (true), (false)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_bool(i: bool) -> bool {
    i
}

/// TypeId::TinyInt // i8
/// ```sql
/// SELECT dfn_echo_tinyint(i) FROM (VALUES (1::TINYINT), (-1::TINYINT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_tinyint(i: i8) -> i8 {
    i
}

/// TypeId::SmallInt // i16
/// ```sql
/// SELECT dfn_echo_smallint(i) FROM (VALUES (1::SMALLINT), (-1::SMALLINT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_smallint(i: i16) -> i16 {
    i
}

/// TypeId::Integer // i32
/// ```sql
/// SELECT dfn_echo_integer(i) FROM (VALUES (1::INTEGER), (-1::INTEGER)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_integer(i: i32) -> i32 {
    i
}

/// TypeId::BigInt // i64
/// ```sql
/// SELECT dfn_echo_bigint(i) FROM (VALUES (1::BIGINT), (-1::BIGINT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_bigint(i: i64) -> i64 {
    i
}

/// TypeId::HugeInt // i128
/// ```sql
/// SELECT dfn_echo_hugeint(1::HUGEINT);
/// ```
#[duck_scalar_function]
fn dfn_echo_hugeint(i: i128) -> i128 {
    i
}

/// TypeId::UTinyInt // u8
/// ```sql
/// SELECT dfn_echo_utinyint(i) FROM (VALUES (0::UTINYINT), (255::UTINYINT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_utinyint(i: u8) -> u8 {
    i
}

/// TypeId::USmallInt // u16
/// ```sql
/// SELECT dfn_echo_usmallint(i) FROM (VALUES (0::USMALLINT), (65535::USMALLINT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_usmallint(i: u16) -> u16 {
    i
}

/// TypeId::UInteger // u32
/// ```sql
/// SELECT dfn_echo_uinteger(i) FROM (VALUES (0::UINTEGER), (4294967295::UINTEGER)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_uinteger(i: u32) -> u32 {
    i
}

/// TypeId::UBigInt // u64
/// ```sql
/// SELECT dfn_echo_ubigint(i) FROM (VALUES (0::UBIGINT), (18446744073709551615::UBIGINT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_ubigint(i: u64) -> u64 {
    i
}

/// TypeId::UHugeInt // u128
/// ```sql
/// SELECT dfn_echo_uhugeint(1::UHUGEINT);
/// ```
#[duck_scalar_function]
fn dfn_echo_uhugeint(i: u128) -> u128 {
    i
}

/// TypeId::Float // f32
/// ```sql
/// SELECT dfn_echo_float(i) FROM (VALUES (1.5::FLOAT), (-2.25::FLOAT)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_float(i: f32) -> f32 {
    i
}

/// TypeId::Double // f64
/// ```sql
/// SELECT dfn_echo_double(i) FROM (VALUES (1.5::DOUBLE), (-2.25::DOUBLE)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_double(i: f64) -> f64 {
    i
}

/// TypeId::Varchar // String
/// ```sql
/// SELECT dfn_echo_varchar(i) FROM (VALUES ('abc'), ('') ) t(i);
/// ```
#[duck_scalar_function]
fn dfn_echo_varchar(i: String) -> String {
    i
}
