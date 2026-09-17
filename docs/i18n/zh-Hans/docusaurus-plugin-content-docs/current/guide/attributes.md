---
title: 属性参考
sidebar_position: 1
description: duckfn 的全部属性、它们共用的参数、各自生成的 item，以及自动注册的机制。
---

# 属性参考

## 属性一览

| 属性 | 注册的对象 | 允许的返回形态 |
| --- | --- | --- |
| `#[duck_scalar_function]` | 标量函数 | `T`、`Option<T>`、`DuckOptionResult<T>` |
| `#[duck_aggregate_function]` | 聚合函数 | 行处理函数返回 `()` 或 `DuckResult<()>`；输出由状态给出 |
| `#[duck_table_function]` | 表函数 | `impl Iterator<Item = Row>`、`DuckResult<impl Iterator<Item = Row>>`、`DuckFullIteratorResult<Row>` |
| `#[duck_cast_function]` | 类型转换 | `T`、`Option<T>`、`DuckOptionResult<T>` |
| `#[duck_replacement_scan]` | 替换扫描 | `Option<String>`、`Option<&'static str>`、`DuckOptionResult<String>`、`DuckOptionResult<&'static str>` |
| `#[duck_sql_macro]` | SQL 宏 | `SqlMacro`、`DuckResult<SqlMacro>`、`String`、`&'static str`，或它们的 `DuckResult` |
| `#[duck_custom_register]` | 函数自己注册的内容 | `fn(&Connection) -> DuckResult<()>` |
| `#[derive(DuckStruct)]` | — | 把结构体映射为 DuckDB 的 `STRUCT` |
| `#[derive(DuckEnum)]` | — | 把只有单元变体的枚举映射为 DuckDB 的 `ENUM`（可选在加载期建类型） |
| `duckfn_entrypoint!("name")` | 扩展入口点 | — |
| `duck_sql_macro_files!("a.sql", …)` | SQL 文件里定义的宏 | — |

返回形态不在表内会直接编译报错，报错信息里会列出受支持的形式。

## 参数

所有函数属性共用同一套参数：

| 参数 | 默认值 | 含义 |
| --- | --- | --- |
| `auto_register` | `true` | 设为 `false` 时只生成 builder，不注册该函数。 |
| `named_param_from` | — | 表函数用：从该参数起（含）全部作为命名参数。 |
| `special_null_handling` | `false` | 让 DuckDB 把 `NULL` 入参交给回调，而不是在 bind 阶段折叠掉。见[标量函数](./scalar-functions.md#null-的处理)。 |
| `implicit_cost` | — | 类型转换用：隐式转换代价。 |
| `overloads_name` | — | 以该函数集的重载形式注册，而不是注册自身的函数名。 |

`#[derive(DuckStruct)]` 通过 `#[duck(...)]` 属性支持同一套参数，结构体式表函数就靠它声明命名参数的起点：

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "start")]
struct CountDownS {
    start: i64,
}
```

`#[derive(DuckEnum)]` 把「只有单元变体的枚举」映射为 DuckDB 的 `ENUM`（字典 = 声明顺序）。
它自己只有一个参数 —— `rename_all`（`lowercase`、`UPPERCASE`、`snake_case`、
`SCREAMING_SNAKE_CASE`、`camelCase`、`PascalCase`、`kebab-case`、`SCREAMING-KEBAB-CASE`，默认
`verbatim`）—— 外加变体上的 `#[duck(rename = "...")]`：

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}
```

之后这个枚举可以用在任何需要值类型的地方 —— 函数参数、返回值、`STRUCT` 字段、容器元素 ——
`Option<Priority>` 则表示可空。**非可空**参数额外需要 `Default`（宏生成的参数结构体会 `derive(Default)`），
所以例子里的 `Default` 与 `#[default]` 是必需的；写成 `Option<Priority>` 就不需要。

### 在 catalog 里建命名类型

两个 derive 都支持：

