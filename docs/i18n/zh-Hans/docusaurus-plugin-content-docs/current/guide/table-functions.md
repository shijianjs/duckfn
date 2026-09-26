---
title: 表函数
sidebar_position: 4
description: 行结构体、三种迭代器返回形态、命名参数与可选参数，以及流式输出。
---

# 表函数

表函数返回一个行结构体，结构体的字段就是输出列。

```rust
#[duck_table_function]
fn dfn_table_range(n: i64) -> impl Iterator<Item = RangeRow> {
    (0..n.max(0)).map(|i| RangeRow {
        n: i,
        square: i * i,
    })
}

#[derive(Default, Debug, Clone, DuckStruct)]
pub struct RangeRow {
    n: i64,
    square: i64,
}
```

```sql
SELECT * FROM dfn_table_range(3);
```

```text
┌───────┬────────┐
│   n   │ square │
├───────┼────────┤
│     0 │      0 │
│     1 │      1 │
│     2 │      4 │
└───────┴────────┘
```

行结构体就是普通的 `#[derive(DuckStruct)]` 类型，因此列可以是可空的（`Option<T>`）、列表、映射、数组，
也可以是嵌套结构体。

## 三种返回形态

| 形态 | 适用场景 |
| --- | --- |
| `impl Iterator<Item = Row>` | bind 阶段不可能失败，且没有行会是 `NULL`。 |
| `DuckResult<impl Iterator<Item = Row>>` | 参数校验可能在任何行产出之前失败。 |
| `DuckFullIteratorResult<Row>` | 单行可以是 `NULL`，或者单行可能失败。 |

`DuckFullIteratorResult<Row>` 是 `DuckResult<Box<dyn Iterator<Item = DuckOptionResult<Row>> + Send>>`，
于是迭代器对每一行有三种结果：

```rust
#[duck_table_function]
fn dfn_table_full(n: i64) -> DuckFullIteratorResult<RangeRow> {
    let rows = (0..n.max(0)).map(|i| -> DuckOptionResult<RangeRow> {
        match i {
            1 => Ok(None),                                     // 这一行全为 NULL
            2 => Err(duck_error("dfn_table_full: bad row 2")),  // 这一行报错
            _ => Ok(Some(RangeRow { n: i, square: i * i })),
        }
    });
    Ok(Box::new(rows))
}
```

```sql
SELECT * FROM dfn_table_full(2);   -- 先 0 0，再 NULL NULL
SELECT * FROM dfn_table_full(5);   -- 报错：dfn_table_full: bad row 2
```

参数校验发生在第一行产出之前，因此中间那种形态最适合承载校验：

```rust
#[duck_table_function]
fn dfn_table_checked(n: i64) -> DuckResult<impl Iterator<Item = RangeRow>> {
    if n < 0 {
        return Err(duck_error("dfn_table_checked: n must be >= 0"));
    }
    Ok((0..n).map(|i| RangeRow { n: i, square: i * i }))
}
```

```sql
SELECT * FROM dfn_table_checked(-1);  -- 报错：dfn_table_checked: n must be >= 0
```

## 位置参数与命名参数

默认所有参数都是位置参数。`named_param_from` 指定从哪个参数开始改用具名传递：

```rust
#[duck_table_function(named_param_from = "start")]
fn dfn_table_countdown(step: i64, start: i64, count: i64) -> impl Iterator<Item = RangeRow> {
    (0..count.max(0)).map(move |i| {
        let n = start - i * step;
        RangeRow { n, square: n * n }
    })
}
```

```sql
SELECT * FROM dfn_table_countdown(2, start=10, count=3);        -- 10, 8, 6
SELECT * FROM dfn_table_countdown(step=2, start=10, count=3);   -- 报错：No function matches
SELECT * FROM dfn_table_countdown(2, start=10, count=3, foo=1); -- 报错：Invalid named parameter "foo"
```

标记之前的参数不能用名字传递；不认识的名字会被拒绝而不是忽略。

## 可选参数与必填参数

`Option<T>` 让命名参数变成可选 —— 传 `NULL` 与不传都会以 `None` 到达：

