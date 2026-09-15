<div align="center">
  <img src="https://raw.githubusercontent.com/shijianjs/duckfn/main/docs/static/img/duckfn-logo.svg" alt="duckfn logo" width="180" />
</div>

# duckfn

[English](README.md) | [简体中文](README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
[![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)

**用纯 Rust 写 DuckDB 扩展。**

`duckfn` 是一个基于 DuckDB C Extension API 的 Rust 框架。借助一个属性宏，就能把普通的 Rust
函数变成 DuckDB 的**标量函数**、**聚合函数**、**表函数**、SQL 宏、替换扫描（replacement scan）、类型转换，
或嵌套类型 —— 无需 C/C++ 胶水代码，也无需在本地编译 DuckDB。

- 仓库地址：<https://github.com/shijianjs/duckfn>
- 已发布 crate：[`duckfn`](https://crates.io/crates/duckfn) · [`duckfn-macro`](https://crates.io/crates/duckfn-macro)
- 基于：[`quack-rs`](https://crates.io/crates/quack-rs) · [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys)
- 无需手写 `unsafe`：不需要 `unsafe fn`，函数体里也碰不到裸指针
- 无需编译 DuckDB，无需 C/C++ 代码
- 属性驱动、基于 `inventory` 的自动注册
- panic 安全：Rust panic 会转成 DuckDB 错误，不会跨 FFI 边界展开
- 可直接复用 DuckDB 官方多平台扩展 CI

> 状态：早期 / 实验性，`1.0` 之前 API 可能变化。

## 仓库结构

本仓库是一个 Cargo workspace：

| 路径 | 说明 | 是否发布到 crates.io |
| --- | --- | --- |
| [`duckfn/`](duckfn/) | 运行时框架：trait、类型适配、函数注册。 | 是 |
| [`duckfn-macro/`](duckfn-macro/) | 过程宏：`#[duck_scalar_function]`、`#[derive(DuckStruct)]` 等。 | 是 |
| `/`（`rusty_quack`） | 使用 `duckfn` 编写的示例扩展，放在根目录是为了复用官方多平台 CI。 | 否，仅作示例 |

## 安装

```toml
[dependencies]
duckfn = "0.0.2"

# duckfn 本身就建立在下面两个 crate 之上；需要直接使用它们的类型或 builder 时显式加上
# （本仓库的示例扩展就是这么写的）。
quack-rs = "0.16.0"
# loadable-extension：走 DuckDB 的 API 函数表分发，而不是链接 libduckdb，
# 这正是「无需在本地编译 DuckDB」的原因。（仅使用头文件）
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

宏已由 `duckfn` 重新导出；只有想脱离运行时单独使用宏时，才需要直接依赖
[`duckfn-macro`](https://crates.io/crates/duckfn-macro)。

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

// 生成 DuckDB 扩展入口（扩展名必须全小写、仅含下划线）
duckfn_entrypoint!("my_ext");
```

`#[duck_scalar_function]` 会自动生成 DuckDB 包装层、逻辑类型，并默认通过 `inventory` 完成注册。
`duckfn_entrypoint!` 负责导出 DuckDB 加载扩展时查找的 `*_init_c_api` 符号。

上面这段全是安全 Rust：不需要写 `unsafe fn`，不需要解引用裸指针，也不需要接触 DuckDB 的 C 类型。

## 文档

完整文档 —— 安装、每个属性的用法、类型映射、错误模型以及可运行的示例扩展 —— 见
**<https://shijianjs.github.io/duckfn/zh-Hans/>**：

| 页面 | 内容 |
| --- | --- |
| [简介](https://shijianjs.github.io/duckfn/zh-Hans/docs/intro) | duckfn 是什么，各 crate 如何配合。 |
| [创建项目](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/create-a-project) | 从 DuckDB 官方 Rust 扩展模板起步。 |
| [安装](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/installation) | 依赖、MSRV，以及为什么不需要编译 DuckDB。 |
| [快速开始](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/quick-start) | 编写、构建并加载第一个扩展。 |
| [指南](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/attributes) | 属性参考、标量/聚合/表函数、类型转换、替换扫描、SQL 宏。 |
| [类型映射](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/types) | DuckDB 与 Rust 的类型对应、可空性规则与已知缺口。 |
| [错误与 panic](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/errors-and-panics) | `duck_error`、`DuckOptionResult` 与 panic 的处理。 |
| [示例扩展](https://shijianjs.github.io/duckfn/zh-Hans/docs/examples/rusty-quack) | `rusty_quack`，每个功能都配可运行的 SQL。 |
| [构建与发布](https://shijianjs.github.io/duckfn/zh-Hans/docs/build-and-release) · [贡献指南](https://shijianjs.github.io/duckfn/zh-Hans/docs/contributing) · [常见问题](https://shijianjs.github.io/duckfn/zh-Hans/docs/faq) | 本地构建、CI 与排错。 |

English docs: <https://shijianjs.github.io/duckfn/>

API 文档：<https://docs.rs/duckfn> · 库使用说明：[`duckfn/README.zh-CN.md`](duckfn/README.zh-CN.md)

## 协议

基于 [MIT 协议](LICENSE) 开源。
