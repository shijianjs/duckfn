use duckfn::{duck_cast_function, duck_custom_register, duck_error, DuckOptionResult, DuckResult};
use quack_rs::prelude::Connection;

// ============================================================================
// duck_cast_function：注册 `CAST(源 AS 目标)` 的转换实现
//
// 宏按签名推断「源类型 -> 目标类型」：
//   - 源类型 = 唯一参数的 Rust 类型（DuckValueType，如 String / i64 / Vec<T> ...）；
//   - 目标类型 = 返回类型，形式与 duck_scalar_function 一致：
//       -> T                   永不为 NULL
//       -> Option<T>           None 落成 NULL
//       -> DuckOptionResult<T> None 落成 NULL，Err 报错
//
// 适配层（src/functions/cast_function_adapter.rs）把 quack-rs 的向量级回调
// 拆成逐行的 Rust 调用，并按 DuckDB 的 cast mode 分流错误：
//   CAST(x AS T)      出错 -> set_error，整条查询失败
//   TRY_CAST(x AS T)  出错 -> set_row_error + 该行写 NULL，继续处理后面的行
// 函数体 panic 由 catch_unwind 捕获成查询错误，不会跨 FFI 展开。
//
// 入参可空性沿用 duckfn 的约定：
//   参数 T          NULL 输入直接输出 NULL，函数体不执行；
//   参数 Option<T>  NULL 以 None 进入函数体，语义由函数自己决定。
//
// 注册到同一个 (源, 目标) 会覆盖/参与竞争 DuckDB 的内置转换 —— 下面几个用例正是
// 用「与内置行为不同」的结果来证明走的是我们的回调。
//
// 警告：这里的 (源, 目标) 都是内置就有转换的类型对，覆盖语义会影响**加载了本扩展的
// 整个数据库**（.test 每个文件跑在独立内存库上，所以只有本文件受影响）。真实扩展里
// 应只对自定义类型、或确实需要改语义的类型对注册 cast，不要照抄这里的「×2 / ÷2」
// 之类演示语义。
//
// 属性参数：
//   implicit_cost = N  设置隐式转换代价，DuckDB 可能自动插入该转换（值越小优先级越高）
//   auto_register = false  只生成 builder，注册交给 #[duck_custom_register]
// ============================================================================

// ============================================================================
// 返回形式 1/3：-> DuckOptionResult<T>（可 NULL、可报错）
// ============================================================================

/// String(VARCHAR) -> i32(INTEGER)：空串当 NULL，解析失败报错
/// ```sql
/// SELECT CAST('42' AS INTEGER);
/// SELECT CAST('' AS INTEGER);
/// SELECT TRY_CAST('abc' AS INTEGER);
/// ```
#[duck_cast_function]
fn dfn_cast_str_to_int(s: String) -> DuckOptionResult<i32> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        // 覆盖内置行为：内置 CAST('' AS INTEGER) 会报错，这里给 NULL
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| {
        duck_error(format!("dfn_cast_str_to_int: not an integer: {s:?}"))
    })
}

// ============================================================================
// 返回形式 2/3：-> T（永不为 NULL，函数体 panic 变成查询错误）
// ============================================================================

/// f64(DOUBLE) -> i64(BIGINT)：四舍五入（内置行为是截断）
/// ```sql
/// SELECT CAST(2.5::DOUBLE AS BIGINT);
/// SELECT CAST('NaN'::DOUBLE AS BIGINT);
/// ```
#[duck_cast_function]
fn dfn_cast_double_to_bigint(v: f64) -> i64 {
    if !v.is_finite() {
        panic!("dfn_cast_double_to_bigint: not a finite number: {v}");
    }
    v.round() as i64
}

// ============================================================================
// 返回形式 3/3 + Option 入参：-> Option<T>，NULL 以 None 进入函数体
// ============================================================================

