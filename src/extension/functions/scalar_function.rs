use duckfn::{duck_custom_register, duck_error, duck_scalar_function, DuckOptionResult, DuckResult};
use quack_rs::prelude::{Connection, Registrar, ScalarFunctionSetBuilder};

// ============================================================================
// duck_scalar_function：三种返回类型形式
//
// 宏按返回类型生成不同的收尾代码（duckfn-macro/src/duck_function.rs::
// build_scalar_return_clause）：
//   -> T                    朴素值，永不为 NULL   => Ok(Some(result))
//   -> Option<T>            可空值                => Ok(result)
//   -> DuckOptionResult<T>  可空值 + 可报错       => result
//
// 三者最终都归到 DuckOptionResult<T> = DuckResult<Option<T>>，Ok(None) 即 SQL NULL，
// Err 让整条查询失败。
//
// 与返回类型无关的另一条约定：入参写成不可空的 `T` 时，NULL 输入由 duckfn 的参数
// 读取层短路成 NULL（函数体不执行）—— 见下面「入参可空性」一节。
// ============================================================================

/// Plain：`-> i32`，宏在函数体外包一层 `Ok(Some(..))`
/// ```sql
/// SELECT dfn_scalar_ret_plain(21);
/// ```
#[duck_scalar_function]
fn dfn_scalar_ret_plain(i: i32) -> i32 {
    i * 2
}

/// Option：`-> Option<i32>`，None 直接落成 SQL NULL
/// ```sql
/// SELECT dfn_scalar_ret_option(i) FROM (VALUES (4), (0), (2)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_scalar_ret_option(i: i32) -> Option<i32> {
    if i == 0 {
        None
    } else {
        Some(100 / i)
    }
}

/// DuckOptionResult：`-> DuckOptionResult<i32>`，可以返回 NULL，也可以返回错误
/// ```sql
/// SELECT dfn_scalar_ret_checked(i) FROM (VALUES (4), (-1)) t(i);
/// ```
#[duck_scalar_function]
fn dfn_scalar_ret_checked(i: i32) -> DuckOptionResult<i32> {
    if i == 0 {
        return Err(duck_error("dfn_scalar_ret_checked: division by zero"));
    }
    if i < 0 {
        return Ok(None);
    }
    Ok(Some(100 / i))
}

/// 函数体 panic 由 duck_scalar_unwind 捕获，消息原样转成查询错误
/// （DuckDB 会再包一层 "Invalid Input Error: "），不会跨 FFI 展开
/// ```sql
/// SELECT dfn_scalar_ret_panic(1);
/// ```
#[duck_scalar_function]
fn dfn_scalar_ret_panic(i: i32) -> i32 {
    if i == 13 {
        panic!("unlucky input: {i}");
    }
    i
}

// ============================================================================
// duck_scalar_function：入参可空性
//
// 参数写 `T` 还是 `Option<T>`，决定 NULL 输入由谁处理：
//
//   参数类型 `T`（不可空）
//     宏生成的参数读取代码对非 Option 字段带 `?`
//     （duckfn/src/duck_columns.rs::read_columns 返回 Option），任一参数为 NULL
//     就整行短路 —— 输出 NULL，函数体不执行。
//
//   参数类型 `Option<T>`（可空）
//     NULL 被读成 None 传进函数体，语义由函数自己决定（返回 NULL、返回默认值、
//     或做 coalesce 之类的处理都可以）。
//
// 这里的 NULL 语义完全由 duckfn 的参数读取层实现。duckfn 注册时沿用 quack-rs
// 的默认 null_handling（DefaultNullHandling，见 ScalarFunctionBuilder::new），
// 但实测 `Option<T>` 参数在入参为 NULL 时仍会进入函数体（下面的用例就用 None 分支
// 的返回值来证明这一点），所以不要指望 DuckDB 侧替我们拦截 NULL。
//
// 「函数体是否执行」用哨兵值 + panic 来证明：函数体一旦读到哨兵值就 panic，
// 查询结果里出现 NULL 而不是报错，即说明该行没有进入函数体。
// ============================================================================

/// 非 Option 入参：任一参数为 NULL 时整行短路
/// ```sql
/// SELECT dfn_scalar_null_arg_plain(a, b) FROM (VALUES (1, 2), (NULL, 2)) t(a, b);
/// ```
#[duck_scalar_function]
fn dfn_scalar_null_arg_plain(a: i32, b: i32) -> i32 {
    if a == -1 || b == -1 {
        panic!("body reached with NULL argument");
    }
    a + b
}

