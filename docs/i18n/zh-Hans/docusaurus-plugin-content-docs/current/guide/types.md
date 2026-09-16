---
title: 类型映射
sidebar_position: 8
description: DuckDB 类型与 Rust 类型的对应关系，涵盖 LIST、MAP、ARRAY、STRUCT，以及可空性规则与暂不支持的部分。
---

# 类型映射

参数或返回值写成你想要的 Rust 类型，DuckDB 类型随之确定。

可空性也只由这个类型表达：写 `T` 就是 NOT NULL，写 `Option<T>` 就是可空。`Option<T>` 本身就是一个值类型
（映射到和 `T` 完全相同的 DuckDB 类型），所以处处都是同一条规则：元素类型（`Vec<Option<i32>>`、
`[Option<i32>; 3]`、`IndexMap<String, Option<i32>>`）、结构体字段、函数参数与返回值。

多一层 `Option` 是允许的，而且不改变任何行为：`Option<Option<T>>` 与 `Option<T>` 完全等价 ——
映射到同一个 DuckDB 类型，读到 `NULL` 得到的是外层 `None`，写 `None` 与写 `Some(None)` 都是写 `NULL`。
这是有意保留的：封装层常常没法把中间类型剥出来（被包的类型本身可能已经是 `Option`），
否则就得为它单独加一层判断。

## 简单类型

| DuckDB | Rust |
| --- | --- |
| `BOOLEAN` | `bool` |
| `TINYINT` / `SMALLINT` / `INTEGER` / `BIGINT` | `i8` / `i16` / `i32` / `i64` |
| `HUGEINT` | `i128` |
| `UTINYINT` / `USMALLINT` / `UINTEGER` / `UBIGINT` | `u8` / `u16` / `u32` / `u64` |
| `UHUGEINT` | `u128` |
| `FLOAT` / `DOUBLE` | `f32` / `f64` |
| `VARCHAR` | `String` |
| `NULL` | `Option<T>` |

```sql
SELECT dfn_echo_integer(42);                             -- 42
SELECT typeof(dfn_echo_integer(42));                     -- INTEGER
SELECT dfn_echo_uinteger(4294967295::UINTEGER);          -- 4294967295
SELECT dfn_echo_hugeint(9223372036854775808::HUGEINT);   -- 9223372036854775808
SELECT dfn_echo_varchar('你好 🦆');                       -- 你好 🦆
```

## 包装类型

物理表示相同、语义不同的类型都有对应的包装类型：

| DuckDB | Rust | 字段 |
| --- | --- | --- |
| `TIMESTAMP` | `DuckTimestamp` | `micros_since_epoch: i64` |
| `TIMESTAMPTZ` | `DuckTimestampTz` | `millis_since_epoch: i64` |
| `TIMESTAMP_S` / `TIMESTAMP_MS` / `TIMESTAMP_NS` | `DuckTimestampS` / `DuckTimestampMs` / `DuckTimestampNs` | `seconds_since_epoch` / `millis_since_epoch` / `nanos_since_epoch` |
| `TIME` | `DuckTime` | `micros_since_midnight: i64` |
| `TIME_NS` | `DuckTimeNs` | `nanos_since_midnight: i64` |
| `TIMETZ` | `DuckTimeTz` | `bits: u64` |
| `DATE` | `DuckDate` | `days_since_epoch: i32` |
| `DECIMAL(W, S)` | `DuckDecimal<W, S>` | `unscaled: i128` |
| `BLOB` | `DuckBlob` | `value: Vec<u8>` |
| `UUID` | `DuckUuid` | `value: u128` |
| `INTERVAL` | `DuckInterval` | 来自 `quack-rs` |

`DuckDecimal` 把精度与标度带在类型上，因此能映射回正确的 `DECIMAL`：

```rust
#[duck_scalar_function]
fn dfn_echo_decimal(i: DuckDecimal<18, 3>) -> DuckDecimal<18, 3> {
    i
}
```

