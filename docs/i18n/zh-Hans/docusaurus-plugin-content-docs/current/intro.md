---
title: 简介
sidebar_position: 1
slug: /intro
description: duckfn 可以把普通的 Rust 函数变成 DuckDB 的标量函数、聚合函数、表函数、SQL 宏、类型转换与替换扫描，无需 C/C++ 胶水代码，也无需本地编译 DuckDB。
---

# 简介

**用纯 Rust 编写 DuckDB 扩展。**

`duckfn` 是一个基于 DuckDB C Extension API 的框架。加一个属性，普通的 Rust 函数就变成 DuckDB 的
**标量函数**、**聚合函数**、**表函数**、SQL 宏、替换扫描（replacement scan）或类型转换。

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

// 生成 DuckDB 加载扩展时查找的符号。
duckfn_entrypoint!("my_ext");
```

FFI 包装、列读写与注册代码都由属性宏生成，所以上面这段完全是安全 Rust。

## 为什么用 duckfn

| | |
| --- | --- |
| **没有 C/C++ 胶水代码** | 代码里不会出现 DuckDB 的 C 类型，写的是 `Option<i64>`、`Vec<String>` 和 `#[derive(DuckStruct)]` 结构体。 |
| **不需要本地编译 DuckDB** | 只依赖头文件编译，加载时通过 DuckDB 的 API table 分发。 |
| **默认安全** | 函数体里没有 `unsafe fn`，也没有裸指针。仅手动注册处还留有 `unsafe`。 |
| **属性驱动注册** | 函数加上属性即可，注册项用 `inventory` 收集，扩展加载时统一注册。 |
| **panic 安全** | 函数体里的 panic 会被捕获并转成 DuckDB 错误，不会跨 FFI 边界展开。 |
| **支持嵌套类型** | `Vec<T>`、`IndexMap<K, V>`、定长数组、`STRUCT` 以及它们的任意嵌套，对应 DuckDB 的 LIST、MAP、ARRAY、STRUCT。 |
| **复用官方 CI** | 本仓库沿用 DuckDB 官方多平台扩展流水线，打 tag 即可产出各平台二进制。 |

## 各部分如何配合

| Crate | 作用 |
| --- | --- |
| [`duckfn`](https://crates.io/crates/duckfn) | 运行时框架：trait、类型适配器、值类型、注册机制；并重新导出全部宏。 |
| [`duckfn-macro`](https://crates.io/crates/duckfn-macro) | `#[duck_scalar_function]`、`#[derive(DuckStruct)]` 等过程宏的实现。 |
| [`quack-rs`](https://crates.io/crates/quack-rs) | `duckfn` 依赖的 DuckDB C API 绑定，其 builder 与值类型属于公共接口的一部分。 |
| [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys) | DuckDB 头文件；开启 `loadable-extension` 后编译期不链接任何库。 |

## 状态

> 早期 / 实验阶段 —— `1.0` 之前 API 可能变动。

## 接下来

- [创建项目](./getting-started/create-a-project.md) —— 从 DuckDB 官方 Rust 扩展模板起步。
- [安装](./getting-started/installation.md) —— 依赖、MSRV，以及为什么不需要编译 DuckDB。
- [快速开始](./getting-started/quick-start.md) —— 构建并加载第一个扩展。
- [属性参考](./guide/attributes.md) —— 完整的属性与参数说明。
- [示例扩展](./examples/rusty-quack.md) —— 覆盖全部功能的可运行示例。
