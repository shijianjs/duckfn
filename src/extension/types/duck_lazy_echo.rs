use duckfn::{duck_scalar_function, duck_table_function, DuckLazy, DuckStruct};

// ============================================================================
// DuckLazy<T> 的全场景覆盖
//
// DuckLazy<T> 是「本行内的延迟读取」：读取只记录位置（O(1)），真正解析推迟到 get()。
// 逻辑类型与 T 完全相同，所以下面的函数在 SQL 侧看起来和直接写 T 一模一样。
//
// 覆盖：
//   - 标量入参（标量函数体在同一个回调里执行，get() 安全）
//   - Option<DuckLazy<T>>：NULL 走既有的 Option 机制，NULL 时根本不构造凭证
//   - struct 字段（#[derive(DuckStruct)] 路径）与容器元素（Vec<DuckLazy<T>>）
//   - 只读：把凭证当返回值时，写路径 panic → 查询报错
//   - bind 参数：表函数 bind 走 Value 路径，明确不支持 → bind 报错
//
// 聚合场景（复杂配置项只解析一次）见 src/extension/demo/lazy_config_demo.rs。
// ============================================================================

/// INTEGER // DuckLazy<i32>
/// ```sql
/// SELECT dfn_echo_lazy_integer(42);
/// ```
#[duck_scalar_function]
fn dfn_echo_lazy_integer(i: DuckLazy<i32>) -> i32 {
    i.get()
}

/// INTEGER，可空 // Option<DuckLazy<i32>>
/// ```sql
/// SELECT dfn_echo_lazy_integer_n(NULL);
/// ```
///
/// NULL 由 `Option` 承载：单元格为 NULL 时读到 `None`，不会构造凭证，也就不会解析。
///
/// NULL is carried by `Option`: a NULL cell reads as `None`, so no token is built and nothing is
/// parsed.
#[duck_scalar_function]
fn dfn_echo_lazy_integer_n(i: Option<DuckLazy<i32>>) -> Option<i32> {
    i.map(|v| v.get())
}

/// VARCHAR // DuckLazy<String>
/// ```sql
/// SELECT dfn_echo_lazy_varchar('abc');
/// ```
#[duck_scalar_function]
fn dfn_echo_lazy_varchar(i: DuckLazy<String>) -> String {
    i.get()
}

/// LIST(INTEGER) // Vec<DuckLazy<i32>>：元素也是延迟读取
/// ```sql
/// SELECT dfn_echo_lazy_list_sum([1, 2, 3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_lazy_list_sum(i: Vec<DuckLazy<i32>>) -> i64 {
    i.iter().map(|v| i64::from(v.get())).sum()
}

/// STRUCT(v INTEGER, s VARCHAR) // 字段是 DuckLazy / Option<DuckLazy<_>>
/// ```sql
/// SELECT dfn_echo_lazy_struct({'v': 1, 's': 'ab'});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct LazyRow {
    /// 非可空字段：`NULL` 会让整行变成 NULL（走既有的字段读取语义）。
    pub v: DuckLazy<i32>,
    /// 可空字段。
    pub s: Option<DuckLazy<String>>,
}

#[duck_scalar_function]
fn dfn_echo_lazy_struct(r: LazyRow) -> i64 {
    r.v.get() as i64 + r.s.map_or(0, |s| s.get().len() as i64)
}

/// 只读：把凭证当返回值 —— 写路径会 panic，适配层把它变成查询报错。
///
/// ```sql
/// SELECT dfn_echo_lazy_passthrough(1);   -- ERROR: DuckLazy<T> is read-only: ...
/// ```
#[duck_scalar_function]
fn dfn_echo_lazy_passthrough(i: DuckLazy<i32>) -> DuckLazy<i32> {
    i
}

#[derive(Clone, Debug, Default, DuckStruct)]
pub struct LazyTableRow {
    pub v: i32,
}

/// bind 参数不支持：表函数的参数走 `Value` 路径，而 `Value` 只在 bind 回调里有效。
///
/// ```sql
/// SELECT * FROM dfn_table_echo_lazy(1);   -- ERROR: DuckLazy<T> is not supported on the bind/Value path: ...
/// ```
#[duck_table_function]
fn dfn_table_echo_lazy(i: DuckLazy<i32>) -> impl Iterator<Item = LazyTableRow> {
    // 正常情况下永远到不了这里：bind 阶段就已经报错。
    let v = i.get();
    std::iter::once(LazyTableRow { v })
}
