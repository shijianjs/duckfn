use duckfn::{
    duck_aggregate_function, duck_custom_register, duck_error, DuckAggregateState, DuckOptionResult,
    DuckResult,
};
use quack_rs::prelude::{AggregateFunctionSetBuilder, Connection, LogicalType, Registrar, TypeId};

// ============================================================================
// duck_aggregate_function：参数与状态的拆分
//
// 宏把函数签名里「带 &mut 的参数」识别为聚合状态（FnArgWrapper::is_agg_state），
// 其余参数是逐行的输入列：
//   - 状态参数必须写成 `&mut MyState`，位置任意（可以在中间，见下文的
//     dfn_agg_state_first）；
//   - 输入参数按顺序生成 DuckArgsImpl 的字段，注册时作为位置参数列表
//     （duck_function.rs::build_aggregate_function_impl -> aggregate_function_adapter.rs
//     的 Args::column_types）；
//   - 每个输入参数每行读一次，读取失败（NULL 且参数不是 Option）就整行跳过，
//     update 不执行 —— 语义见下面「入参可空性」一节。
//
// 状态类型需要满足：
//   - `Default + Clone + Debug`（宏生成的结构体 derive 了这三个）；
//   - 实现 duckfn::DuckAggregateState，用 simple_combine/simple_result 或自定义
//     combine/result 描述「合并两个状态」与「状态出结果」；
//   - Output 决定 SQL 返回类型，result() 返回 Ok(None) 即该组结果为 SQL NULL。
// ============================================================================

/// 求和状态：总和 + 参与行数（行数用于区分「空输入」与「和为 0」）
#[derive(Default, Debug, Clone)]
struct SumState {
    total: i64,
    rows: i64,
}

impl DuckAggregateState for SumState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
        self.rows += other.rows;
    }

    fn simple_result(&self) -> Self::Output {
        self.total
    }
}

/// 最简形态：无返回类型，宏在函数调用后补 `; Ok(())`
/// ```sql
/// SELECT dfn_agg_sum(x) FROM (VALUES (1), (2), (3)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_sum(input: i64, state: &mut SumState) {
    state.total += input;
    state.rows += 1;
}

/// 状态参数在最前：宏按签名顺序把输入参数与 `&mut self.state` 依次传进去
/// ```sql
/// SELECT dfn_agg_state_first(x) FROM (VALUES (1), (2)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_state_first(state: &mut SumState, input: i64) {
    state.total += input;
    state.rows += 1;
}

/// 零个输入参数：生成的参数结构体没有字段，注册成无参聚合
/// ```sql
/// SELECT dfn_agg_row_count() FROM (VALUES (1), (2), (3)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_row_count(state: &mut RowCountState) {
    state.rows += 1;
}

/// 计数状态：只统计行数
#[derive(Default, Debug, Clone)]
struct RowCountState {
    rows: i64,
}

impl DuckAggregateState for RowCountState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.rows += other.rows;
    }

    fn simple_result(&self) -> Self::Output {
        self.rows
    }
}

/// 多输入参数：两个参数按位置匹配，状态仍然是同一个
/// ```sql
/// SELECT dfn_agg_weighted(x, w) FROM (VALUES (1, 10), (2, 20)) t(x, w);
/// ```
#[duck_aggregate_function]
fn dfn_agg_weighted(input: i64, weight: i64, state: &mut WeightedState) {
    state.total += input * weight;
}

/// 加权和状态
#[derive(Default, Debug, Clone)]
struct WeightedState {
    total: i64,
}

impl DuckAggregateState for WeightedState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
    }

    fn simple_result(&self) -> Self::Output {
        self.total
    }
}

// ============================================================================
// duck_aggregate_function：入参可空性
//
//   参数类型 `T`（不可空）
//     参数读取层（duckfn/src/duck_columns.rs::read_columns）对非 Option 字段带 `?`，
//     任一参数为 NULL 就返回 None，AggregateFunctionAdapter::handle_row_with_null
//     于是跳过这一行 —— 聚合继续，而不是整组变 NULL。
//
//   参数类型 `Option<T>`（可空）
//     NULL 读成 None 传进函数体，可以「看见」NULL（比如统计 NULL 个数）。
// ============================================================================

/// Option 入参：NULL 以 None 进入函数体，因此可以统计 NULL 的个数
/// ```sql
/// SELECT dfn_agg_null_count(x) FROM (VALUES (1), (NULL), (3)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_null_count(input: Option<i64>, state: &mut NullCountState) {
    state.seen += 1;
    if input.is_none() {
        state.nulls += 1;
    }
}