/// Option 入参：NULL 以 None 的形式进入函数体
/// ```sql
/// SELECT dfn_scalar_null_arg_option(a) FROM (VALUES (7), (NULL), (8)) t(a);
/// ```
#[duck_scalar_function]
fn dfn_scalar_null_arg_option(a: Option<i32>) -> i64 {
    match a {
        Some(v) => i64::from(v),
        // NULL 走这里，函数自己决定语义
        None => -1,
    }
}

/// 混合入参：非 Option 参数把 NULL 拦在函数体外，Option 参数把 NULL 带进来
/// ```sql
/// SELECT dfn_scalar_null_arg_mixed(a, b) FROM (VALUES (1, 2), (1, NULL)) t(a, b);
/// ```
#[duck_scalar_function]
fn dfn_scalar_null_arg_mixed(a: i32, b: Option<i32>) -> i64 {
    if a == -1 {
        panic!("body reached with NULL argument");
    }
    match b {
        Some(v) => i64::from(a) * 100 + i64::from(v),
        None => -1,
    }
}

/// 非 Option 参数即使整列 NULL，函数体也一次都不执行
/// ```sql
/// SELECT dfn_scalar_null_arg_all(a) FROM (VALUES (NULL), (NULL)) t(a);
/// ```
#[duck_scalar_function]
fn dfn_scalar_null_arg_all(_a: i32) -> i32 {
    panic!("body reached with NULL argument");
}

// ============================================================================
// duck_scalar_function：参数个数与顺序
//
// 宏把每个参数变成生成结构体的一个字段（字段名 = 参数名，字段类型 = 参数类型），
// 注册时按字段顺序生成位置参数列表（DuckStruct::s_named_columns_type_fn ->
// DuckColumns::column_types -> with_params），所以：
//   - 零参数函数合法：生成空参数结构体 + 空参数列表；
//   - 参数顺序即 SQL 位置参数顺序，类型必须逐个匹配，否则 DuckDB 报
//     "No function matches the given name and argument types ..."；
//   - 参数名只出现在生成结构体里，SQL 层只能用位置参数（见下面「注册控制」一节的
//     named_param_from 用例）。
// ============================================================================

/// 零参数：生成的参数结构体没有字段
/// ```sql
/// SELECT dfn_scalar_arity_zero();
/// ```
#[duck_scalar_function]
fn dfn_scalar_arity_zero() -> i32 {
    42
}

/// 单参数
/// ```sql
/// SELECT dfn_scalar_arity_one(1);
/// ```
#[duck_scalar_function]
fn dfn_scalar_arity_one(a: i32) -> i32 {
    a + 1
}

/// 三参数且类型互不相同，用来验证位置与类型的逐个映射
/// ```sql
/// SELECT dfn_scalar_arity_three(1, 'x', 2.5);
/// ```
#[duck_scalar_function]
fn dfn_scalar_arity_three(a: i32, b: String, c: f64) -> String {
    format!("{a}|{b}|{c}")
}

/// 同类型多参数：验证顺序不被搞混
/// ```sql
/// SELECT dfn_scalar_arity_order(3, 2, 1);
/// ```
#[duck_scalar_function]
fn dfn_scalar_arity_order(a: i32, b: i32, c: i32) -> String {
    format!("{a}-{b}-{c}")
}

// ============================================================================
// duck_scalar_function：注册控制
//
//   #[duck_scalar_function]                        自动注册（auto_register 默认 true）
//   #[duck_scalar_function(auto_register = false)]  只生成 builder，不注册
//
// auto_register = false 时宏仍在以函数名命名的模块里生成：
//   scalar_function_builder()  -> quack_rs::ScalarFunctionBuilder（单签名注册）
//   scalar_overload_builder()  -> quack_rs::ScalarOverloadBuilder（挂进函数集做重载）
// 再配一个 #[duck_custom_register] 函数把 builder 提交到 inventory。
//
// #[duck_custom_register] 要求函数签名正好是 `fn(&Connection) -> DuckResult<()>`
// —— 宏会把函数名本身当作 DuckFunctionItem::register_fn 提交，不做任何包装。
// ============================================================================

/// auto_register = false 且不手动注册：宏不写 inventory 提交，SQL 层永远没有这个名字
/// ```sql
/// SELECT dfn_scalar_reg_unregistered(1);
/// ```
#[allow(dead_code)]
#[duck_scalar_function(auto_register = false)]
fn dfn_scalar_reg_unregistered(i: i32) -> i32 {
    i
}

