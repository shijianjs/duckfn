# duckfn

[English](https://github.com/shijianjs/duckfn/blob/main/duckfn/README.md) | [简体中文](https://github.com/shijianjs/duckfn/blob/main/duckfn/README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
[![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)

**用纯 Rust 写 DuckDB 扩展。**

`duckfn` 是一个基于 DuckDB C Extension API 的运行时框架。配合
[`duckfn-macro`](https://crates.io/crates/duckfn-macro)，借助一个属性宏就能把普通的 Rust 函数
变成 DuckDB 的**标量函数**、**聚合函数**、**表函数**、SQL 宏、replacement scan、类型转换，或
嵌套类型 —— 无需 C/C++ 胶水代码，也无需在本地编译 DuckDB。

- 仓库地址：<https://github.com/shijianjs/duckfn>
- 基于 [`quack-rs`](https://crates.io/crates/quack-rs) · [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys)
- 无需手写 `unsafe`：不需要 `unsafe fn`，函数体里也碰不到裸指针
- 无需编译 DuckDB，无需 C/C++ 代码
- 属性驱动、基于 `inventory` 的自动注册
- panic 安全：Rust panic 会转成 DuckDB 错误，不会跨 FFI 边界展开
- 可直接复用 DuckDB 官方多平台扩展 CI

> 状态：早期 / 实验性，`1.0` 之前 API 可能变化。

## 安装

```toml
[dependencies]
duckfn = "0.0.2"

# duckfn 本身就建立在下面两个 crate 之上；需要直接使用它们的类型或 builder 时显式加上。
quack-rs = "0.16.0"
# loadable-extension：走 DuckDB 的 API 函数表分发，而不是链接 libduckdb，
# 这也是「无需在本地编译 DuckDB」的原因。
# （仅使用头文件，不链接库）
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

如果只想用宏、不要运行时，可以直接依赖
[`duckfn-macro`](https://crates.io/crates/duckfn-macro)；否则宏已由 `duckfn` 重新导出，不必额外添加。

## 快速开始

```rust
use duckfn::{duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult};

/// ```sql
/// SELECT double_it(21);    -- 42
/// SELECT double_it(NULL);  -- NULL
/// SELECT double_it(13);    -- error: unlucky input
/// ```
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

// 生成扩展入口（扩展名必须全小写、仅含下划线）
duckfn_entrypoint!("my_ext");
```

包装层、逻辑类型和注册都由宏生成，所以上面这段全是安全 Rust —— 不需要 `unsafe fn`，不碰裸
指针，也不接触 DuckDB 的 C 类型。唯一会出现 `unsafe` 的地方是手动注册：
`#[duck_custom_register]` 中调用 quack-rs 的 `unsafe fn register_scalar` /
`register_aggregate` / `register_table`。

## 可用宏

| 宏 | 作用 |
| --- | --- |
| `#[duck_scalar_function]` | 注册标量函数。 |
| `#[duck_aggregate_function]` | 注册聚合函数。 |
| `#[duck_cast_function]` | 注册类型转换（`CAST(x AS T)` / `TRY_CAST`）。唯一参数是源值、返回类型是目标类型；支持 `Option<T>` 入参、`implicit_cost = N` 与 `auto_register = false`。 |
| `#[duck_table_function]` | 注册表函数。 |
| `#[duck_replacement_scan]` | 把「DuckDB 不认识的表名（通常是文件路径）」重定向到表函数，即 `SELECT * FROM 'data.points'`。返回 `Option<String>` / `Option<&'static str>` / `DuckOptionResult<...>`；路径作为第一个 VARCHAR 参数传给目标表函数。 |
| `#[duck_sql_macro]` | 注册 SQL 宏。返回 `SqlMacro` / `DuckResult<SqlMacro>`，也可直接返回 SQL 字符串（`String` / `&'static str` / `DuckResult<...>`），注册时直接执行。 |
| `#[duck_custom_register]` | 手动注册 builder，签名为 `fn(&Connection) -> DuckResult<()>`。 |
| `#[derive(DuckStruct)]` | 把结构体映射为 DuckDB `STRUCT`。 |
| `duckfn_entrypoint!("name")` | 生成扩展入口。 |

常用参数：

- `auto_register = false` —— 只生成 builder（`scalar_function_builder()`、
  `scalar_overload_builder()` 等）不自动注册，配合 `#[duck_custom_register]` 使用。
- `named_param_from = "field"` —— 表函数命名参数从哪个字段开始。

`#[duck_scalar_function]` 会生成一个与函数同名的模块，里面暴露生成的 builder，方便自行注册重载
或函数集。

## 类型映射

| DuckDB | Rust |
| --- | --- |
| `BOOLEAN` | `bool` |
| `TINYINT` / `SMALLINT` / `INTEGER` / `BIGINT` | `i8` / `i16` / `i32` / `i64` |
| `UTINYINT` / `USMALLINT` / `UINTEGER` / `UBIGINT` | `u8` / `u16` / `u32` / `u64` |
| `HUGEINT` / `UHUGEINT` | `i128` / `u128` |
| `FLOAT` / `DOUBLE` | `f32` / `f64` |
| `VARCHAR` | `String` |
| `NULL` | `Option<T>` |
| `LIST(T)` | `Vec<T>`，支持嵌套（`Vec<Option<Vec<Option<T>>>>` 等） |
| `MAP(K, V)` | `IndexMap<K, V>` |
| `ARRAY(T, N)` | `[T; N]`（`DuckArray`）/ `[Option<T>; N]`（`DuckOptionArray`） |
| `STRUCT(...)` | `#[derive(DuckStruct)]`，支持嵌套 struct 和 list |

可空性由 Rust 签名决定：

- 非 `Option` 参数遇到 `NULL` 输入时整行短路为 `NULL`，函数体不会执行；
- `Option<T>` 参数把 `NULL` 读成 `None`，由函数自己决定语义。

## 错误处理与 panic

返回 `DuckOptionResult<T>`（即 `Result<Option<T>, ExtensionError>`）可以输出 `NULL`，或用
`duck_error("...")` 让整条查询失败。函数体内的 panic 会被捕获并转换成 DuckDB 错误，而不会跨
FFI 边界展开。

标量函数支持三种返回形式：

```rust
#[duck_scalar_function] fn plain(i: i32) -> i32 { i * 2 }                 // 永不为 NULL
#[duck_scalar_function] fn maybe(i: i32) -> Option<i32> { Some(i) }       // None -> SQL NULL
#[duck_scalar_function] fn checked(i: i32) -> duckfn::DuckOptionResult<i32> { Ok(Some(i)) }
```

## 示例扩展

仓库中有一个覆盖全部能力的示例扩展（`rusty_quack`）：

<https://github.com/shijianjs/duckfn>

## 协议

基于 [MIT 协议](https://github.com/shijianjs/duckfn/blob/main/LICENSE) 开源。