/// NULL 计数状态
#[derive(Default, Debug, Clone)]
struct NullCountState {
    nulls: i64,
    seen: i64,
}

impl DuckAggregateState for NullCountState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.nulls += other.nulls;
        self.seen += other.seen;
    }

    fn simple_result(&self) -> Self::Output {
        self.nulls
    }
}

/// 混合入参：非 Option 参数把整行拦下，Option 参数把 NULL 带进函数体
/// ```sql
/// SELECT dfn_agg_mixed(a, b) FROM (VALUES (1, 2), (NULL, 5), (3, NULL)) t(a, b);
/// ```
#[duck_aggregate_function]
fn dfn_agg_mixed(a: i64, b: Option<i64>, state: &mut MixedState) {
    state.sum += a;
    match b {
        Some(v) => state.sum += v,
        // a 一定能读到，b 为 NULL 时走这里
        None => state.nulls += 1,
    }
}

/// 混合统计状态，用 String 输出把两个计数都带出来
#[derive(Default, Debug, Clone)]
struct MixedState {
    sum: i64,
    nulls: i64,
}

impl DuckAggregateState for MixedState {
    type Output = String;

    fn simple_combine(&mut self, other: &Self) {
        self.sum += other.sum;
        self.nulls += other.nulls;
    }

    fn simple_result(&self) -> Self::Output {
        format!("sum:{}|nulls:{}", self.sum, self.nulls)
    }
}

// ============================================================================
// duck_aggregate_function：行处理函数的返回形式
//
//   -> ()                默认形式，宏补 `; Ok(())`，行处理不能报错
//   -> DuckResult<()>    函数自己返回 Err，宏直接把返回值当 handle_row 的结果，
//                        错误经 AggregateFunctionInfo::set_error 变成查询错误
//
// 函数体 panic 由 duck_aggregate_unwind 捕获成查询错误，不会跨 FFI 展开。
// ============================================================================

/// 返回 DuckResult<()>：函数体可以按行报错
/// ```sql
/// SELECT dfn_agg_checked(x) FROM (VALUES (1), (2)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_checked(input: i64, state: &mut SumState) -> DuckResult<()> {
    if input == 13 {
        return Err(duck_error("dfn_agg_checked: unlucky input 13"));
    }
    state.total += input;
    state.rows += 1;
    Ok(())
}

/// panic 传播：update 阶段 panic 会变成查询错误
/// ```sql
/// SELECT dfn_agg_panic(13);
/// ```
#[duck_aggregate_function]
fn dfn_agg_panic(input: i64, state: &mut SumState) {
    if input == 13 {
        panic!("dfn_agg_panic: unlucky input 13");
    }
    state.total += input;
    state.rows += 1;
}

// ============================================================================
// duck_aggregate_function：输出类型与结果语义
//
// Output 可以是任意 DuckValueType（i64 / f64 / String / Vec<...> / ...），
// 最终由 AggregateFunctionAdapter::c_finalize 批量写进结果向量。
// 想输出 NULL 就让 result() 返回 Ok(None)；想报错就返回 Err。
// ============================================================================

/// 自定义 combine + result：没有有效行时返回 NULL，而不是 0
/// ```sql
/// SELECT dfn_agg_avg(x) FROM (VALUES (1), (2), (3)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_avg(input: i64, state: &mut AvgState) {
    state.total += input as f64;
    state.rows += 1;
}

/// 平均值状态（覆盖 DuckAggregateState 的 combine/result）
#[derive(Default, Debug, Clone)]
struct AvgState {
    total: f64,
    rows: i64,
}

impl DuckAggregateState for AvgState {
    type Output = f64;

    fn combine(&mut self, other: &Self) -> DuckResult<()> {
        self.total += other.total;
        self.rows += other.rows;
        Ok(())
    }

    fn result(&self) -> DuckOptionResult<f64> {
        if self.rows == 0 {
            // 一行都没读到 → SQL NULL
            Ok(None)
        } else {
            Ok(Some(self.total / self.rows as f64))
        }
    }
}

/// String 输出：拼接所有非 NULL 输入
/// ```sql
/// SELECT dfn_agg_concat(x) FROM (VALUES ('a'), ('b'), ('c')) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_concat(input: String, state: &mut ConcatState) {
    if state.rows > 0 {
        state.text.push(',');
    }
    state.text.push_str(&input);
    state.rows += 1;
}

