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

别名 `DuckList<T>` / `DuckOptionList<T>` 就是这两种类型（`Vec<T>` / `Vec<Option<T>>`）—— 只是为了和
[`DuckArray`](#数组) 的命名对齐，并不存在需要另找的专用包装类型。可空性由元素类型自己承载，
所以 `Vec<T>` 只需一份实现：元素类型装得下 NULL 时该元素变成 `None`，装不下时整行变成 `NULL`。

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

`DuckMap<K, V>` / `DuckOptionMap<K, V>` 是 `IndexMap<K, V>` / `IndexMap<K, Option<V>>` 的别名，同样是为了和
[`DuckArray`](#数组) 的命名对齐 —— NULL 由值的类型承载。

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
| `DuckOptionArray<T, N>`（即 `[Option<T>; N]`） | `T[N]`，元素可空 |

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

derive 有两个约束：结构体必须是**具名字段**，可选字段只能是单层 `Option<T>`（字段类型写成
`Option<Vec<Option<i32>>>` 没问题，但不支持 `Option<Option<T>>`）。

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
`DuckOptionArray<DuckOptionArray<i32, 2>, 2>` 都有对应的 DuckDB 类型。

## 已知缺口

| 缺口 | 说明 |
| --- | --- |
| 需要 feature 的类型 | `TIME_NS` 已由 `DuckTimeNs` 映射，但要开启 [`duckdb-1-5` feature](../getting-started/installation.md#cargo-feature)。 |
| 未映射，但可以自己实现 | `ENUM`、`UNION`、`BIT`、`VARINT`、`GEOMETRY`、`VARIANT`：quack-rs 没有它们的读写方法，但你可以[自己实现 `DuckValueType`](./custom-types.md)，通过裸向量句柄直接调用 DuckDB 的 C API。 |
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
