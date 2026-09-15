use duckfn::{duck_error, DuckResult};

// ============================================================================
// dfn_table_echo_* 的公共规则
//
// 表函数的参数在 bind 阶段按 duckdb_value 读取
// （duckfn/src/functions/table_function_adapter.rs::bind ->
//   DuckArgsImpl::read_by_duck_value），与标量函数「逐行读向量」的机制不同：
//
//   - 参数走 Value API（as_bool / as_blob / list_items / struct_child ...），
//     不按下标访问子向量，所以不会出现标量类型测试里记录的
//     duckdb#25616「常量向量的 child 只有 1 个物理元素」那类读越界。
//   - Value 侧的读取只有 DuckResult、没有 Option：容器里的 NULL 元素在
//     Vec<T> / IndexMap<K, V> / 不可空 struct 字段上会直接变成 bind 阶段错误，
//     而不是像标量函数那样让整个值变成 NULL。
//
// 行数规则（所有 dfn_table_echo_* 一致）：
//   - count 缺省 1（单行回显），负数按 0 处理
//   - 第 0、2、4… 行写出参数值本身，第 1、3、5… 行写出 NULL
//   一次调用就能覆盖「写值 / 写 NULL / 变长子向量按 offset 累加」三条写路径，
//   这是表函数相对标量函数多出来的部分（结果向量按行写）。
// ============================================================================

/// 行数：count 缺省 1，负数按 0 处理
pub fn echo_row_count(count: Option<i64>) -> i64 {
    count.unwrap_or(1).max(0)
}

/// 偶数下标行写出值，奇数下标行写出 NULL
pub fn is_value_row(index: i64) -> bool {
    index % 2 == 0
}

/// 按回显规则把值铺成 count 行
pub fn echo_rows<T, R, F>(value: Option<T>, count: Option<i64>, make: F) -> impl Iterator<Item = R>
where
    T: Clone + Send + 'static,
    F: Fn(Option<T>) -> R + Send,
    R: 'static,
{
    (0..echo_row_count(count)).map(move |i| {
        let v = if is_value_row(i) { value.clone() } else { None };
        make(v)
    })
}

/// LIST -> ARRAY(N)：做定长校验，长度不符时在 bind 阶段报错
///
/// ARRAY 不能作为表函数参数（duck_array.rs 的 read_by_duck_value_valid 返回
/// "Bind value to array type is not supported"），
/// 所以 ARRAY 出参的写路径统一用等价的 LIST 入参。
pub fn to_array<T, const N: usize>(fn_name: &str, v: Vec<T>) -> DuckResult<[T; N]> {
    let len = v.len();
    v.try_into()
        .map_err(|_| duck_error(format!("{fn_name}: expected {N} elements, got {len}")))
}

/// Option<LIST> -> Option<ARRAY(N)>：NULL 入参回显 NULL，其余交给 to_array
pub fn option_to_array<T, const N: usize>(
    fn_name: &str,
    v: Option<Vec<T>>,
) -> DuckResult<Option<[T; N]>> {
    option_to_array_with(fn_name, v, |v| Ok(v))
}

/// Option<LIST<T>> -> Option<ARRAY(U, N)>：元素先经 convert 转成 U，再做定长校验
pub fn option_to_array_with<T, U, const N: usize, F>(
    fn_name: &str,
    v: Option<Vec<T>>,
    convert: F,
) -> DuckResult<Option<[U; N]>>
where
    F: FnMut(T) -> DuckResult<U>,
{
    let v = match v {
        Some(v) => v,
        None => return Ok(None),
    };
    let len = v.len();
    let items: Vec<U> = v.into_iter().map(convert).collect::<DuckResult<Vec<U>>>()?;
    let items: [U; N] = items
        .try_into()
        .map_err(|_| duck_error(format!("{fn_name}: expected {N} elements, got {len}")))?;
    Ok(Some(items))
}