/// 拼接状态：空输入返回 NULL
#[derive(Default, Debug, Clone)]
struct ConcatState {
    text: String,
    rows: i64,
}

impl DuckAggregateState for ConcatState {
    type Output = String;

    fn simple_combine(&mut self, other: &Self) {
        self.text.push_str(&other.text);
        self.rows += other.rows;
    }

    fn result(&self) -> DuckOptionResult<String> {
        if self.rows == 0 {
            Ok(None)
        } else {
            Ok(Some(self.text.clone()))
        }
    }
}

/// LIST 输出：把所有输入（含 NULL）按顺序收集
/// ```sql
/// SELECT dfn_agg_list(x) FROM (VALUES (1), (2), (3)) t(x);
/// ```
#[duck_aggregate_function]
fn dfn_agg_list(input: Option<i64>, state: &mut ListState) {
    state.values.push(input);
}

/// 收集状态：输出 LIST(BIGINT)
#[derive(Default, Debug, Clone)]
struct ListState {
    values: Vec<Option<i64>>,
}

impl DuckAggregateState for ListState {
    type Output = Vec<Option<i64>>;

    fn simple_combine(&mut self, other: &Self) {
        self.values.extend(other.values.iter().cloned());
    }

    fn result(&self) -> DuckOptionResult<Vec<Option<i64>>> {
        if self.values.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.values.clone()))
        }
    }
}

// ============================================================================
// duck_aggregate_function：注册控制
//
//   #[duck_aggregate_function]                        自动注册（auto_register 默认 true）
//   #[duck_aggregate_function(auto_register = false)]  只生成 builder，注册交给
//                                                      #[duck_custom_register]
//
// auto_register = false 时宏生成：
//   aggregate_function_builder() -> quack_rs::AggregateFunctionBuilder（单签名注册）
//   aggregate_overload_builder(builder) -> OverloadBuilder（挂进函数集做重载）
// ============================================================================

/// auto_register = false 且不手动注册：SQL 层没有这个名字
/// ```sql
/// SELECT dfn_agg_reg_unregistered(1);
/// ```
#[allow(dead_code)]
#[duck_aggregate_function(auto_register = false)]
fn dfn_agg_reg_unregistered(input: i64, state: &mut SumState) {
    state.total += input;
    state.rows += 1;
}

/// 手动注册：直接用生成的 aggregate_function_builder()
/// ```sql
/// SELECT dfn_agg_reg_manual(1);
/// ```
#[duck_aggregate_function(auto_register = false)]
fn dfn_agg_reg_manual(input: i64, state: &mut SumState) {
    state.total += input;
    state.rows += 1;
}

#[duck_custom_register]
fn dfn_agg_reg_manual_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_aggregate(dfn_agg_reg_manual::aggregate_function_builder()) }
}

/// 重载分支 1：INTEGER -> VARCHAR
#[duck_aggregate_function(auto_register = false)]
fn dfn_agg_reg_over_int(input: i32, state: &mut TextState) {
    state.text.push_str(&format!("int({input});"));
}

/// 重载分支 2：VARCHAR -> VARCHAR
#[duck_aggregate_function(auto_register = false)]
fn dfn_agg_reg_over_varchar(input: String, state: &mut TextState) {
    state.text.push_str(&format!("str({input});"));
}

/// 用 AggregateFunctionSetBuilder + aggregate_overload_builder(b) 注册同名重载：
/// 函数集不生成 returns_logical，返回类型要在函数集上设置一次
#[duck_custom_register]
fn dfn_agg_reg_over_register(c: &Connection) -> DuckResult<()> {
    unsafe {
        c.register_aggregate_set(
            AggregateFunctionSetBuilder::new("dfn_agg_reg_overload")
                .returns_logical(LogicalType::new(TypeId::Varchar))
                .overloads(1..=1, |_, b| {
                    dfn_agg_reg_over_int::aggregate_overload_builder(b)
                })
                .overloads(1..=1, |_, b| {
                    dfn_agg_reg_over_varchar::aggregate_overload_builder(b)
                }),
        )
    }
}

/// 重载分支共用的文本状态，空输入返回 NULL
#[derive(Default, Debug, Clone)]
struct TextState {
    text: String,
}

impl DuckAggregateState for TextState {
    type Output = String;

    fn simple_combine(&mut self, other: &Self) {
        self.text.push_str(&other.text);
    }

    fn result(&self) -> DuckOptionResult<String> {
        if self.text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.text.clone()))
        }
    }
}
