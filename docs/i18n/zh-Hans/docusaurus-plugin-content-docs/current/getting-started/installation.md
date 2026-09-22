---
title: 安装
sidebar_position: 2
description: 添加 duckfn 依赖、声明 cdylib crate 类型，以及为什么不需要本地编译 DuckDB。
---

# 安装

## 添加依赖

```toml
[dependencies]
duckfn = "{{DUCKFN_VERSION}}"

# duckfn 基于这两个 crate 构建。只要你直接书写它们的类型或 builder ——
# 例如 SqlMacro、Connection、LogicalType、Value —— 就需要显式添加。
quack-rs = "0.16.0"
# `loadable-extension` 通过 DuckDB 的 API table 分发，而不是链接 libduckdb，
# 这正是无需本地编译 DuckDB 的原因。只用到头文件。
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

`duckfn` 已重新导出 [`duckfn-macro`](https://crates.io/crates/duckfn-macro) 的全部宏，因此上面的片段就够了。
只有想脱离运行时单独使用宏时，才需要直接依赖 `duckfn-macro = "{{DUCKFN_VERSION}}"`。

## Cargo feature

| feature | 作用 | 要求 |
| --- | --- | --- |
| `duckdb-1-5` | 支持 DuckDB 1.5 新增的逻辑类型 —— 目前是 `TIME_NS`（`DuckTimeNs`）。 | `libduckdb-sys` 使用 DuckDB 1.5 及以上的头文件。 |
| `chrono` | 时间包装类型与 [`chrono`](https://crates.io/crates/chrono) 的互转（`DuckDate::to_naive_date` 等），把纪元换算交给 duckfn。 | 由本 feature 引入的可选 `chrono` 依赖。 |

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["duckdb-1-5", "chrono"] }
```

`loadable-extension` 是 `libduckdb-sys` 的 feature，不是 `duckfn` 的，需要你自己开启。

## crate 必须产出 `cdylib`

DuckDB 以动态库的形式加载扩展：

```toml
[lib]
crate-type = ["cdylib"]
```

构建 WebAssembly 版本时 crate 类型则要改成 `staticlib`，因为最终链接由 `emcc` 完成。本仓库的示例扩展
采用单独一个 `[[example]]` 目标承载 `crate-type = ["staticlib"]` 来做到这一点。

## 为什么不需要本地编译 DuckDB

通常 DuckDB 扩展需要针对 DuckDB 本体编译并链接，`duckfn` 避开了这一步：

1. 开启 `loadable-extension` 的 `libduckdb-sys` 只针对 DuckDB **头文件**编译，不链接任何库。
2. DuckDB API 调用走一张函数指针表，宿主 DuckDB 在加载扩展并调用入口点时把这张表填好。

带来的好处是构建快、无外部依赖（交叉编译也能工作），代价是扩展必须与加载它的 DuckDB 版本匹配。
另外扩展被标记为使用 *unstable* C API，所以加载时必须加 `-unsigned`。

## 环境要求

| | |
| --- | --- |
| Rust | 1.86 及以上（crate 使用 edition 2024） |
| DuckDB | 针对 v1.5.5 验证 |
| Python 3 *（可选）* | 仅 `make configure` / `make test` 流程需要，用于准备 sqllogictest 运行环境 |

## 接下来

- [快速开始](./quick-start.md) —— 编写、构建并加载一个扩展。
- [属性参考](../guide/attributes.md) —— 完整的属性与参数说明在指南里。
- [架构](../internals/architecture.md) —— API table 分发到底是怎么工作的。