```rust
#[duck_table_function(named_param_from = "start")]
fn dfn_table_opt(start: i64, step: Option<i64>, count: Option<i64>) -> impl Iterator<Item = RangeRow> {
    let step = step.unwrap_or(1);
    let count = count.unwrap_or(3);
    /* … */
}
```

```sql
SELECT * FROM dfn_table_opt(start=1);                         -- 1, 2, 3
SELECT * FROM dfn_table_opt(start=1, step=10, count=2);        -- 1, 11
SELECT * FROM dfn_table_opt(start=1, step=NULL, count=NULL);   -- 1, 2, 3
```

写成普通 `T` 的参数则是必填的。不传或传 `NULL` 会在 bind 阶段失败：

```sql
SELECT * FROM dfn_table_req();           -- 报错：Parameter start cannot be null
SELECT * FROM dfn_table_req(start=NULL); -- 报错：Parameter start cannot be null
SELECT * FROM dfn_table_req(start=5);    -- 5, 25
```

## 输出列

[类型映射](./types.md)里的任意字段类型都可以作为列，包括可空字段、列表与嵌套结构体：

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct TypedRow {
    id: i64,
    name: String,
    score: Option<f64>,
    tags: Vec<Option<i64>>,
}
```

```sql
DESCRIBE SELECT * FROM dfn_table_typed(3);
-- id     BIGINT
-- name   VARCHAR
-- score  DOUBLE
-- tags   BIGINT[]

SELECT id, score FROM dfn_table_typed(3);
-- 0 0.0
-- 1 NULL
-- 2 1.0
```

嵌套结构体会成为 `STRUCT` 列，可以逐字段取用：

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct ShapeRow {
    name: String,
    from_point: Point,
    to_point: Point,
}
```

```sql
SELECT name, from_point.x, to_point.y FROM dfn_table_nested(2);
-- s0 0 0
-- s1 1 3
```

## 参数

表函数可以完全不接收参数，也可以接收复杂类型：`LIST` 参数（`Vec<i64>`）或 `MAP` 参数
（`IndexMap<String, i64>`）：

```sql
SELECT * FROM dfn_table_zero_args();                    -- 0 0 / 1 1 / 2 4
SELECT * FROM dfn_table_from_list([3, 1, 2]);           -- 3 9 / 1 1 / 2 4
SELECT * FROM dfn_table_from_map(MAP {'a': 1, 'b': 2}); -- a 1 / b 2
```

:::note[限制]

- 参数是 *bind* 参数，因此 `Vec<T>`（元素类型写 `T`，即不可空）里出现 `NULL` 会报错，而不是跳过该元素；
  需要接受 `NULL` 元素请写成 `Vec<Option<T>>`。
- `ARRAY` 类型不能作为 bind 参数：
  `SELECT * FROM dfn_table_echo_array_integer_param([1,2,3]::INTEGER[3])` 会报
  `Bind value to array type is not supported`。
- 表函数的列不能作为 lateral join 的参数使用。
:::

## 流式输出

行是惰性产出的，一次一个 DuckDB vector，因此大结果集不必整体物化。
`SELECT count(*) FROM dfn_table_range(2048)` 返回 `2048`，再大一号的规模也一样 —— 迭代器被一直拉到穷尽为止。

## 动态列

有时列直到 bind 阶段才知道 —— 由文件头、字典表或远端 schema 决定。`dynamic_columns = true`
把整个 schema 的决定权搬进 bind：

```rust
#[duck_table_function(dynamic_columns = true)]
fn dfn_table_dynamic(source: String, n: i64) -> DuckResult<DuckDynamicTable> {
    // bind 阶段：读外部元数据、把 schema 定下来
    let schema = DuckResultSchema::new(vec![
        ("id".to_string(), DuckTypeDesc::scalar(TypeId::BigInt)),
        (
            "tags".to_string(),
            DuckTypeDesc::list(DuckTypeDesc::scalar(TypeId::Varchar)),
        ),
    ]);

    let rows = (0..n.max(0)).map(|i| -> DuckOptionResult<DuckDynamicRow> {
        Ok(Some(DuckDynamicRow::new(vec![
            Some(DuckDynamicValue::BigInt(i)),
            Some(DuckDynamicValue::list([
                Some(DuckDynamicValue::Varchar("t".to_string())),
            ])),
        ])))
    });

    Ok(DuckDynamicTable::new(schema, Box::new(rows)))
}
```