/// auto_register = false：宏只生成 builder，注册与否由下面的 custom register 决定
/// ```sql
/// SELECT dfn_scalar_reg_manual(1);
/// ```
#[duck_scalar_function(auto_register = false)]
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}

/// 手动注册：直接用生成的 scalar_function_builder()
#[duck_custom_register]
fn dfn_scalar_reg_manual_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_scalar(dfn_scalar_reg_manual::scalar_function_builder()) }
}

/// 重载分支 1：INTEGER -> VARCHAR
#[duck_scalar_function(auto_register = false)]
fn dfn_scalar_reg_over_int(i: i32) -> String {
    format!("integer:{i}")
}

/// 重载分支 2：VARCHAR -> VARCHAR
#[duck_scalar_function(auto_register = false)]
fn dfn_scalar_reg_over_varchar(i: String) -> String {
    format!("varchar:{i}")
}

/// 用 ScalarFunctionSetBuilder + scalar_overload_builder() 注册同名重载
#[duck_custom_register]
fn dfn_scalar_reg_over_register(c: &Connection) -> DuckResult<()> {
    unsafe {
        c.register_scalar_set(
            ScalarFunctionSetBuilder::new("dfn_scalar_reg_overload")
                .overload(dfn_scalar_reg_over_int::scalar_overload_builder())
                .overload(dfn_scalar_reg_over_varchar::scalar_overload_builder()),
        )
    }
}

/// `named_param_from = "b"` 会被写进生成结构体的 `s_named_param_from()`（表函数用它
/// 划命名参数区），而 scalar 的注册只用 `column_types()` 的位置参数列表，因此该属性对
/// scalar function 没有可观察效果：`:=` 里的名字会被 DuckDB 直接忽略，参数按书写顺序
/// 绑定到位置参数（名字对不上也不报错），见同名 .test。
/// ```sql
/// SELECT dfn_scalar_reg_named_param(1, 2);
/// ```
#[duck_scalar_function(named_param_from = "b")]
fn dfn_scalar_reg_named_param(a: i32, b: i32) -> i32 {
    a * 10 + b
}

// ============================================================================
// duck_scalar_function：special_null_handling
//
// 适配层（duckfn/src/functions/scalar_function_adapter.rs::null_handling）默认
// 返回 DefaultNullHandling，即注册时不调用
// duckdb_scalar_function_set_special_handling。
// 属性里显式写 special_null_handling = true 时，宏在 impl 块里覆盖
// null_handling()，改成 SpecialNullHandling，quack-rs 注册时会调用上面那个
// FFI 设置函数，告诉 DuckDB「NULL 输入也交给回调」。
//
// 注意两点，测试正是围绕它们设计的：
//   1. 覆盖只影响监听 DuckDB 的 NULL 语义，函数体能否真的看到 NULL 仍取决于
//      参数是否写成 Option<T> —— 写 T 时读取层会先把 NULL 行短路成 NULL；
//   2. 无论哪种设置，duckfn 自己都不会改变输出：读不到参数就是 Ok(None)。
// ============================================================================

/// 默认 null handling：NULL 参数交给回调处理
/// ```sql
/// SELECT dfn_scalar_null_handling_default(a) FROM (VALUES (7), (NULL), (8)) t(a);
/// ```
#[duck_scalar_function]
fn dfn_scalar_null_handling_default(a: Option<i32>) -> i64 {
    // None 来自 NULL；-1 用来证明函数体确实被调用了
    a.map(i64::from).unwrap_or(-1)
}

/// special_null_handling = true：与上面唯一的差别就是注册时开了 special handling
/// ```sql
/// SELECT dfn_scalar_null_handling_special(a) FROM (VALUES (7), (NULL), (8)) t(a);
/// ```
#[duck_scalar_function(special_null_handling = true)]
fn dfn_scalar_null_handling_special(a: Option<i32>) -> i64 {
    a.map(i64::from).unwrap_or(-1)
}

/// special_null_handling = true + 非 Option 入参：
/// 即使 DuckDB 把 NULL 放进来，读取层仍会短路，函数体不执行
/// ```sql
/// SELECT dfn_scalar_null_handling_special_plain(a) FROM (VALUES (7), (NULL), (8)) t(a);
/// ```
#[duck_scalar_function(special_null_handling = true)]
fn dfn_scalar_null_handling_special_plain(a: i32) -> i32 {
    if a == -1 {
        panic!("dfn_scalar_null_handling_special_plain: body reached with NULL argument");
    }
    a * 2
}
