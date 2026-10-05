---
title: Aggregate functions
sidebar_position: 3
description: Aggregate row handlers, state types, empty input semantics, parallel aggregation and the collect-then-compute shortcut (auto_collect).
---

# Aggregate functions

An aggregate function is a row handler plus a state type. The handler is called once per row and
mutates the state; the state decides the output.

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

```sql {"type":"duckfn"}
SELECT dfn_agg_sum(x) FROM (VALUES (1), (2), (3)) t(x);   -- 6
```

```sql {"type":"duckfn"}
SELECT typeof(dfn_agg_sum(x)) FROM (VALUES (1)) t(x);     -- BIGINT
```

## The row handler

- Exactly one argument must be `&mut State`; its position is free. Arguments before and after it are
  the SQL parameters:

  ```rust
  #[duck_aggregate_function]
  fn dfn_agg_state_first(state: &mut SumState, input: i64) {
      state.total += input;
      state.rows += 1;
  }
  ```

- An aggregate may take no argument at all:

  ```rust
  #[duck_aggregate_function]
  fn dfn_agg_row_count(state: &mut RowCountState) {
      state.rows += 1;
  }
  ```

  ```sql
  SELECT dfn_agg_row_count() FROM (VALUES (1), (2), (3)) t(x);  -- 3
  ```

- Returning `()` and returning `DuckResult<()>` are both accepted. Return `Err(duck_error(…))` to
  fail the whole query:

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

## The state type

The state must derive `Default` (DuckDB may create several and combine them) and implement
`duckfn::DuckAggregateState`:

| Method | Default | Purpose |
| --- | --- | --- |
| `type Output` | — | The SQL return type of the aggregate. |
| `simple_result(&self) -> Output` | `todo!()` | Return the result, no NULL possible. |
| `simple_combine(&mut self, other)` | `todo!()` | Merge another state, no error possible. |
| `result(&self) -> DuckOptionResult<Output>` | `Ok(Some(simple_result()))` | Override when the result may be `NULL`. |
| `combine(&mut self, other) -> DuckResult<()>` | `Ok(simple_combine(other))` | Override when combining may fail. |

Override `result` whenever the aggregate can produce `NULL`, which is the usual way to distinguish
"no rows" from a real value. `dfn_agg_sum` over an empty input returns `0`, whereas an average has
nothing to return:

```rust
impl DuckAggregateState for AvgState {
    type Output = f64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
        self.rows += other.rows;
    }

    fn result(&self) -> DuckOptionResult<f64> {
        if self.rows == 0 {
            Ok(None)          // empty input -> SQL NULL
        } else {
            Ok(Some(self.total as f64 / self.rows as f64))
        }
    }
}
```

```sql {"type":"duckfn"}
SELECT dfn_agg_avg(x) FROM (VALUES (1), (2), (3)) t(x);   -- 2.0
```

```sql {"type":"duckfn"}
SELECT dfn_agg_avg(x) FROM (SELECT NULL::BIGINT AS x) t;  -- NULL
```

```sql {"type":"duckfn"}
SELECT dfn_agg_sum(x) FROM (VALUES (1), (2), (3)) t(x) WHERE x > 10;  -- 0
```

## `combine` and parallel aggregation

DuckDB may split a group across threads and merge partial states through `combine`, so
`simple_combine` (or `combine`) has to be correct. `dfn_agg_sum` over 10 000 rows returns
`49995000` both with a single thread and with `PRAGMA threads=4`.

Output types can be structured: `dfn_agg_list` collects into a `LIST` and reports `NULL` for an
empty input, and `dfn_agg_concat` concatenates strings:

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

```sql {"type":"duckfn"}
SELECT dfn_agg_list(x) FROM (VALUES (1), (2), (3)) t(x);          -- [1, 2, 3]
```

```sql {"type":"duckfn"}
SELECT dfn_agg_list(x) FROM (VALUES (1), (NULL), (3)) t(x);       -- [1, NULL, 3]
```

```sql {"type":"duckfn"}
SELECT dfn_agg_concat(x) FROM (VALUES ('a'), ('b'), ('c')) t(x);  -- a,b,c
```

## NULL input

The default is to skip `NULL` rows: the state is not updated at all. That is why `dfn_agg_sum` over
`(1), (NULL), (3)` is `4`, and why `dfn_agg_null_count` — which declares `Option<i64>` — is the way
to count them:

```rust
#[duck_aggregate_function]
fn dfn_agg_mixed(a: i64, b: Option<i64>, state: &mut MixedState) {
    state.sum += a;
    if b.is_none() {
        state.nulls += 1;
    }
}
```

