---
title: 类型映射
sidebar_position: 9
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
| `TIMESTAMPTZ` | `DuckTimestampTz` | `micros_since_epoch: i64` |
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

### 与其它 crate 互转

包装类型里往往只剩「原始标量」—— `TIMESTAMP` 是自纪元起的微秒数、`DuckUuid` 是 128 位、
`DuckDecimal` 是「未缩放整数 + 标度」。把它们变成代码里真正要用的类型，就是下面三个可选 feature 干的事；
每个都只是给**已有类型**加固有方法，开了也不会改变你现在的 API：

| feature | crate | 转换 |
| --- | --- | --- |
| `chrono` | [`chrono`](https://crates.io/crates/chrono) | `DuckDate` ↔ `NaiveDate`；`DuckTimestamp` / `DuckTimestampS` / `DuckTimestampMs` / `DuckTimestampNs` ↔ `NaiveDateTime`；`DuckTimestampTz` ↔ `DateTime<Utc>`；`DuckTime` / `DuckTimeNs` ↔ `NaiveTime` |
| `uuid` | [`uuid`](https://crates.io/crates/uuid) | `DuckUuid` ↔ `Uuid` |
| `rust_decimal` | [`rust_decimal`](https://crates.io/crates/rust_decimal) | `DuckDecimal<W, S>` ↔ `Decimal` |

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["chrono", "uuid", "rust_decimal"] }
# duckfn 不 re-export 这三个 crate，用到哪些类型就自己加依赖。chrono 核心类型加 std 就够；
# 只有自己调 Utc::now() 才需要 clock（取系统时间）。
chrono = { version = "0.4", default-features = false, features = ["std"] }
uuid = "1"
rust_decimal = "1"
```

#### `chrono`

把原始刻度变成可读、可算的日期时间所需的纪元数学，默认都得调用方自己写；`chrono` feature 把它接过来了：

```rust
use chrono::{Days, NaiveDate};

#[duck_scalar_function]
fn dfn_date_add(d: DuckDate, days: i64) -> DuckOptionResult<DuckDate> {
    let date = d.to_naive_date()?;                                   // DATE -> NaiveDate
    let shifted = date.checked_add_days(Days::new(days.unsigned_abs()))
        .ok_or_else(|| duck_error("dfn_date_add: out of chrono's range"))?;
    Ok(Some(DuckDate::from_naive_date(shifted)?))                    // NaiveDate -> DATE
}
```

两条约定：

- **不 panic**：`DATE` 是 `i32` 天数、`TIMESTAMP` 是 `i64` 微秒数，值域都远大于 chrono 能表示的范围，
  而且 DuckDB 还有 `infinity` / `-infinity` 两个哨兵（`DATE` 存 `±i32::MAX`，`TIMESTAMP` 系列存
  `±i64::MAX`）。chrono 装不下的值一律返回 `DuckResult` 错误，不饱和、也不绕回成另一个时间点。
- **单位换算由方法承担**：不用再从字段名猜精度 —— `TIMESTAMP_S` / `TIMESTAMP_MS` / `TIMESTAMP_NS`
  分别是秒 / 毫秒 / 纳秒，而 `TIMESTAMP` 与 `TIMESTAMPTZ` 都是微秒（两者共用同一份 `i64` 存储）。
  反向写进更粗的单位时（`from_naive_datetime` 写进 `TIMESTAMP_S`，或写进 `TIME`）按 DuckDB 自己的做法
  **向零截断**。

四对里最常用的就是 `DuckDate` ↔ `NaiveDate`：SQL 给你的是一个日期，而计算要的是一份日历。

#### `uuid`

`DuckUuid` 的 `value` 是 DuckDB **渲染出来的**那 128 位：`UUID` 列物理上是 `HUGEINT`，DuckDB 又给最高位
做了翻转，好让有符号整数的排序与 UUID 文本排序一致；quack-rs 的 `read_uuid` / `write_uuid` /
`Value::as_uuid` 已经把这层翻转撤销了。而 `Uuid::as_u128` / `Uuid::from_u128` 用的正是同一套大端规范字节序，
所以这一对是**无损**的，两个方向都不返回 `DuckResult`：

```rust
#[duck_scalar_function]
fn dfn_uuid_to_text(u: DuckUuid) -> String {
    u.to_uuid().to_string()          // 与 CAST(u AS VARCHAR) 逐字相同
}

#[duck_scalar_function]
fn dfn_uuid_parse(text: String) -> DuckOptionResult<DuckUuid> {
    let uuid = Uuid::parse_str(&text)?;                 // 会失败的是解析这一半
    Ok(Some(DuckUuid::from_uuid(uuid)))
}
```

#### `rust_decimal`

这一对两边的能力**并不对等**：DuckDB 的 `DECIMAL(W, S)` 是「`i128` 未缩放整数 + 标度」，`W` 最大 38；
而 rust_decimal 的尾数是 96 位（约 28~29 位有效数字），标度上限 28。因此两个方向都返回 `DuckResult`：

- `to_decimal` 把未缩放整数与标度原样交给 rust_decimal，由它报错说明放不下 —— `DECIMAL(38, 0)` 里的
  大值，或任何大于 28 的标度；
- `from_decimal` 先把标度调到 `S`（变细则补零；变粗必须**整除**，否则会丢位），再检查结果是否在 `W` 位以内。

```rust
#[duck_scalar_function]
fn dfn_decimal_double(d: DuckDecimal<18, 3>) -> DuckOptionResult<DuckDecimal<18, 3>> {
    let doubled = d.to_decimal()? * Decimal::TWO;      // DECIMAL(18, 3) -> Decimal
    Ok(Some(DuckDecimal::from_decimal(doubled)?))      // Decimal -> DECIMAL(18, 3)
}
```

精度超出 rust_decimal 能承受的范围时（比如 `DECIMAL(38, 0)` 的极端值）没有桥可走：直接读 `unscaled`
字段（`i128`）自己算。

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
SELECT (dfn_echo_struct_nested({'id': 1, 'inner': {'key': 'k', 'value': 2}, 'maybe': NULL})).inner.value;  -- 2
```

`STRUCT` **字面量**按它的匿名类型精确匹配，所以字段列表必须完全对上 —— 字段名、字段类型、字段个数都要
一致。DuckDB 不会为了补齐、删掉或改名而插入隐式 cast，而它给的报错并不会提这一点：对着上面这个结构体写
`{'id': 1}` 会报 `No function matches the given name and argument types
'dfn_echo_struct_nested(STRUCT(id INTEGER))'. You might need to add explicit type casts.`。要么把字段写全
（可空字段也写上 `NULL`），要么显式 cast —— `…::STRUCT(id INTEGER, "inner" STRUCT(…), maybe STRUCT(…))`
（字段名和关键字冲突时要加引号，这里的 `inner` 就是），或者 cast 到 `create_type` 建好的命名类型
（`…::ticket`，见下文）。而**列**只要有正确的类型就不需要这些 —— 它本来就是那个类型。

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

把 `create_type = true` 换成 `create_type = "print"`，渲染的是**同一条**语句，但不建类型：
DDL 先收进队列，等全部注册跑完再一次性打印（带 `-- [duckfn]` 提示框、写明没有执行，可直接复制去跑）。见
[属性参考 → 在 catalog 里建命名类型](./attributes.md#在-catalog-里建命名类型)。

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
#[derive(Default, Debug, Clone)]
struct MyState {
    cfg: DuckLazySlot<Config>,
    sum: f64,
}

#[duck_aggregate_function]
fn my_agg(cfg: DuckLazy<Config>, v: i64, state: &mut MyState) -> DuckResult<()> {
    // 第一行解析一次；后续每一行只做一次引用计数递增。
    let cfg = state.cfg.resolve(&cfg)?;
    state.sum += cfg.weight(v);
    Ok(())
}
```

几条需要知道的规则：

- 凭证**只在产生它的那次回调内有效**。把它存进聚合状态、或在之后的 chunk / 其它线程里消费都属于误用 ——
  运行时守卫会把它变成 `DuckLazy<T> is stale: ...` 的查询报错，而不是未定义行为。请缓存**解析后的值**，
  不要缓存凭证。
- `DuckLazy<T>` **只读**：作为返回类型或输出字段使用会直接报错。
- bind/`Value` 路径（表函数参数）显式拒绝：那类值只在 bind 回调内有效。
- 单元格为 `NULL` 时和其它类型一样读到 `None` —— 参数可能为 `NULL` 就写 `Option<DuckLazy<T>>`。
- 解析出来的值交给 `DuckLazySlot<T>` 保管：`resolve` 只解析一次（可空参数用 `resolve_optional`），
  DuckDB 并行合并状态时用 `combine` 把结果搬过去（不重新解析），`result()` 里用 `get` 取回。

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
  `create_type = "print"` 渲染的是同一条语句，但只打印到 stderr，catalog 不受影响；
- 与其它参数一样，**非可空**的枚举参数需要 `Default`（宏生成的参数结构体会 `derive(Default)`），
  所以例子里有 `#[derive(Default)]` + `#[default]`。

注意 DuckDB 只对**常量**字符串做隐式 `VARCHAR → ENUM` 转换；列或表达式需要显式写 `'low'::priority`。
`#[duck(...)]` 的完整参数见[属性参考](./attributes.md)。

## 两套类型体系：静态与动态

上面讲的全是**静态**体系：通过实现 `DuckValueType`，一个 Rust 类型在编译期就与一个 DuckDB 类型
绑定（`#[derive(DuckStruct)]` / `#[derive(DuckEnum)]` 会替你生成实现）。标量函数、聚合函数、类型
转换、`COPY`、SQL 宏和普通表函数用的都是它 —— 它也是默认选择：一个 Rust 类型 ↔ 一个逻辑类型，没有
运行时分发。

**动态**体系的存在，是因为编译期映射在结构上回答不了「这条查询的列是什么」——当列要等 bind 阶段
读了文件头、字典表或远端 schema 才知道时。它不是替代品，也没有重复上面的映射，而是**建在它之上**：

| | 静态（`DuckValueType`） | 动态（`DuckTypeDesc` / `DuckDynamicValue`） |
| --- | --- | --- |
| 类型何时确定 | 编译期 | bind 阶段（运行时） |
| 一个 Rust 类型 ↔ | 一个逻辑类型 | 任意逻辑类型，按列描述 |
| 使用者 | 标量 / 聚合 / cast / `COPY` / SQL 宏 / 普通表函数 | `dynamic_columns = true` 的表函数，或手写 `DynamicTableFunctionAdapter` |
| 列名 | 结构体的字段名 | `DuckResultSchema` 里写什么就是什么 |
| 开销 | 无 | 每格一次枚举分发、每行一次 `Vec` |

动态侧真正复用的东西（而不是重写一遍）：

- 标量写出直接调用各基础类型的 `DuckValueType::write_valid_to_vector_writer`，物理写入与静态路径
  完全一致；
- `LIST` / `MAP` / `STRUCT` 的布局约定（子向量、entry、NULL 行、收尾）集中在一个共享模块里，两条
  通路共用；
- 参数解析仍然是 `#[derive(DuckStruct)]` —— 动态表函数的 `Args` 就是一个 `DuckBindArgs`；
- `DuckTypeDesc` 的标量分支，就是本页开头那张表里的 `TypeId`。

有两件事动态侧**故意不做**，请继续用静态体系：`ENUM` 与 `ARRAY`（以及 `UNION` / `BIT`）无法用
`DuckTypeDesc` 表达，因为它们的参数不在 `TypeId` 里；而编译期已知的 schema 永远更适合
`#[derive(DuckStruct)]` 行结构体 —— 动态路径每格多一次枚举分发、每行多一次 `Vec`。

### 读取 `Value` 的一条规则

bind 参数、以及 `LIST` / `MAP` / `STRUCT` 值的子元素，都是以 `Value` 形式拿到的，有一条规则必须
知道：

- 用 `duckfn::duck_value_is_null(&value)` 判断，**不要**用 `value.is_null()`。quack-rs 的
  `is_null()` 只看句柄指针是否为空，于是 SQL 里显式写的 `arg = NULL` 会漏过去，紧跟着的类型读取
  会让进程以 `fatal runtime error: Rust cannot catch foreign exceptions` 中止；只有*省略*该参数
  才会拿到空指针。
- `NULL` 读成 `None`；嵌套读取用 `Some(None)` 表示「这个元素是 NULL」。

## 已知缺口

| 缺口 | 说明 |
| --- | --- |
| 需要 feature 的类型 | `TIME_NS` 已由 `DuckTimeNs` 映射，但要开启 [`duckdb-1-5` feature](../getting-started/installation.md#cargo-feature)；[与其它 crate 互转](#与其它-crate-互转) 需要 `chrono` / `uuid` / `rust_decimal`。 |
| 未映射，但可以自己实现 | `UNION`、`BIT`、`VARINT`、`GEOMETRY`、`VARIANT`：quack-rs 没有它们的读写方法，但你可以[自己实现 `DuckValueType`](./custom-types.md)，通过裸向量句柄直接调用 DuckDB 的 C API。`ENUM` 则由 [`#[derive(DuckEnum)]`](#枚举) 生成。 |
| 不可存储类型 | `ANY`、`SQLNULL` 以及整数/字符串字面量类型只存在于 DuckDB 自身的函数签名与字面量中，不能作为扩展的参数或返回类型。 |
| 没有专用的 `DuckList` / `DuckMap` 包装类型 | `DuckList<T>` / `DuckMap<K, V>` 只是 `Vec<T>` / `IndexMap<K, V>` 的别名，真正的类型是标准库 / `indexmap` 的那个。 |
| `ARRAY` 作为 bind 参数 | 不支持（见上文）。 |
| `MAP` 的键 | 永远不可为空。 |
| `DECIMAL` | DuckDB 要求 `WIDTH < 39`。 |

## 源码与测试

- [`test/extension/types/`](https://github.com/shijianjs/duckfn/tree/main/test/extension/types) —— 每种类型的 echo 函数
- [`test/sql/types/`](https://github.com/shijianjs/duckfn/tree/main/test/sql/types) —— 期望结果
- [`src/value_types/`](https://github.com/shijianjs/duckfn/tree/main/src/value_types) —— 类型实现本身
- [`test/extension/functions/chrono_bridge.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/chrono_bridge.rs) —— `chrono` 转换的实际用法
- [`test/sql/functions/chrono_bridge.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/chrono_bridge.test) —— 期望结果，含 `infinity` 报错
- [`test/extension/functions/uuid_bridge.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/uuid_bridge.rs) · [`test/sql/functions/uuid_bridge.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/uuid_bridge.test) —— `uuid` 一对，与 DuckDB 自己的渲染对照
- [`test/extension/functions/rust_decimal_bridge.rs`](https://github.com/shijianjs/duckfn/blob/main/test/extension/functions/rust_decimal_bridge.rs) · [`test/sql/functions/rust_decimal_bridge.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/rust_decimal_bridge.test) —— `rust_decimal` 一对，含越界的情形

## 接下来

- [自定义类型](./custom-types.md) —— 自己实现类型映射。
- [错误与 panic](./errors-and-panics.md)
- [表函数](./table-functions.md) —— 这些类型作为输出列的用法。
