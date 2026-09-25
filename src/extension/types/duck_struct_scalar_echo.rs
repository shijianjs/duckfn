use duckfn::duck_scalar_function;
use duckfn::{DuckArray, DuckBlob, DuckDate, DuckDecimal, DuckStruct, DuckTimestamp};
use indexmap::IndexMap;

use super::duck_enum_echo::Priority;

// ============================================================================
// STRUCT 类型 echo 函数
//
// 覆盖 duck_struct.rs 的 DuckStructTrait：
//   - #[derive(DuckStruct)] 为命名结构体生成字段 schema、子字段读取器 / 写入器，
//     再由 `impl<T: DuckStructTrait> DuckValueType for T` 得到 DuckValueType
//   - 字段不可为 NULL 时写 `T`，可为 NULL 时写 `Option<T>`；非 Option 字段读到
//     NULL 会让整个 struct 返回 NULL（宏生成代码用 `?` 提前返回）
//
// 每个函数同时验证：
//   - 入参：StructVector.get_child -> 子字段 DuckValueType::read
//   - 出参：StructVector.get_child -> 子字段 DuckValueType::write
//   - 类型：logical_type 是否映射为 STRUCT(...)
//
// SQL 侧 struct 字面量形如 {'id': 1, 'name': 'a'}，字段按名字匹配。
// ============================================================================

// ---------------------------------------------------------------------------
// 基础结构
// ---------------------------------------------------------------------------

/// STRUCT(id INTEGER, name VARCHAR)
/// ```sql
/// SELECT dfn_echo_struct_simple({'id': 1, 'name': 'a'});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructSimple {
    pub id: i32,
    pub name: String,
}

#[duck_scalar_function]
fn dfn_echo_struct_simple(i: DuckStructSimple) -> DuckStructSimple {
    i
}

/// 覆盖 bool / i8 / i16 / i32 / i64 / f32 / f64 / String 八种字段类型
/// STRUCT(b BOOLEAN, t TINYINT, s SMALLINT, i INTEGER, l BIGINT, f FLOAT, d DOUBLE, v VARCHAR)
/// ```sql
/// SELECT dfn_echo_struct_simple_types(
///     {'b': true, 't': 1::TINYINT, 's': 2::SMALLINT, 'i': 3::INTEGER,
///      'l': 4::BIGINT, 'f': 1.5::FLOAT, 'd': 2.5::DOUBLE, 'v': 'x'});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructSimpleTypes {
    pub b: bool,
    pub t: i8,
    pub s: i16,
    pub i: i32,
    pub l: i64,
    pub f: f32,
    pub d: f64,
    pub v: String,
}

#[duck_scalar_function]
fn dfn_echo_struct_simple_types(i: DuckStructSimpleTypes) -> DuckStructSimpleTypes {
    i
}

/// 包装类型字段：DATE / TIMESTAMP / DECIMAL(18,3) / BLOB
/// STRUCT(d DATE, ts TIMESTAMP, dec DECIMAL(18,3), bl BLOB)
/// ```sql
/// SELECT dfn_echo_struct_wrapper(
///     {'d': DATE '2024-01-02', 'ts': TIMESTAMP '2024-01-02 03:04:05',
///      'dec': 1.234::DECIMAL(18,3), 'bl': '\xAA\xBB'::BLOB});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructWrapper {
    pub d: DuckDate,
    pub ts: DuckTimestamp,
    pub dec: DuckDecimal<18, 3>,
    pub bl: DuckBlob,
}

#[duck_scalar_function]
fn dfn_echo_struct_wrapper(i: DuckStructWrapper) -> DuckStructWrapper {
    i
}

// ---------------------------------------------------------------------------
// 字段可空：Option<T>
// ---------------------------------------------------------------------------

/// STRUCT(id INTEGER, age INTEGER, name VARCHAR)
/// ```sql
/// SELECT dfn_echo_struct_nullable({'id': 1, 'age': NULL, 'name': NULL});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructNullable {
    pub id: i32,
    pub age: Option<i32>,
    pub name: Option<String>,
}

#[duck_scalar_function]
fn dfn_echo_struct_nullable(i: DuckStructNullable) -> DuckStructNullable {
    i
}

/// 所有字段都可空
/// STRUCT(id INTEGER, name VARCHAR, data INTEGER[])
/// ```sql
/// SELECT dfn_echo_struct_all_nullable({'id': NULL, 'name': NULL, 'data': NULL});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructAllNullable {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub data: Option<Vec<Option<i32>>>,
}

#[duck_scalar_function]
fn dfn_echo_struct_all_nullable(i: DuckStructAllNullable) -> DuckStructAllNullable {
    i
}

// ---------------------------------------------------------------------------
// 字段里嵌套变长结构：LIST / MAP / ARRAY
// ---------------------------------------------------------------------------

/// STRUCT(id INTEGER, data INTEGER[], tags VARCHAR[])
///
/// 写回时子字段的变长结构要等所有行写完才能确认长度，
/// 因此依赖 s_write_finish 递归调用子字段的 write_finish。
/// ```sql
/// SELECT dfn_echo_struct_with_list({'id': 1, 'data': [1, 2, 3], 'tags': ['a', 'b']});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructWithList {
    pub id: i32,
    pub data: Vec<i32>,
    pub tags: Vec<String>,
}