| 参数 | 默认值 | 含义 |
| --- | --- | --- |
| `sql_name` | 类型名的小写蛇形 | SQL 侧类型名（`Priority` → `priority`、`Ticket` → `ticket`）。 |
| `create_type` | `false` | `true`：扩展加载时执行 `CREATE TYPE IF NOT EXISTS <sql_name> AS <类型>;`；`"print"`：只把该语句打印到 stderr；`false`：什么都不做。 |

语句是幂等的 —— `LOAD` 两次也没问题 —— 且不会覆盖已存在的同名类型；执行路径与 SQL 宏相同
（`duckdb_query`）。枚举建成 `ENUM(...)`，结构体建成 `STRUCT(...)`，而结构体的字段类型是从 DuckDB
**自己的逻辑类型**渲染出来的，所以嵌套枚举/结构体、`LIST` / `ARRAY` / `MAP`、手写的自定义字段类型
都会自动带上：

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = true)]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,
    pub labels: Vec<String>,
}
// CREATE TYPE IF NOT EXISTS "ticket" AS
//   STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'), "labels" VARCHAR[]);
```

`create_type = "print"` 渲染的是**完全相同**的那条语句，只是不注册进 catalog。DDL 会先被收进
队列，等全部注册项跑完再**一次性**打出来 —— 所以哪怕有十几个 `"print"` 类型，也只有一块提示，
不会每个类型重复一遍「未执行 / 可手动执行」。提示行用 `-- [duckfn]` 开头（SQL 注释），
整块直接复制出去就能跑，也不会再出现「一行孤零零的 DDL、看不出到底跑没跑」的歧义：

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = "print")]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,
}
```

```
-- [duckfn] create_type = "print": the statement below was NOT executed.
-- [duckfn] Copy it and run it yourself if you want the type created.
CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'));
-- [duckfn] end - nothing above was executed.
```

如果希望宏完全不介入、DDL 与提示都由自己安排，就保持默认的 `create_type = false`，在
`#[duck_custom_register]` 里自己打印 —— 用现成的 `duckfn::named_type_ddl` 渲染、
用 `duckfn::print_sql_preview` 加自己的说明：

```rust
#[duck_custom_register]
fn show_the_create_type_ddl(_connection: &Connection) -> DuckResult<()> {
    // create_type = false：要不要亮出 DDL、配什么说明，全部由作者决定
    let ddl = duckfn::named_type_ddl("ticket", &Ticket::logical_type())?;
    duckfn::print_sql_preview(
        "my extension will NOT create this type",
        &ddl,
        "end - copy the statement above and run it yourself if you want it",
    );
    Ok(())
}
```

枚举同理：`duckfn::named_type_ddl("priority", &Priority::logical_type())`（`logical_type()` 本身就
带字典），或者用 `duckfn::register_enum_type` / `duckfn::queue_enum_type_ddl` 直接给标签列表。