```sql {"type":"duckfn"}
SELECT dfn_agg_mixed(a, b)
FROM (VALUES (1, 2), (NULL, 5), (3, NULL), (4, 6)) t(a, b);
-- sum:16|nulls:1   (the row with a = NULL is skipped entirely)
```

`special_null_handling = true` has the same meaning as for scalar functions: it stops DuckDB from
folding constant `NULL`s, so the body still runs. `dfn_agg_seen_default` and
`dfn_agg_seen_special` differ only for constant inputs.

## A constant, expensive argument

The argument struct is rebuilt for every row, so an argument that never changes still pays its full
parse cost 5000 times over 5000 rows. When that argument is expensive — a configuration struct ten times
heavier than the values being aggregated — wrap it in [`DuckLazy<T>`](./types.md#lazy-arguments) and keep
the parsed value in a `DuckLazySlot<T>`:

```rust
#[derive(Default, Debug, Clone)]
struct WeightedState {
    config: DuckLazySlot<Config>,
    sum: f64,
}

#[duck_aggregate_function]
fn dfn_agg_weighted(cfg: DuckLazy<Config>, v: i64, state: &mut WeightedState) -> DuckResult<()> {
    // Parsed once, on the first row; every later row only bumps a refcount.
    let config = state.config.resolve(&cfg)?;
    state.sum += config.weight(v);
    Ok(())
}

impl DuckAggregateState for WeightedState {
    type Output = f64;

    fn simple_combine(&mut self, other: &Self) {
        // Both sides parsed the same column: carry the result over, never re-parse.
        self.config.combine(&other.config);
        self.sum += other.sum;
    }

    fn simple_result(&self) -> f64 {
        self.sum
    }
}
```

`resolve` parses **once** and hands back an `Arc<Config>`; every later row is a refcount bump. The slot
caches the **parsed value**, never the token, because `DuckLazy<T>` is only valid inside the callback
that produced it — a token kept in the state fails the query later (with a `DuckLazy<T> is stale: ...`
error) instead of reading a stale vector.

A nullable argument is written `Option<DuckLazy<Config>>` and read with `resolve_optional`, which
returns `Ok(None)` when the cell is `NULL`; `get()` reads the slot back from `result()` (it yields
`Option<Arc<Config>>`, and `None` covers both "never parsed" and "parsed as `NULL`").

That last part is the trap in `result()`: an **empty group** and a `NULL` argument both leave `get()`
at `None`, and they mean opposite things. Decide from the row count first, and only then read the
slot — otherwise "no rows at all" quietly turns into "the configuration was `NULL`":

```rust
fn result(&self) -> DuckOptionResult<f64> {
    if self.rows == 0 {
        return Ok(None);                    // empty group: there is nothing to report
    }
    let config = self.config.get()          // only here does None really mean the argument was NULL
        .ok_or_else(|| duck_error("dfn_agg_weighted: config must not be NULL"))?;
    Ok(Some(self.sum * config.scale()))
}
```

The slot cannot tell the two cases apart by itself: it only records what the row handler saw, and on
an empty group the row handler never ran at all. `result()` is where the two are separated, and the
row count (`self.rows`, the same counter that distinguishes "no rows" from a real value further up)
is how.

The example extension measures both spellings in
`test/sql/demo/lazy_config_demo.test`: over 5000 rows the `DuckLazy` argument parses the
configuration **once** while the eager `Config` argument parses it **5000 times**, with identical
results. The same file also covers the nullable argument and a
`PRAGMA threads=4` run, where merging the partial states must carry the configuration over rather than
re-parse (or lose) it.

## Collect first, compute once (`auto_collect`)

"Collect a column, then evaluate a single value" is one of the most common aggregate shapes: the
state's only job is to `push` every row into a `Vec`, and `finalize` hands the slice to a plain
function. Writing that by hand — a state struct, a `DuckAggregateState` impl and a one-line row
handler — is pure boilerplate. `auto_collect = true` turns the annotated function into the **finalize
handler** and hides the collection entirely:

- a `Vec<T>` parameter is a column to collect across rows;
- a `DuckFirst<T>` parameter is a per-query constant resolved once (the lazy argument above, minus
  the manual `DuckLazySlot`);
- the return value is the aggregate result, following the same rules as scalar functions (`-> T`,
  `-> Option<T>` or `-> DuckOptionResult<T>`).

There is **no `&mut State` parameter** — the macro builds the state for you in exactly the
`SummaryState` shape from further up (a `Vec<T>` per collected column, merged in `simple_combine`,
evaluated in `result`). Everything else keeps working unchanged: parallel `combine`, NULL
propagation, structured output types and `overloads_name`.

```rust
#[duck_aggregate_function(
    auto_collect = true,
    description = "Sample covariance of two DOUBLE columns (Bessel-corrected)",
    comment = "A row with a NULL in either column is skipped, keeping the two columns paired",
    example = "SELECT dfn_agg_covariance(x, y) FROM (VALUES (0.0,-5.0),(3.0,4.0),(-2.0,10.0)) t(x, y)"
)]
fn dfn_agg_covariance(x: Vec<f64>, y: Vec<f64>) -> DuckOptionResult<f64> {
    let n = x.len();
    if n < 2 {
        return Ok(None);          // fewer than two paired rows -> NULL
    }
    let nf = n as f64;
    let mean_x = x.iter().sum::<f64>() / nf;
    let mean_y = y.iter().sum::<f64>() / nf;
    Ok(Some(
        x.iter().zip(y.iter()).map(|(a, b)| (a - mean_x) * (b - mean_y)).sum::<f64>() / (nf - 1.0),
    ))
}
```

```sql {"type":"duckfn"}
SELECT dfn_agg_covariance(x, y) FROM (VALUES (0.0,-5.0),(3.0,4.0),(-2.0,10.0)) t(x, y);   -- -5.5
```

NULL propagation is the same rule as everywhere else: a NULL in any **non-`Option`** column drops the
whole row from the collection (which is what keeps the two columns paired above). To keep the NULLs
inside the collected vector instead, declare `Vec<Option<T>>`:

```rust
#[duck_aggregate_function(auto_collect = true)]
fn dfn_agg_count_present(values: Vec<Option<f64>>) -> i64 {
    values.iter().filter(|v| v.is_some()).count() as i64
}
```

```sql {"type":"duckfn"}
SELECT dfn_agg_count_present(x) FROM (VALUES (1.0), (NULL), (3.0)) t(x);   -- 2
```

`DuckFirst<T>` is the per-query constant, read once through the lazy machinery underneath. It is an
identity alias — the value type is just `T` — so the function receives a plain value: `DuckFirst<f64>`
gives an `f64`, and a nullable constant is written `DuckFirst<Option<T>>` (an all-`NULL` column then
yields `None`). A non-nullable `DuckFirst<T>` on an **empty group** has no value to resolve, so
`auto_collect` reports `NULL` for that group rather than calling the function:

```rust
#[duck_aggregate_function(auto_collect = true)]
fn dfn_agg_scaled(values: Vec<f64>, factor: DuckFirst<f64>) -> f64 {
    values.iter().sum::<f64>() * factor
}
```

```sql {"type":"duckfn"}
SELECT dfn_agg_scaled(x, 2.0) FROM (VALUES (1.0), (2.0), (3.0)) t(x);   -- 12.0
```

A `DuckFirst<Option<T>>` is not limited to scalars — a whole `#[derive(DuckStruct)]` configuration
that stays constant across the query is exactly the case it saves you the manual slot for.

`auto_collect` composes with every other argument (`overloads_name`, `auto_register`,
`special_null_handling`, the documentation keys): it is the same aggregate you would have written by
hand, just generated from the finalize function instead.

## Overloads

As with scalar functions, `overloads_name` merges several aggregates into one function set. Build it
with `quack-rs`'s `AggregateFunctionSetBuilder` and per-overload `AggregateOverloadBuilder`s: each
macro-generated `aggregate_overload_builder(builder)` returns one overload, and since `quack-rs` 0.18
every overload carries its own return type, so one set may hold overloads with *different* `Output`s.

Each generated module also exports `SQL_NAME`: with `overloads_name` set it is the **function-set
name**, otherwise the function name. Error messages that a user sees should be prefixed with it —
`format!("{}: ...", duckfn_agg_html::SQL_NAME)` — rather than with a hand-written copy of the
attribute's string literal, which is exactly the copy that drifts.

## Source and tests

- [`test/extension/functions/aggregate_function.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/aggregate_function.rs) — the example aggregates and their state types
- [`test/extension/functions/aggregate_auto_collect.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/aggregate_auto_collect.rs) — the `auto_collect` examples (`Vec<T>` columns, `DuckFirst<T>` constants)
- [`test/sql/functions/aggregate_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/aggregate_function.test) — the expected results
- [`test/sql/functions/aggregate_auto_collect.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/aggregate_auto_collect.test) — the expected `auto_collect` results
- [`src/functions/aggregate_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/aggregate_function_adapter.rs) — the runtime side
- [`src/value_types/duck_lazy_slot.rs`](https://github.com/shijianjs/duckfn/blob/main/src/value_types/duck_lazy_slot.rs) — `DuckLazySlot<T>`, the parse-once slot used above

## Next

- [Table functions](./table-functions.md)
- [Type mapping](./types.md) — what a state's `Output` may be.
- [File system access](./file-system.md) — reading files (`s3://`, `http(s)://`, local disk) from a row handler or `finalize`.