/// i64(BIGINT) -> f64(DOUBLE)：NULL 不再短路，而是以 None 进函数体（用哨兵证明）
/// ```sql
/// SELECT CAST(3::BIGINT AS DOUBLE);
/// SELECT CAST(NULL::BIGINT AS DOUBLE);
/// ```
#[duck_cast_function]
fn dfn_cast_bigint_to_double(v: Option<i64>) -> Option<f64> {
    match v {
        // 演示语义（和内置的 v as f64 不同），用来说明覆盖确实生效
        Some(v) => Some(v as f64 / 2.0),
        // NULL 走了这条分支 -> 返回 -1.0 这个哨兵值
        None => Some(-1.0),
    }
}

// ============================================================================
// implicit_cost：让 DuckDB 在需要隐式转换时自动使用这个 cast
//
// 内置没有 VARCHAR -> HUGEINT 的隐式转换，设置 implicit_cost 后
// `CAST('41' AS VARCHAR) + 1::HUGEINT` 这种表达式才能解析：
//   typeof(...) = HUGEINT
// 显式 CAST 依然走 CAST。
//
// 注意：隐式转换是**整个数据库**级别的影响，选型时要么挂在别的转换不需要的类型上，
// 要么接受它会改变绑定的后果（例如给 VARCHAR -> BIGINT 加隐式代价后，
// `dfn_agg_sum('x')` 会从「无匹配函数」变成「转换失败」）。
// ============================================================================

/// String(VARCHAR) -> i128(HUGEINT)，隐式转换代价 100
/// ```sql
/// SELECT CAST('41' AS VARCHAR) + 1::HUGEINT;
/// SELECT CAST('41' AS HUGEINT);
/// ```
#[duck_cast_function(implicit_cost = 100)]
fn dfn_cast_str_to_hugeint(s: String) -> i128 {
    s.trim()
        .parse::<i128>()
        .unwrap_or_else(|_| panic!("dfn_cast_str_to_hugeint: not an integer: {s:?}"))
}

// ============================================================================
// 复杂类型：LIST(VARCHAR) -> LIST(INTEGER)
//
// 源/目标类型统一走 DuckValueType::logical_type()，所以 Vec<Option<String>> /
// Vec<Option<i32>> 这类嵌套类型也能直接用（注册时用 new_logical）。
// ============================================================================

/// 逐元素解析；NULL 元素保持 NULL；任一个元素解析失败 -> 报错（TRY_CAST 时该行变 NULL）
/// ```sql
/// SELECT CAST(['1', '2'] AS INTEGER[]);
/// SELECT CAST([NULL, '2'] AS INTEGER[]);
/// SELECT TRY_CAST(['1', 'x'] AS INTEGER[]);
/// ```
#[duck_cast_function]
fn dfn_cast_list_str_to_int(v: Vec<Option<String>>) -> DuckOptionResult<Vec<Option<i32>>> {
    let mut out: Vec<Option<i32>> = Vec::with_capacity(v.len());
    for s in &v {
        match s {
            None => out.push(None),
            Some(s) => out.push(Some(s.trim().parse::<i32>().map_err(|_| {
                duck_error(format!("dfn_cast_list_str_to_int: not an integer: {s:?}"))
            })?)),
        }
    }
    Ok(Some(out))
}

// ============================================================================
// 注册控制：auto_register = false 与手动注册
// ============================================================================

/// 只声明、不注册：TINYINT -> BIGINT 仍然走内置转换
/// ```sql
/// SELECT CAST(3::TINYINT AS BIGINT);
/// ```
#[allow(dead_code)]
#[duck_cast_function(auto_register = false)]
fn dfn_cast_unregistered(v: i8) -> i64 {
    i64::from(v) * 100
}

/// 手动注册：SMALLINT -> BIGINT 被替换成 ×10
/// ```sql
/// SELECT CAST(3::SMALLINT AS BIGINT);
/// ```
#[duck_cast_function(auto_register = false)]
fn dfn_cast_manual(v: i16) -> i64 {
    i64::from(v) * 10
}

#[duck_custom_register]
fn dfn_cast_manual_register(c: &Connection) -> DuckResult<()> {
    dfn_cast_manual::cast_function_register(c)
}
