---
title: 自定义类型
sidebar_position: 11
description: 为自己的类型实现 DuckValueType，以及如何触达 duckfn 尚未映射的逻辑类型。
---

# 自定义类型

`DuckValueType` 是公开且未封闭的 trait，因此在 `duckfn` 之外定义的类型也能映射到 DuckDB 类型。
示例扩展就是这么做的，实现在 `duckfn-quack/src/extension/types/custom_type_echo.rs`，本页逐段说明。

## 什么时候需要自己实现

- **同一个物理类型，两种含义。** `DOUBLE` 在这里是温度、在那里是距离，用 newtype 把两者区分开 ——
  duckfn 自己也是出于同样的理由，把 `TIMESTAMP`、`TIMESTAMP_S`、`TIME` 等包成 newtype，而不是一律映射成 `i64`。
- **duckfn 还没映射的逻辑类型。** `ENUM`、`BIT`、`VARINT`、`GEOMETRY`、`VARIANT` 以及 DuckDB 1.5 新增的类型，
  要么需要开启某个 feature，要么需要直接调用 C API；见[类型映射](./types.md#已知缺口)。

## 最小实现

```rust
use duckfn::DuckValueType;
use quack_rs::prelude::{TypeId, Value, VectorReader, VectorWriter};

/// 摄氏温度：物理表示就是 `DOUBLE`，语义由这个 newtype 承载 —— DuckDB 没有温度类型。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Celsius(pub f64);

impl DuckValueType for Celsius {
    fn type_id() -> TypeId {
        TypeId::Double
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self(unsafe { reader.read_f64(row) })
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f64(idx, v.0) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self(value.as_f64())
    }
}
```

| 方法 | 何时需要 | 作用 |
| --- | --- | --- |
| `type_id()` | 总是 | 映射到的 DuckDB 逻辑类型。 |
| `read_valid_by_vector_reader()` | 总是 | 从输入向量读一个有效值。 |
| `write_valid_to_vector_writer()` | 总是 | 把一个有效值写进输出向量。 |
| `read_by_duck_value_valid_simple()` | 作为参数时 | 从 `duckdb_value` 读取 —— 表函数参数、结构体字段与类型转换走的就是这条路径。 |
| `logical_type()` | 参数化类型 | 当类型无法只用 `type_id()` 描述时重写 —— `DECIMAL(18,3)`、`LIST(T)`、`ARRAY(T,N)`、`STRUCT(...)`。 |
| `from_null()` | 可空类型 | 该类型能否用一个「值」表示 `NULL`：`Option<T>` 返回 `Some(None)`；默认 `None` 表示「装不下 NULL，遇到 NULL 槽位就让整个值作废」。 |
| `write_null()` | 容器类型 | 需要把 `NULL` 一并传播到子向量时重写。 |
| `create_reader_from_vector()` / `create_writer_batch()` / `write_finish()` | 容器类型 | 挂接子读写器、收尾子向量长度的地方。 |

动手前有两条约束值得留意：

- trait 本身要求 `Clone + Debug + Send + Sync + 'static`。
- 作为**函数参数**的类型还需要 `Default`，因为宏生成的参数结构体会 derive 它。

`read()` 与 `write()` 是带 NULL 判定的入口，刻意不打算被重写；`read_slot()` 建在 `read()` 之上、
额外接上 `from_null()` 回退，容器与结构体字段读单个元素/字段走的就是它。

## 用法

作为标量函数的入参与返回值，不需要额外做什么：

```rust
#[duck_scalar_function]
fn dfn_echo_celsius(t: Celsius) -> Celsius {
    t
}
```

```sql
SELECT dfn_echo_celsius(21.5::DOUBLE);         -- 21.5
SELECT typeof(dfn_echo_celsius(21.5::DOUBLE)); -- DOUBLE
```

它也能直接作为 `STRUCT` 字段 —— `#[derive(DuckStruct)]` 会为每个字段生成编译期的
`assert_impl_duck_value_type::<T>()`，因此字段类型没实现 trait 时是编译错误，而不是运行时的意外：

```rust
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct TemperatureReading {
    pub place: String,
    pub celsius: Celsius,
}
```

```sql
SELECT (dfn_echo_temperature_reading({'place': 'oslo', 'celsius': -3.5::DOUBLE})).place;  -- oslo
SELECT typeof((dfn_echo_temperature_reading({'place': 'oslo', 'celsius': -3.5::DOUBLE})).celsius);
-- DOUBLE
```

作为表函数的参数与输出列同样可以：

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoCelsiusRow {
    pub v: Option<Celsius>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_celsius(
    v: Option<Celsius>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoCelsiusRow> {
    echo_rows(v, count, |v| TableEchoCelsiusRow { v })
}
```

```sql
SELECT CAST(v AS VARCHAR) FROM dfn_table_echo_celsius(1.5::DOUBLE, count => 3);
-- 1.5
-- NULL
-- 1.5
```

## 更进一步

**quack-rs 没有映射的逻辑类型。** `DuckValueReader` 与 `DuckValueWriter` 在高层的 `vector_reader` /
`vector_writer` 之外，都暴露了裸的 `c_duckdb_vector`，因此必要时可以下到 DuckDB 的 C API。`ENUM`
（读下标、再从枚举字典取出标签）、`BIT`、`VARINT` 这些没有 quack-rs 访问器的类型，走的就是这条路。
`duckfn-quack/src/extension/types/custom_type_echo.rs` 里的 `Color` 就是这个例子的完整实现：字典用
`LogicalType::enum_type(&[...])` 声明，读写覆盖带裸向量的 `read_valid` / `write_valid` 来搬运下标，
bind 阶段的标签用 `Value::as_str()` 从 `duckdb_value` 取。
具体到 `ENUM`，通常不必手写：`#[derive(DuckEnum)]` 生成的就是这份实现，配上 `create_type = true`
还会在加载期执行 `CREATE TYPE ... AS ENUM (...)` —— 手写版本的意义是把机制讲清楚，而不是日常用法。

**容器类型。** 容器需要子读写器，并且要把 NULL 传播进子向量。
`src/value_types/duck_list.rs`、`duck_map.rs`、`duck_array.rs`、`duck_struct.rs` 就是参考实现。

**延迟读取。** `duck_lazy.rs` 是另一个极端：`read_valid` 只记录位置和一个存活凭证，真正的解析推迟到
`get()`。自己的类型想推迟工作时就照这个模式来 —— 包括那个凭证，正是它把「源 chunk 已经死了还在消费」
从未定义行为变成了查询报错。

**DuckDB 1.5 新增的逻辑类型。** 其中一些只需要开启一个 feature 就能用：`TIME_NS` 已经内置在 duckfn 里，
由 `duckdb-1-5` 控制，该 feature 转发到 quack-rs 的同名 feature —— 见[安装](../getting-started/installation.md#cargo-feature)。

## 源码与测试

- [`duckfn-quack/src/extension/types/custom_type_echo.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/src/extension/types/custom_type_echo.rs) —— `Celsius`，在 duckfn 之外实现
- [`duckfn-quack/test/sql/types/custom_type_echo.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/types/custom_type_echo.test) —— 期望结果
- [`src/value_types/duck_value_type.rs`](https://github.com/shijianjs/duckfn/blob/main/src/value_types/duck_value_type.rs) —— trait 本身

## 接下来

- [类型映射](./types.md) —— 哪些已经内置，哪些还没有。
- [架构](../internals/architecture.md#6-值类型) —— 读写器之间如何协作。
