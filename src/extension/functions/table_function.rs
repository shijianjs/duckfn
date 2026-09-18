use duckfn::{
    duck_custom_register, duck_error, duck_table_function, DuckFullIteratorResult, DuckOptionResult,
    DuckResult, DuckStruct,
};
use indexmap::IndexMap;
use quack_rs::prelude::{Connection, Registrar};

// ============================================================================
// duck_table_function：表函数由「返回类型 + 输出结构体 + 参数」三段决定
//
//   返回类型（duckfn-macro/src/table_function.rs::table_return_type）：
//     -> impl Iterator<Item = Out>              SimpleIterator
//          参数解析成功后不会再失败，也不会产生 NULL
//     -> DuckResult<impl Iterator<Item = Out>>  ResultIterator
//          能处理入参异常（参数语义错误），行数据仍然保证正常
//     -> DuckFullIteratorResult<Out>            Full
//          = DuckResult<Box<dyn Iterator<Item = DuckOptionResult<Out>> + Send>>
//          入参异常 + 行级 Err/NULL 都能处理
//
//   输出结构体：必须 #[derive(DuckStruct)]，字段顺序即结果列顺序，字段名即列名；
//     字段类型决定列类型（Option<T> 是可空列，Vec<Option<T>> 是 LIST 列），
//     嵌套的 DuckStruct 字段就是 STRUCT 列。
//     由 duckfn/src/functions/table_function_adapter.rs 在 bind 阶段
//     （config_result_columns）声明 schema，在 scan 阶段批量写列。
//
//   参数：函数签名里的每个参数都是 DuckArgsImpl 的一个字段。
//     默认全部是位置参数；#[duck_table_function(named_param_from = "字段名")]
//     让该字段及其后面的字段变成命名参数（位置参数必须写在前面）。
//     参数类型写 T 时 NULL/缺省会报 "Parameter x cannot be null"，
//     写 Option<T> 时得到 None —— 可以自己给默认值。
// ============================================================================

/// 基础输出：两列都不可空
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct RangeRow {
    n: i64,
    square: i64,
}

/// 带名字的输出列，用来观察 VARCHAR 列
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct NamedValue {
    name: String,
    value: i64,
}

// ============================================================================
// 返回类型：三种形式
// ============================================================================

/// 简式：参数解析成功后不会再失败，也不会产生 NULL
/// ```sql
/// SELECT * FROM dfn_table_range(3);
/// ```
#[duck_table_function]
fn dfn_table_range(n: i64) -> impl Iterator<Item = RangeRow> {
    (0..n.max(0)).map(|i| RangeRow {
        n: i,
        square: i * i,
    })
}

/// 入参异常：错误在 bind 阶段抛出，整条查询失败
/// ```sql
/// SELECT * FROM dfn_table_checked(3);
/// ```
#[duck_table_function]
fn dfn_table_checked(n: i64) -> DuckResult<impl Iterator<Item = RangeRow>> {
    if n < 0 {
        return Err(duck_error("dfn_table_checked: n must be >= 0"));
    }
    Ok((0..n).map(|i| RangeRow {
        n: i,
        square: i * i,
    }))
}

/// 全功能：入参异常 + 行级错误（Err）+ 行级 NULL（Ok(None)）
/// ```sql
/// SELECT * FROM dfn_table_full(2);
/// ```
#[duck_table_function]
fn dfn_table_full(n: i64) -> DuckFullIteratorResult<RangeRow> {
    let rows = (0..n.max(0)).map(|i| -> DuckOptionResult<RangeRow> {
        match i {
            // Ok(None) -> 这一行的所有列都是 NULL
            1 => Ok(None),
            // 行级错误：写这一行时报错
            2 => Err(duck_error("dfn_table_full: bad row 2")),
            _ => Ok(Some(RangeRow {
                n: i,
                square: i * i,
            })),
        }
    });
    Ok(Box::new(rows))
}

// ============================================================================
// 参数：位置参数 / 命名参数 / 可空参数 / 复杂类型参数
// ============================================================================

/// 零参数表函数：DuckArgsImpl 是空结构体
/// ```sql
/// SELECT * FROM dfn_table_zero_args();
/// ```
#[duck_table_function]
fn dfn_table_zero_args() -> impl Iterator<Item = RangeRow> {
    (0..3).map(|i| RangeRow {
        n: i,
        square: i * i,
    })
}

/// 混合参数：step 是位置参数，start/count 从这里开始变成命名参数
/// ```sql
/// SELECT * FROM dfn_table_countdown(2, start=10, count=3);
/// ```
#[duck_table_function(named_param_from = "start")]
fn dfn_table_countdown(step: i64, start: i64, count: i64) -> impl Iterator<Item = RangeRow> {
    (0..count.max(0)).map(move |i| {
        let n = start - i * step;
        RangeRow {
            n,
            square: n * n,
        }
    })
}

