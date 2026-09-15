---
title: 聚合函数
sidebar_position: 3
description: 聚合函数的行处理函数、状态类型、空输入语义与并行聚合。
---

# 聚合函数

一个聚合函数 = 一个行处理函数 + 一个状态类型。每一行调用一次行处理函数来修改状态，输出由状态决定。

```rust
#[duck_aggregate_function]
fn dfn_agg_sum(input: i64, state: &mut SumState) {
    state.total += input;
    state.rows += 1;
}

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
```

```sql
SELECT dfn_agg_sum(x) FROM (VALUES (1), (2), (3)) t(x);   -- 6
SELECT typeof(dfn_agg_sum(x)) FROM (VALUES (1)) t(x);     -- BIGINT
```

## 行处理函数

- 必须恰好有一个 `&mut 状态` 参数，位置不限。它前后的参数就是 SQL 参数：

  ```rust
  #[duck_aggregate_function]
  fn dfn_agg_state_first(state: &mut SumState, input: i64) {
      state.total += input;
      state.rows += 1;
  }
  ```

- 聚合函数也可以完全不接收参数：

  ```rust
  #[duck_aggregate_function]
  fn dfn_agg_row_count(state: &mut RowCountState) {
      state.rows += 1;
  }
  ```

  ```sql
  SELECT dfn_agg_row_count() FROM (VALUES (1), (2), (3)) t(x);  -- 3
  ```

- 返回 `()` 或 `DuckResult<()>` 都被接受。返回 `Err(duck_error(…))` 会让整条查询失败：

  ```rust
  #[duck_aggregate_function]
  fn dfn_agg_checked(input: i64, state: &mut SumState) -> DuckResult<()> {
      if input == 13 {
          return Err(duck_error("dfn_agg_checked: unlucky input 13"));
      }
      state.total += input;
      state.rows += 1;
      Ok(())
  }
  ```

## 状态类型

状态必须实现 `Default`（DuckDB 可能创建多个状态再合并），并实现 `duckfn::DuckAggregateState`：

| 方法 | 默认实现 | 作用 |
| --- | --- | --- |
| `type Output` | — | 该聚合的 SQL 返回类型。 |
| `simple_result(&self) -> Output` | `todo!()` | 返回结果，不可能为 NULL 时用它。 |
| `simple_combine(&mut self, other)` | `todo!()` | 合并另一个状态，不会失败时用它。 |
| `result(&self) -> DuckOptionResult<Output>` | `Ok(Some(simple_result()))` | 结果可能为 `NULL` 时重写它。 |
| `combine(&mut self, other) -> DuckResult<()>` | `Ok(simple_combine(other))` | 合并可能失败时重写它。 |

只要聚合可能产出 `NULL`，就应当重写 `result` —— 这也是区分「没有行」与「真值」的常规做法。空输入时
`dfn_agg_sum` 返回 `0`，而平均值则无值可返：

```rust
impl DuckAggregateState for AvgState {
    type Output = f64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
        self.rows += other.rows;
    }

    fn result(&self) -> DuckOptionResult<f64> {
        if self.rows == 0 {
            Ok(None)          // 空输入 -> SQL NULL
        } else {
            Ok(Some(self.total as f64 / self.rows as f64))
        }
    }
}
```

```sql
SELECT dfn_agg_avg(x) FROM (VALUES (1), (2), (3)) t(x);   -- 2.0
SELECT dfn_agg_avg(x) FROM (SELECT NULL::BIGINT AS x) t;  -- NULL
SELECT dfn_agg_sum(x) FROM (VALUES (1), (2), (3)) t(x) WHERE x > 10;  -- 0
```

## `combine` 与并行聚合

DuckDB 可能把同一个分组拆到多个线程上，并通过 `combine` 合并部分状态，因此 `simple_combine`（或 `combine`）
必须是正确的。`dfn_agg_sum` 对 10000 行求和，单线程与 `PRAGMA threads=4` 都返回 `49995000`。

输出类型可以是结构化的：`dfn_agg_list` 收集成 `LIST`，空输入返回 `NULL`；`dfn_agg_concat` 则拼接字符串：

```rust
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
```

```sql
SELECT dfn_agg_list(x) FROM (VALUES (1), (2), (3)) t(x);          -- [1, 2, 3]
SELECT dfn_agg_list(x) FROM (VALUES (1), (NULL), (3)) t(x);       -- [1, NULL, 3]
SELECT dfn_agg_concat(x) FROM (VALUES ('a'), ('b'), ('c')) t(x);  -- a,b,c
```

## NULL 输入

默认行为是跳过 `NULL` 行：状态完全不更新。所以 `(1), (NULL), (3)` 上 `dfn_agg_sum` 的结果是 `4`；
想统计 NULL 的个数，就要把入参写成 `Option<i64>`，例如 `dfn_agg_null_count`：

```rust
#[duck_aggregate_function]
fn dfn_agg_mixed(a: i64, b: Option<i64>, state: &mut MixedState) {
    state.sum += a;
    if b.is_none() {
        state.nulls += 1;
    }
}
```

```sql
SELECT dfn_agg_mixed(a, b)
FROM (VALUES (1, 2), (NULL, 5), (3, NULL), (4, 6)) t(a, b);
-- sum:16|nulls:1（a 为 NULL 的那一行被整体跳过）
```

`special_null_handling = true` 的含义与标量函数一致：阻止 DuckDB 折叠常量 `NULL`，让函数体仍然被执行。
`dfn_agg_seen_default` 与 `dfn_agg_seen_special` 只在常量输入上有差别。

## 重载

与标量函数相同，`overloads_name` 可以把多个聚合合并成一个函数集。当各重载需要**不同**返回类型时要注意：
`quack-rs` 的 `AggregateFunctionSetBuilder` 只能在整个函数集上设一个返回类型 —— 此时应改用
`duckfn::DuckfnAggregateFunctionSetBuilder` 配合宏生成的 `aggregate_function_guard()`，它把每个重载注册成
独立的 DuckDB 函数，因此各自保留自己的 `Output`。

## 接下来

- [表函数](./table-functions.md)
- [类型映射](./types.md) —— 状态的 `Output` 可以是哪些类型。