#[duck_scalar_function]
fn dfn_echo_struct_with_list(i: DuckStructWithList) -> DuckStructWithList {
    i
}

/// STRUCT(id INTEGER, data INTEGER[])
///
/// 列表整体可 NULL、列表元素也可 NULL。
/// ```sql
/// SELECT dfn_echo_struct_with_list_n({'id': 1, 'data': [1, NULL, 3]});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructWithListN {
    pub id: i32,
    pub data: Option<Vec<Option<i32>>>,
}

#[duck_scalar_function]
fn dfn_echo_struct_with_list_n(i: DuckStructWithListN) -> DuckStructWithListN {
    i
}

/// STRUCT(id INTEGER, m MAP(VARCHAR, INTEGER))
/// ```sql
/// SELECT dfn_echo_struct_with_map({'id': 1, 'm': map(['a', 'b'], [1, 2])});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructWithMap {
    pub id: i32,
    pub m: IndexMap<String, i32>,
}

#[duck_scalar_function]
fn dfn_echo_struct_with_map(i: DuckStructWithMap) -> DuckStructWithMap {
    i
}

/// STRUCT(id INTEGER, arr INTEGER[3])
/// ```sql
/// SELECT dfn_echo_struct_with_array({'id': 1, 'arr': [1, 2, 3]::INTEGER[3]});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructWithArray {
    pub id: i32,
    pub arr: DuckArray<i32, 3>,
}

#[duck_scalar_function]
fn dfn_echo_struct_with_array(i: DuckStructWithArray) -> DuckStructWithArray {
    i
}

// ---------------------------------------------------------------------------
// struct 嵌套 struct
// ---------------------------------------------------------------------------

/// STRUCT(key VARCHAR, value INTEGER)
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructInner {
    pub key: String,
    pub value: i32,
}

/// STRUCT(id INTEGER, inner STRUCT(key VARCHAR, value INTEGER), maybe STRUCT(key VARCHAR, value INTEGER))
///
/// inner 不可空，maybe 可空，覆盖嵌套 struct 的两种字段形态。
/// ```sql
/// SELECT dfn_echo_struct_nested(
///     {'id': 1, 'inner': {'key': 'a', 'value': 2}, 'maybe': NULL});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructNested {
    pub id: i32,
    pub inner: DuckStructInner,
    pub maybe: Option<DuckStructInner>,
}

#[duck_scalar_function]
fn dfn_echo_struct_nested(i: DuckStructNested) -> DuckStructNested {
    i
}

/// 顶层 STRUCT(key VARCHAR, value INTEGER)，用来单独观察内层 struct 的读写路径
/// ```sql
/// SELECT dfn_echo_struct_inner({'key': 'a', 'value': 2});
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_inner(i: DuckStructInner) -> DuckStructInner {
    i
}

/// STRUCT(id INTEGER, inner STRUCT(key VARCHAR, value INTEGER))
/// 与 DuckStructNested 只差在没有可空的嵌套字段
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructNestedOnly {
    pub id: i32,
    pub inner: DuckStructInner,
}

#[duck_scalar_function]
fn dfn_echo_struct_nested_only(i: DuckStructNestedOnly) -> DuckStructNestedOnly {
    i
}

// ---------------------------------------------------------------------------
// struct 作为其它容器的元素
//
// 每个容器给两版对照：元素类型不带 Option（NULL 元素无法表示，会让整个容器
// 变成 NULL）与带 Option（NULL 元素被保留）。带 Option 的版本以 _nullable
// 结尾（duck_map_scalar_echo.rs / duck_array_scalar_echo.rs 中同类对照用 _n 后缀，
// 那是给简单类型 / 包装类型元素用的）。
// ---------------------------------------------------------------------------

/// LIST(STRUCT(id INTEGER, name VARCHAR))
/// ```sql
/// SELECT dfn_echo_struct_list([{'id': 1, 'name': 'a'}, {'id': 2, 'name': 'b'}]);
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_list(i: Vec<DuckStructSimple>) -> Vec<DuckStructSimple> {
    i
}

/// LIST(STRUCT(id INTEGER, name VARCHAR))，元素可空
/// - 元素类型为 Option<T> 时列表可以承载 NULL 元素
/// ```sql
/// SELECT dfn_echo_struct_list_nullable(
///     [{'id': 1, 'name': 'a'}, NULL::STRUCT(id INTEGER, name VARCHAR)]);
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_list_nullable(
    i: Vec<Option<DuckStructSimple>>,
) -> Vec<Option<DuckStructSimple>> {
    i
}

/// MAP(VARCHAR, STRUCT(id INTEGER, name VARCHAR))
/// ```sql
/// SELECT dfn_echo_struct_map_value(map(['k'], [{'id': 1, 'name': 'a'}]));
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_map_value(
    i: IndexMap<String, DuckStructSimple>,
) -> IndexMap<String, DuckStructSimple> {
    i
}