```sql
SET TimeZone = 'UTC';  -- 让 TIMESTAMPTZ 的输出稳定
SELECT dfn_echo_date(DATE '2024-01-02');                -- 2024-01-02
SELECT typeof(dfn_echo_decimal(1.234::DECIMAL(18,3)));  -- DECIMAL(18,3)
SELECT CAST(dfn_echo_time_ns(TIME_NS '03:04:05.123456789') AS VARCHAR);  -- 03:04:05.123456789
SELECT CAST(dfn_echo_uuid('00000000-0000-0000-0000-000000000001'::UUID) AS VARCHAR);
```

`TIME_NS` 是 DuckDB 1.5 新增的类型，因此需要开启
[`duckdb-1-5` feature](../getting-started/installation.md#cargo-feature)。

## 列表

`LIST(T)` 就是 `Vec<T>`。元素是否可空由 Rust 类型决定：

| Rust | DuckDB | 出现 `NULL` 元素时 |
| --- | --- | --- |
| `Vec<T>` | `LIST(T) NOT NULL` | 整行变成 `NULL`。 |
| `Vec<Option<T>>` | `LIST(T)` | 该元素保持 `NULL`。 |

别名 `DuckList<T>` 就是 `Vec<T>` —— 只是让 LIST / ARRAY / MAP 共用 `Duck*` 命名，并不存在需要另找的
专用包装类型。可空性由元素类型自己承载（元素可空就写 `Vec<Option<T>>`），所以 `Vec<T>` 只需一份实现：
元素类型装得下 NULL 时该元素变成 `None`，装不下时整行变成 `NULL`。

```rust
#[duck_scalar_function]
fn dfn_echo_list_integer_n(i: Vec<Option<i32>>) -> Vec<Option<i32>> {
    i
}
```

```sql
SELECT CAST(dfn_echo_list_integer([1, 2, 3]) AS VARCHAR);      -- [1, 2, 3]
SELECT dfn_echo_list_integer([1, NULL, 3]);                    -- NULL
SELECT CAST(dfn_echo_list_integer_n([1, NULL, 3]) AS VARCHAR); -- [1, NULL, 3]
```

列表可以任意嵌套：`Vec<Vec<i32>>`、`Vec<Option<Vec<Option<i32>>>>`。

## 映射

`MAP(K, V)` 对应 [`IndexMap`](https://docs.rs/indexmap)，会保留插入顺序：

| Rust | 值可以为 `NULL` |
| --- | --- |
| `IndexMap<K, V>` | 不可以 —— 出现 `NULL` 值会报错。 |
| `IndexMap<K, Option<V>>` | 可以。 |

别名 `DuckMap<K, V>` 就是 `IndexMap<K, V>`，同样共用 `Duck*` 命名 —— NULL 由值的类型承载，
值可空就写 `IndexMap<K, Option<V>>`。

键永远不可以为 `NULL`。`MAP` 参数也是把键值数据传进表函数的方式：

```sql
SELECT CAST(dfn_echo_map_varchar_integer(MAP {'a': 1, 'b': 2}) AS VARCHAR);  -- {a=1, b=2}
SELECT * FROM dfn_table_from_map(MAP {'a': 1, 'b': 2});                      -- a 1 / b 2
```

## 数组

`ARRAY(T, N)` 是定长数组，长度写在 Rust 类型里：

| Rust | DuckDB |
| --- | --- |
| `DuckArray<T, N>`（即 `[T; N]`） | `T[N]`，元素不可空 |
| `[Option<T>; N]` | `T[N]`，元素可空 |

```rust
#[duck_scalar_function]
fn dfn_echo_array_integer(i: DuckArray<i32, 3>) -> DuckArray<i32, 3> {
    i
}
```

```sql
SELECT typeof(dfn_echo_array_integer([1, 2, 3]));  -- INTEGER[3]
SELECT dfn_echo_array_integer([1, 2, 3]);          -- [1, 2, 3]
SELECT dfn_echo_array_integer([1, 2]);             -- 报错：No function matches
```

:::warning[数组不能作为 bind 参数]
DuckDB 无法把 `Value` 绑定到 `ARRAY` 参数上，因此数组类型不能出现在表函数签名里。数组作为标量函数参数、
作为列表元素、作为结构体字段都没有问题。
:::

## 结构体

`STRUCT` 就是用 `#[derive(DuckStruct)]` 标注的 Rust 结构体，字段名成为列名，字段类型成为列类型：

```rust
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct DuckStructInner {
    key: String,
    value: i32,
}

#[derive(Debug, Clone, Default, DuckStruct)]
pub struct DuckStructNested {
    id: i32,
    inner: DuckStructInner,
    maybe: Option<DuckStructInner>,
}
```

```sql
SELECT (dfn_echo_struct_simple({'id': 1, 'name': 'a'})).id;                                 -- 1
SELECT (dfn_echo_struct_nested({'id': 1, 'inner': {'key': 'k', 'value': 2}})).inner.value;  -- 2
```

结构体可以嵌套，也可以放进其它容器里 —— `Vec<DuckStructSimple>`、`IndexMap<String, DuckStructSimple>`、
`DuckArray<DuckStructSimple, 2>` 以及它们的 `Option` 版本都支持。

现在只剩一个约束：结构体必须是**具名字段** —— 字段类型按你写的原样使用。`Option<T>` 表示该字段可空，
嵌套也没问题：`Option<Vec<Option<i32>>>`、`Option<Option<i32>>`、`Vec<Option<Option<i32>>>` 都能编译。
多出来的 `Option` 层是「惰性」的：映射到同一个 DuckDB 类型，读到 `NULL` 得到的是外层 `None`，
而写 `None` 与写嵌套的 `Some(None)` 都是写 `NULL`。

结构体也能在 catalog 里**取个名字**：`#[duck(sql_name = "ticket", create_type = true)]` 会让扩展在加载时
执行 `CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT(...)`，之后 SQL 里就能把 `ticket` 当类型用
（列类型或 cast 目标）：

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = true)]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,      // #[derive(DuckEnum)] 生成的枚举
    pub labels: Vec<String>,
}
// CREATE TYPE IF NOT EXISTS "ticket" AS
//   STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'), "labels" VARCHAR[]);
```

```sql
CREATE TABLE tickets (v ticket);
INSERT INTO tickets VALUES ({'id': 1, 'priority': 'low', 'labels': ['a']});
SELECT dfn_echo_struct_ticket(v) FROM tickets;   -- 函数用的是等价的结构化类型
```

字段类型不是宏写死的：每个字段的 `LogicalType` 会被递归渲染（走 DuckDB 自己的类型 introspection），
所以枚举、嵌套结构体、`LIST` / `ARRAY` / `MAP`、`DECIMAL` 以及手写的自定义类型都会自动带上 ——
`Option<T>` 也不会改变类型文本，因为可空性不是 DuckDB 类型的一部分。语句是幂等的，`LOAD` 两次没问题，
已存在的同名类型也不会被覆盖。

## 容器的组合

各类容器可以自由组合：结构体字段可以是列表或映射，列表元素可以是结构体，映射的值可以是另一个映射：

```rust
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct DuckStructWithList {
    id: i32,
    data: Vec<i32>,
    tags: Vec<String>,
}
```

`Vec<Option<Vec<Option<T>>>>`、`IndexMap<String, Option<IndexMap<String, Option<i32>>>>`、
`DuckArray<Option<DuckArray<Option<i32>, 2>>, 2>` 都有对应的 DuckDB 类型。

## 懒加载参数

`DuckLazy<T>` 在 SQL 侧看起来就是 `T`（逻辑类型相同、`NULL` 规则相同），但解析被推迟了：读一行只记录
「值在哪」（O(1)），真正的解析发生在 `get()` / `try_get()` 里。它面向的是「多行不变、却比旁边的值复杂
得多」的参数，典型就是聚合函数的配置项：

```rust
#[duck_aggregate_function]
fn my_agg(cfg: DuckLazy<Config>, v: i64, state: &mut MyState) -> DuckResult<()> {
    // 第一行解析一次；后续每一行复用解析结果。
    if state.cfg.is_none() {
        state.cfg = Some(cfg.get());
    }
    // ... 使用 state.cfg
}
```

几条需要知道的规则：

- 凭证**只在产生它的那次回调内有效**。把它存进聚合状态、或在之后的 chunk / 其它线程里消费都属于误用 ——
  运行时守卫会把它变成 `DuckLazy<T> is stale: ...` 的查询报错，而不是未定义行为。请缓存**解析后的值**，
  不要缓存凭证。
- `DuckLazy<T>` **只读**：作为返回类型或输出字段使用会直接报错。
- bind/`Value` 路径（表函数参数）显式拒绝：那类值只在 bind 回调内有效。
- 单元格为 `NULL` 时和其它类型一样读到 `None` —— 参数可能为 `NULL` 就写 `Option<DuckLazy<T>>`。

## 枚举

DuckDB 的 `ENUM` 不在 duckfn 内建映射的值类型里，但它的映射规则完全是机械的，所以
`#[derive(DuckEnum)]` 可以直接从普通 Rust 枚举生成：

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "lowercase", sql_name = "priority", create_type = true)]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}
```

- **字典就是声明顺序**，标签由 `rename_all` / 变体上的 `#[duck(rename = "...")]` 决定（上面是 `['low', 'medium', 'high']`）；
- 逻辑类型就是那个 `ENUM(...)`，因此枚举可以直接作为参数、返回值、`STRUCT` 字段或容器元素，
  `Option<Priority>` 表示可空；
