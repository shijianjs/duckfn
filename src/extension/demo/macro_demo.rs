// 示例代码：部分函数/结构体仅用于演示，并未全部注册或调用。
#![allow(dead_code)]

use quack_rs::prelude::SqlMacro;
use duckfn::{duck_aggregate_function, duck_scalar_function, duck_sql_macro, duck_table_function, DuckResult, DuckStruct};

#[derive(Clone, Default,  Debug, DuckStruct)]
#[duck(named_param_from = "data")]
pub struct DuckStructDemo1 {
    pub count: i64,
    pub data: Vec<i64>,
    pub age: Option<i32>,
    pub nest_data: Option<Vec<Vec<i64>>>,
}

#[duck_scalar_function]
fn error_scalar_demo(input: i64,_input2: i64) -> duckfn::DuckOptionResult<i64> {
    Ok(Some(input * 2))
}

#[duck_aggregate_function]
fn word_count_w_demo(_input: Option<String>, _arg2: i64,  _state: &mut WcAggState)-> duckfn::DuckResult<()>  {
    todo!()
}

#[derive(Default, Debug, Clone)]
struct WcAggState {
    count: i64,
}
impl duckfn::DuckAggregateState for WcAggState {
    type Output = i64;

    fn combine(&mut self, _other: &Self) -> duckfn::DuckResult<()> {
        todo!()
    }

    fn result(&self) -> duckfn::DuckOptionResult<i64> {
        todo!()
    }
}

#[duck_table_function]
fn table_fun_demo(start: i64) -> impl Iterator<Item =CountDownOutput> {
    (0..start).rev()
        .map(|x| CountDownOutput { n: x })
}
#[derive(Default, Debug, Clone, duckfn::DuckStruct)]
pub struct CountDownOutput {
    n: i64,
}

/// ```sql
/// SELECT clamp(range,4, 7) from range(9);
/// ```
#[duck_sql_macro]
pub fn sql_macro_demo()->DuckResult<SqlMacro>{
    quack_rs::prelude::SqlMacro::scalar("clamp", &["x", "lo", "hi"],
                                        "greatest(lo, least(hi, x))")
}

/// ```sql
/// SELECT add_two_v1(1);
/// ```
///
/// 直接返回 SQL 字符串（`DuckResult<String>`），
/// 注册时通过 `duckfn::register_sql_macro_str` 直接执行。
#[duck_sql_macro]
pub fn sql_macro_str_demo() -> DuckResult<String> {
    Ok("CREATE MACRO add_two_v1(x) AS x + 2".to_string())
}

/// ```sql
/// SELECT add_two_v2(2);
/// ```
///
/// 直接返回静态 SQL 字符串。
#[duck_sql_macro]
pub fn sql_macro_static_str_demo() -> DuckResult<&'static str> {
    Ok("CREATE MACRO add_two_v2(x) AS x + 2")
}

/// ```sql
/// SELECT add_two_v3(3);
/// ```
///
/// 直接返回 `String`。
#[duck_sql_macro]
pub fn sql_macro_plain_str_demo() -> String {
    "CREATE MACRO add_two_v3(x) AS x + 2".to_string()
}

/// ```sql
/// SELECT add_two_v4(4);
/// ```
///
/// 直接返回 `&'static str`。
#[duck_sql_macro]
pub fn sql_macro_plain_static_str_demo() -> &'static str {
    "CREATE MACRO add_two_v4(x) AS x + 2"
}