/// MAP(VARCHAR, STRUCT(id INTEGER, name VARCHAR))，value 可空
/// - MAP 的 key 不允许为 NULL，所以只有 value 有 Option 变体
/// ```sql
/// SELECT dfn_echo_struct_map_value_nullable(
///     map(['k'], [NULL::STRUCT(id INTEGER, name VARCHAR)]));
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_map_value_nullable(
    i: IndexMap<String, Option<DuckStructSimple>>,
) -> IndexMap<String, Option<DuckStructSimple>> {
    i
}

/// ARRAY(STRUCT(id INTEGER, name VARCHAR), 2)，元素不可空
/// ```sql
/// SELECT dfn_echo_struct_array(
///     [{'id': 1, 'name': 'a'}, {'id': 2, 'name': 'b'}]::STRUCT(id INTEGER, name VARCHAR)[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_array(i: DuckArray<DuckStructSimple, 2>) -> DuckArray<DuckStructSimple, 2> {
    i
}

/// ARRAY(STRUCT(id INTEGER, name VARCHAR), 2)，元素可空
/// ```sql
/// SELECT dfn_echo_struct_array_nullable(
///     [{'id': 1, 'name': 'a'}, NULL]::STRUCT(id INTEGER, name VARCHAR)[2]);
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_array_nullable(
    i: [Option<DuckStructSimple>; 2],
) -> [Option<DuckStructSimple>; 2] {
    i
}

// ---------------------------------------------------------------------------
// create_type：加载期建一个命名 STRUCT 类型
// ---------------------------------------------------------------------------

/// 作为 `DuckStructTicket` 一个字段的嵌套结构体：STRUCT(flag BOOLEAN)。
///
/// A nested struct used as one field of `DuckStructTicket`: `STRUCT(flag BOOLEAN)`.
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct DuckStructTicketInner {
    pub flag: bool,
}

/// `#[duck(create_type = true)]`：加载期执行
/// `CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'), "labels" VARCHAR[], "slot" INTEGER[3], "scores" MAP(VARCHAR, DOUBLE), "amount" DECIMAL(18, 3), "inner" STRUCT("flag" BOOLEAN));`
///
/// 字段类型不是在这里写死的 SQL —— 注册函数把整个结构体的**逻辑类型**交给
/// `duckfn::register_named_type`，由它递归渲染（`duckdb_get_type_id` + 子类型 / 字典 / 精度查询）。
/// 所以枚举（含 `#[derive(DuckEnum)]` 生成的）、自定义类型、嵌套结构体、LIST / ARRAY / MAP
/// 都能自动带上，`Option<T>` 的可空性也不影响类型文本。
///
/// `#[duck(create_type = true)]` runs `CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT(...)` at load
/// time. The field types are not hard-coded SQL: the registrar hands the struct's *logical type* to
/// `duckfn::register_named_type`, which renders it recursively, so enums, custom types, nested
/// structs and LIST / ARRAY / MAP come along automatically.
///
/// ```sql
/// SELECT typeof(NULL::ticket);       -- STRUCT(id BIGINT, ...)：类型可以在 SQL 里直接用
/// CREATE TABLE tickets (v ticket);   -- 也可以作为列类型
/// SELECT dfn_echo_struct_ticket(v) FROM tickets;
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = true)]
pub struct DuckStructTicket {
    pub id: i64,
    pub priority: Priority,
    pub labels: Vec<String>,
    pub slot: [i32; 3],
    pub scores: IndexMap<String, f64>,
    pub amount: DuckDecimal<18, 3>,
    pub inner: DuckStructTicketInner,
}

#[duck_scalar_function]
fn dfn_echo_struct_ticket(i: DuckStructTicket) -> DuckStructTicket {
    i
}

// ---------------------------------------------------------------------------
// create_type = "print"：加载期只收集建类型的 DDL，不建类型
// ---------------------------------------------------------------------------

/// 打印模式：`#[duck(create_type = "print")]` 把
/// `CREATE TYPE IF NOT EXISTS "preview_ticket" AS STRUCT("id" BIGINT, "name" VARCHAR);`
/// 收进队列、不执行，`LOAD` 结束后和其它 `"print"` 类型一起打印到 stderr；所以
/// `duckdb_types()` 里查不到 `preview_ticket`。
///
/// Print mode: the `CREATE TYPE IF NOT EXISTS "preview_ticket" AS STRUCT(...)` statement is queued
/// instead of executed and shows up in the batch printed to stderr after the load, so
/// `preview_ticket` never appears in `duckdb_types()`.
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "preview_ticket", create_type = "print")]
pub struct DuckStructPreviewTicket {
    pub id: i64,
    pub name: String,
}

/// 打印模式只影响「建不建类型」：结构体作为 STRUCT 值类型照常可用。
///
/// ```sql
/// SELECT dfn_echo_struct_preview_ticket({'id': 1, 'name': 'a'});
/// ```
#[duck_scalar_function]
fn dfn_echo_struct_preview_ticket(t: DuckStructPreviewTicket) -> DuckStructPreviewTicket {
    t
}