- `create_type = true` 还会在扩展加载时执行 `CREATE TYPE IF NOT EXISTS "priority" AS ENUM (...)` ——
  幂等，且不会覆盖已存在的同名类型 —— 之后 SQL 里可以直接写 `'high'::priority`，也能把列声明成 `priority`；
- 与其它参数一样，**非可空**的枚举参数需要 `Default`（宏生成的参数结构体会 `derive(Default)`），
  所以例子里有 `#[derive(Default)]` + `#[default]`。

注意 DuckDB 只对**常量**字符串做隐式 `VARCHAR → ENUM` 转换；列或表达式需要显式写 `'low'::priority`。
`#[duck(...)]` 的完整参数见[属性参考](./attributes.md)。

## 已知缺口

| 缺口 | 说明 |
| --- | --- |
| 需要 feature 的类型 | `TIME_NS` 已由 `DuckTimeNs` 映射，但要开启 [`duckdb-1-5` feature](../getting-started/installation.md#cargo-feature)。 |
| 未映射，但可以自己实现 | `UNION`、`BIT`、`VARINT`、`GEOMETRY`、`VARIANT`：quack-rs 没有它们的读写方法，但你可以[自己实现 `DuckValueType`](./custom-types.md)，通过裸向量句柄直接调用 DuckDB 的 C API。`ENUM` 则由 [`#[derive(DuckEnum)]`](#枚举) 生成。 |
| 不可存储类型 | `ANY`、`SQLNULL` 以及整数/字符串字面量类型只存在于 DuckDB 自身的函数签名与字面量中，不能作为扩展的参数或返回类型。 |
| 没有专用的 `DuckList` / `DuckMap` 包装类型 | `DuckList<T>` / `DuckMap<K, V>` 只是 `Vec<T>` / `IndexMap<K, V>` 的别名，真正的类型是标准库 / `indexmap` 的那个。 |
| `ARRAY` 作为 bind 参数 | 不支持（见上文）。 |
| `MAP` 的键 | 永远不可为空。 |
| `DECIMAL` | DuckDB 要求 `WIDTH < 39`。 |

## 源码与测试

- [`src/extension/types/`](https://github.com/shijianjs/duckfn/tree/main/src/extension/types) —— 每种类型的 echo 函数
- [`test/sql/types/`](https://github.com/shijianjs/duckfn/tree/main/test/sql/types) —— 期望结果
- [`duckfn/src/value_types/`](https://github.com/shijianjs/duckfn/tree/main/duckfn/src/value_types) —— 类型实现本身

## 接下来

- [自定义类型](./custom-types.md) —— 自己实现类型映射。
- [错误与 panic](./errors-and-panics.md)
- [表函数](./table-functions.md) —— 这些类型作为输出列的用法。
