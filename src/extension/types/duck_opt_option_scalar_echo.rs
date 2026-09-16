use duckfn::duck_scalar_function;
use duckfn::{DuckDate, DuckStruct};

use super::duck_struct_scalar_echo::DuckStructSimple;

// ============================================================================
// Option<Option<T>> 的 echo 往返
//
// 「双层 Option」与单层 Option 行为等价，这是有意提供的能力：
// 封装层有时拿不到、也不方便把中间类型剥出来（泛型参数本身可能已经是 Option），
// 直接再套一层 Option 就能表达「可空」，不必为「本来就是 Option」单独加分支。
//
// 等价性来自 duck_option.rs 的实现：
//   - logical_type 逐层转发给最内层类型：Option<Option<i32>> 与 i32 都是 INTEGER；
//   - 读：NULL 槽位由 read() 在外层判掉，所以读不出 Some(None)，只会得到最外层 None；
//   - 写：None 与 Some(None) 都落到 T::write_null，都是写 NULL（见本目录的 .test）。
//
// 覆盖的 T：简单类型（i32 / String）、包装类型（DuckDate）、容器（LIST）、
// 结构体（DuckStructSimple），以及「结构体字段本身是双层 Option」的写法。
// ============================================================================

/// INTEGER // Option<Option<i32>>
/// ```sql
/// SELECT dfn_echo_opt_option_integer(42);
/// SELECT dfn_echo_opt_option_integer(NULL);
/// ```
#[duck_scalar_function]
fn dfn_echo_opt_option_integer(i: Option<Option<i32>>) -> Option<Option<i32>> {
    i
}

/// VARCHAR // Option<Option<String>>
/// ```sql
/// SELECT dfn_echo_opt_option_varchar('abc');
/// ```
#[duck_scalar_function]
fn dfn_echo_opt_option_varchar(i: Option<Option<String>>) -> Option<Option<String>> {
    i
}

/// DATE // Option<Option<DuckDate>>
/// ```sql
/// SELECT dfn_echo_opt_option_date(DATE '2024-01-02');
/// ```
#[duck_scalar_function]
fn dfn_echo_opt_option_date(i: Option<Option<DuckDate>>) -> Option<Option<DuckDate>> {
    i
}

/// LIST(INTEGER)，列表可空、元素也可空 // Option<Option<Vec<Option<i32>>>>
/// ```sql
/// SELECT dfn_echo_opt_option_list([1, NULL, 3]);
/// ```
#[duck_scalar_function]
fn dfn_echo_opt_option_list(
    i: Option<Option<Vec<Option<i32>>>>,
) -> Option<Option<Vec<Option<i32>>>> {
    i
}

/// STRUCT(v INTEGER, l INTEGER[])，字段本身是双层 Option
/// ```sql
/// SELECT dfn_echo_opt_option_struct({'v': 5, 'l': [1, NULL, 3]});
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct OptOptionRow {
    /// 双层 Option 的标量字段：`NULL` 与「有值」都能表达。
    ///
    /// A double-`Option` scalar field: it expresses both `NULL` and a value.
    pub v: Option<Option<i32>>,
    /// 列表元素也是双层 Option：元素 `NULL` 原样保留，不会像 `Vec<i32>` 那样整体变 NULL。
    ///
    /// The list elements are double-`Option` as well, so a `NULL` element stays `NULL` instead of
    /// turning the whole list `NULL` like `Vec<i32>` does.
    pub l: Option<Vec<Option<Option<i32>>>>,
}

#[duck_scalar_function]
fn dfn_echo_opt_option_struct(r: OptOptionRow) -> OptOptionRow {
    r
}

/// STRUCT(id INTEGER, name VARCHAR) // Option<Option<DuckStructSimple>>
/// ```sql
/// SELECT dfn_echo_opt_option_struct_simple({'id': 1, 'name': 'a'});
/// ```
#[duck_scalar_function]
fn dfn_echo_opt_option_struct_simple(
    i: Option<Option<DuckStructSimple>>,
) -> Option<Option<DuckStructSimple>> {
    i
}
