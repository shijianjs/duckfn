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