之后 SQL 里就能直接把 `ticket` 当类型用（列类型、cast 目标），而函数注册用的仍是等价的结构化类型 ——
两者可以互相转换。见[类型 → 枚举](./types.md#枚举)与[类型 → 结构体](./types.md#结构体)。

## 宏展开了什么

函数属性会展开成「原函数 + 一个**以函数名命名**的模块」：

```rust
#[duck_scalar_function]
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}
```

大致等价于：

```rust
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}

mod dfn_scalar_reg_manual {
    use super::*;

    // 每个参数一个字段，由 derive 映射成 STRUCT。
    #[derive(duckfn::DuckStruct, Debug, Clone, Default)]
    pub struct DuckArgsImpl {
        pub i: i32,
    }

    pub struct ScalarFunctionImpl;
    impl duckfn::ScalarFunctionAdapter for ScalarFunctionImpl {
        const NAME: &'static str = "dfn_scalar_reg_manual";
        type Args = DuckArgsImpl;
        type Output = i32;
        // …
    }

    pub fn scalar_function_builder() -> quack_rs::prelude::ScalarFunctionBuilder { /* … */ }
    pub fn scalar_overload_builder() -> quack_rs::prelude::ScalarOverloadBuilder { /* … */ }
}
```

模块会继承函数的可见性，`pub fn` 得到 `pub mod`。各属性宏在模块内提供的内容：

| 属性宏 | 生成的 item |
| --- | --- |
| `#[duck_scalar_function]` | `ScalarFunctionImpl`、`scalar_function_builder()`、`scalar_overload_builder()` |
| `#[duck_aggregate_function]` | `AggregateFunctionImpl`、`aggregate_function_builder()`、`aggregate_overload_builder(builder)`、`aggregate_function_guard()` |
| `#[duck_table_function]` | `TableFunctionImpl`、`table_function_builder()`（返回 `DuckResult`） |
| `#[duck_cast_function]` | `CastFunctionImpl`、`cast_function_builder()`、`cast_function_register(connection)` |
| `#[duck_replacement_scan]` | `ReplacementScanImpl`、`replacement_scan_register(connection)`，不生成 `DuckArgsImpl` |

`#[duck_custom_register]` 与 `#[duck_sql_macro]` 完全不生成模块：函数保持原样，只向注册表提交一项。

## 自动注册与手动注册

默认 `auto_register = true` 时，宏把自己提交到 `inventory` 注册表，由 `duckfn_entrypoint!` 生成的入口点在
DuckDB 加载扩展时统一注册。这一段样板代码不需要你写。

写成 `auto_register = false` 时函数照样生成，但不会注册，SQL 层看不到它，直到你手动注册：

```rust
#[duck_scalar_function(auto_register = false)]
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}

#[duck_custom_register]
fn dfn_scalar_reg_manual_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_scalar(dfn_scalar_reg_manual::scalar_function_builder()) }
}
```

`#[duck_custom_register]` 不做任何包装，因此签名必须正好是 `fn(&Connection) -> DuckResult<()>`。
其它类型同理：

```rust
unsafe { c.register_aggregate(dfn_agg_reg_manual::aggregate_function_builder()) }  // 聚合函数
unsafe { c.register_table(dfn_table_reg_manual::table_function_builder()?) }      // 表函数
dfn_cast_manual::cast_function_register(c)                                        // 类型转换
dfn_scan_manual::replacement_scan_register(c)                                     // 替换扫描
```

注意表函数的 builder 返回 `DuckResult`，所以要加 `?`；类型转换与替换扫描的辅助函数直接接收连接。

手动注册也是自己拼装函数集的方式：

```rust
#[duck_custom_register]
fn dfn_scalar_reg_over_register(c: &Connection) -> DuckResult<()> {
    unsafe {
        c.register_scalar_set(
            ScalarFunctionSetBuilder::new("dfn_scalar_reg_overload")
                .overload(dfn_scalar_reg_over_int::scalar_overload_builder())
                .overload(dfn_scalar_reg_over_varchar::scalar_overload_builder()),
        )
    }
}
```

`ScalarFunctionSetBuilder`、`Connection` 以及这些注册方法都来自 `quack-rs`；为什么需要自己添加这个依赖，
见[安装](../getting-started/installation.md)。

## 入口点

```rust
duckfn_entrypoint!("my_ext");
```

它生成符号 `my_ext_init_c_api`，正是 DuckDB 加载扩展时查找的符号。名称不能为空，且只能包含小写 ASCII
字母、数字和下划线；不满足会在编译期被拒绝，而符号名写错则会导致扩展加载失败。

## 从文件注册 SQL 宏

`duck_sql_macro_files!` 用于把宏放在 `.sql` 文件里，而不是 Rust 字符串字面量中：

```rust
duck_sql_macro_files!(
    "sql/macro_files_a.sql",
    "sql/macro_files_b.sql",
    "sql/macro_files_c.sql"
);
```

路径相对调用宏的 `.rs` 文件解析，编译期由 `include_str!` 内联，并按书写顺序执行。一个文件里可以定义任意多个宏，
详见 [SQL 宏](./sql-macros.md)。

## 源码与测试

- [`duckfn-macro/src/attr_args.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/src/attr_args.rs) —— 所有属性共用的参数定义
- [`duckfn-macro/src/duck_function.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/src/duck_function.rs) —— 各宏展开成什么
- [`src/extension/functions/scalar_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/scalar_function.rs) 与 [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) —— 手动注册的示例