/// 可空命名参数：没传（或传 NULL）就是 None，可以自己兜默认值
/// ```sql
/// SELECT * FROM dfn_table_opt(start=1, step=10, count=2);
/// ```
#[duck_table_function(named_param_from = "start")]
fn dfn_table_opt(
    start: i64,
    step: Option<i64>,
    count: Option<i64>,
) -> impl Iterator<Item = RangeRow> {
    let step = step.unwrap_or(1);
    let count = count.unwrap_or(3);
    (0..count.max(0)).map(move |i| {
        let n = start + i * step;
        RangeRow {
            n,
            square: n * n,
        }
    })
}

/// 不可空命名参数：不传或传 NULL 都会报 "Parameter start cannot be null"
/// ```sql
/// SELECT * FROM dfn_table_req(start=5);
/// ```
#[duck_table_function(named_param_from = "start")]
fn dfn_table_req(start: i64) -> impl Iterator<Item = RangeRow> {
    std::iter::once(RangeRow {
        n: start,
        square: start * start,
    })
}

/// LIST 参数：Vec<i64> -> LIST(BIGINT)，元素不能为 NULL
/// ```sql
/// SELECT * FROM dfn_table_from_list([3, 1, 2]);
/// ```
#[duck_table_function]
fn dfn_table_from_list(values: Vec<i64>) -> impl Iterator<Item = RangeRow> {
    values.into_iter().map(|v| RangeRow {
        n: v,
        square: v * v,
    })
}

/// MAP 参数：IndexMap<String, i64> -> MAP(VARCHAR, BIGINT)
/// ```sql
/// SELECT * FROM dfn_table_from_map(MAP {'a': 1, 'b': 2});
/// ```
#[duck_table_function]
fn dfn_table_from_map(map: IndexMap<String, i64>) -> impl Iterator<Item = NamedValue> {
    map.into_iter().map(|(name, value)| NamedValue { name, value })
}

// ============================================================================
// 输出 schema：多列、可空列、LIST 列、嵌套 STRUCT 列
// ============================================================================

/// 列类型覆盖：BIGINT / VARCHAR / 可空 DOUBLE / LIST(BIGINT)
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct TypedRow {
    id: i64,
    name: String,
    score: Option<f64>,
    tags: Vec<Option<i64>>,
}

/// ```sql
/// SELECT * FROM dfn_table_typed(3);
/// ```
#[duck_table_function]
fn dfn_table_typed(n: i64) -> impl Iterator<Item = TypedRow> {
    (0..n.max(0)).map(|i| TypedRow {
        id: i,
        name: format!("row_{i}"),
        // 半数列是 NULL
        score: if i % 2 == 0 {
            Some(i as f64 / 2.0)
        } else {
            None
        },
        // LIST 列允许元素为 NULL
        tags: (0..i).map(Some).collect(),
    })
}

/// 嵌套输出结构体
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct Point {
    x: i64,
    y: i64,
}

/// STRUCT 列：字段本身是 DuckStruct
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct ShapeRow {
    name: String,
    from_point: Point,
    to_point: Point,
}

/// ```sql
/// SELECT name, from_point, to_point FROM dfn_table_nested(2);
/// ```
#[duck_table_function]
fn dfn_table_nested(n: i64) -> impl Iterator<Item = ShapeRow> {
    (0..n.max(0)).map(|i| ShapeRow {
        name: format!("s{i}"),
        from_point: Point { x: i, y: i + 1 },
        to_point: Point { x: i * 2, y: i * 3 },
    })
}

// ============================================================================
// 注册控制
//
//   #[duck_table_function]                        自动注册
//   #[duck_table_function(auto_register = false)]  只生成 table_function_builder()，
//                                                  注册交给 #[duck_custom_register]
// ============================================================================

/// 只声明、不注册：SQL 层看不到这个名字
/// ```sql
/// SELECT * FROM dfn_table_reg_unregistered(1);
/// ```
#[allow(dead_code)]
#[duck_table_function(auto_register = false)]
fn dfn_table_reg_unregistered(n: i64) -> impl Iterator<Item = RangeRow> {
    (0..n.max(0)).map(|i| RangeRow {
        n: i,
        square: i * i,
    })
}

/// 手动注册
/// ```sql
/// SELECT * FROM dfn_table_reg_manual(2);
/// ```
#[duck_table_function(auto_register = false)]
fn dfn_table_reg_manual(n: i64) -> impl Iterator<Item = RangeRow> {
    (0..n.max(0)).map(|i| RangeRow {
        n: i + 100,
        square: (i + 100) * (i + 100),
    })
}

#[duck_custom_register]
fn dfn_table_reg_manual_register(c: &Connection) -> DuckResult<()> {
    // table_function_builder 返回 DuckResult<TableFunctionBuilder>
    unsafe { c.register_table(dfn_table_reg_manual::table_function_builder()?) }
}