返回类型是 `DuckDynamicTable`（或 `DuckResult<DuckDynamicTable>`）：schema 加一个行迭代器。`bind`
负责读元数据、声明列，并把迭代器交给 scan；scan 依旧一次写一个 DuckDB vector，不整表物化。

- `DuckTypeDesc` 是可跨线程保存的递归类型描述（`Scalar` / `Decimal` / `List` / `Struct` /
  `Map`）。`to_logical_type()` 把它转成 DuckDB 逻辑类型交给 `add_result_column_with_type`；
  `from_logical_type()` 则反过来，用于 schema 本身就来自 DuckDB 的场景。
- `DuckDynamicValue` 只携带数据 —— 类型一律来自 schema，因此只有一处真相。每个单元格都是
  `Option`：`None` 就是 SQL NULL。
- `DuckDynamicRow::write_batch` 在写向量前会按列描述校验每个值，因此类型错配是可读的错误，
  而不是写坏向量。

```sql
DESCRIBE SELECT * FROM dfn_table_dynamic('sales', 1);
-- id      BIGINT
-- region  VARCHAR
-- amount  DOUBLE
-- tags    VARCHAR[]
-- info    STRUCT(host VARCHAR, code BIGINT)
-- attrs   MAP(VARCHAR, BIGINT)

SELECT id, len(tags), info.host FROM dfn_table_dynamic('sales', 6);
-- 0 0    host-0
-- 1 1    host-1
-- 2 2    NULL
-- 3 3    host-0
-- 4 NULL host-1
-- 5 1    NULL
```

零行结果同样会声明出列，因为 schema 只来自元数据、从不来自数据。

### 底层用法

`DuckDynamicTable` 也可以手写产出：实现 `DynamicTableFunctionAdapter`（只要求 `NAME` / `Args` /
`bind`，builder、`with_state`、`scan` 都有默认实现），再自行注册。

```rust
struct MyDynamic;

impl DynamicTableFunctionAdapter for MyDynamic {
    const NAME: &'static str = "my_dynamic";
    type Args = MyArgs;

    fn bind(args: Self::Args) -> DuckResult<DuckDynamicTable> {
        // 由参数 / 外部元数据算出 schema，再造出行迭代器
        /* … */
    }
}

#[duck_custom_register]
fn my_dynamic_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_table(MyDynamic::table_function_builder()?) }
}
```

:::note[限制]

- `ENUM`、`ARRAY`、`UNION`、`BIT` 无法用 `DuckTypeDesc` 描述（它们的参数不在 `TypeId` 里），
  会直接报错，而不是给出一个残缺的类型。
- 动态列每个单元格多一次枚举分发、每行多一次 `Vec` 分配。只要 schema 在编译期已知，就优先用静态的
  `#[derive(DuckStruct)]` 行结构体 —— 它仍是最快的通路。
- 参数行为与静态通路完全一致（位置 / 命名 / 可空 / 复杂类型）。

:::

## 源码与测试

- [`test/extension/functions/table_function.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/table_function.rs) —— 示例表函数及其行结构体
- [`test/sql/functions/table_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/table_function.test) —— 期望结果
- [`src/functions/table_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/src/functions/table_function_adapter.rs) —— 运行时侧
- [`test/extension/functions/dynamic_table_function.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/dynamic_table_function.rs) —— 动态列示例（宏层与底层）
- [`test/sql/functions/dynamic_table_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/dynamic_table_function.test) —— 对应的期望结果
- [`src/dynamic`](https://github.com/shijianjs/duckfn/blob/main/src/duck_dynamic/mod.rs) —— 动态列的运行时侧

## 接下来

- [类型转换](./casts.md)
- [替换扫描](./replacement-scans.md)
- [类型映射](./types.md)
