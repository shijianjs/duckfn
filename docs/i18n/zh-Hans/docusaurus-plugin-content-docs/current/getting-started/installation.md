---
title: 安装
sidebar_position: 3
description: 添加 duckfn 依赖、声明 cdylib crate 类型，以及为什么不需要本地编译 DuckDB。
---

# 安装

## 添加依赖

```toml
[dependencies]
duckfn = "{{DUCKFN_VERSION}}"

# duckfn 基于这两个 crate 构建。只要你直接书写它们的类型或 builder ——
# 例如 SqlMacro、Connection、LogicalType、Value —— 就需要显式添加。
quack-rs = "0.18.0"
# `loadable-extension` 通过 DuckDB 的 API table 分发，而不是链接 libduckdb，
# 这正是无需本地编译 DuckDB 的原因。只用到头文件。
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

`duckfn` 已重新导出 [`duckfn-macro`](https://crates.io/crates/duckfn-macro) 的全部宏，因此上面的片段就够了。
只有想脱离运行时单独使用宏时，才需要直接依赖 `duckfn-macro = "{{DUCKFN_VERSION}}"`。

## Cargo feature

| feature | 作用 | 要求 |
| --- | --- | --- |
| `duckdb-1-5` | 支持 DuckDB 1.5 新增的逻辑类型 —— 目前是 `TIME_NS`（`DuckTimeNs`）—— 以及 `COPY ... TO` 用的 COPY 函数类型。 | `libduckdb-sys` 使用 DuckDB 1.5 及以上的头文件。 |
| `sql` | 在回调里执行 SQL（`duckfn::sql`）：注册期捕获一条常驻长连接，之后用 `execute` / `query`（可绑定参数）跑语句，结果按动态列读回。**只用 C API 的稳定区**，因此不需要 `duckdb-1-5`：配 `USE_UNSTABLE_C_API=0` 与 `v1.2.0` 的 floor，一份二进制就能在 DuckDB 1.3 – 2.0 上加载。有副作用：连接跟进程同寿、取用串行，并且在「同线程同步执行 DuckDB」的运行时（DuckDB-Wasm 的 Node **blocking** 绑定）里会死锁 —— 见[执行 SQL](../guide/sql-execution.md#代价与限制)。 | 除本 crate 自身的 `libduckdb-sys` 外没有额外要求：所有调用都在稳定区，这也正是产物能跨版本加载的原因。 |
| `duck_vfs` | 宿主文件系统（`duckfn::duck_vfs`）：在同一条注册期长连接之上再加 DuckDB 的 VFS，让聚合函数这类拿不到客户端上下文的回调也能读写文件（含 `s3://` 与 `http(s)://`）。它依赖 `owned-connection` 与 `duckdb-1-5`，副作用同 `sql` —— 见[文件系统访问](../guide/file-system.md#代价)。 | 其余与 `duckdb-1-5` 相同。 |
| `owned-connection` | 两项「回调侧能力」共用的底座：注册期捕获的一条常驻长连接。它**不暴露任何 API** —— `sql` 与 `duck_vfs` 是它的两个公开面，各自依赖它，而它不依赖任何一方。只在想把这个底座写明确时才直接列它。 | 无。 |
| `chrono` | 时间包装类型与 [`chrono`](https://crates.io/crates/chrono) 的互转（`DuckDate::to_naive_date` 等），把纪元换算交给 duckfn。 | 由本 feature 引入的可选 `chrono` 依赖。 |
| `uuid` | `DuckUuid` 与 [`uuid`](https://crates.io/crates/uuid) 的互转（`to_uuid` / `from_uuid`）。 | 可选的 `uuid` 依赖。 |
| `rust_decimal` | `DuckDecimal<W, S>` 与 [`rust_decimal`](https://crates.io/crates/rust_decimal) 的互转；越界与丢位都以错误返回。 | 可选的 `rust_decimal` 依赖。 |
| `quack` | **本仓库**（以及模板）示例扩展与 CLI 的标记：它只决定那两个 target 编不编，不决定它们有什么能力 —— 那由构建时列的 feature 决定。下游永远用不到它。 | 示例自己的那份能力清单，由构建命令一并给出。 |

每个 feature 都以它暴露的模块命名，并且**没有「全开」开关**：依赖里写清楚要用哪几项。这是刻意的
——「全开」说不清代价（带连接的那两项有副作用；`duckdb-1-5` 把构建钉在某个引擎发行版上），也看不出
一份构建到底开了哪些能力。

三个互转 feature 各自独立，用到哪个开哪个：

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["sql", "chrono", "uuid", "rust_decimal"] }
```

`cli` 只多带 `clap` 与 `csv` 给 CLI 那个 bin 用，属于构建 `src/bin/duckfn.rs` 的那个 crate，不该进
想压到最小的依赖树。

`loadable-extension` 是 `libduckdb-sys` 的 feature，不是 `duckfn` 的，需要你自己开启。

## crate 必须产出 `cdylib`

DuckDB 以动态库的形式加载扩展：

```toml
[lib]
crate-type = ["cdylib"]
```

构建 WebAssembly 版本时 crate 类型则要改成 `staticlib`，因为最终链接由 `emcc` 完成。做法不是改 lib，
而是给 wasm 那边单独一个 crate root：一个 `[[example]]` 目标承载 `crate-type = ["staticlib"]`，见
[项目结构约定](./project-structure.md)。

## 为什么不需要本地编译 DuckDB

通常 DuckDB 扩展需要针对 DuckDB 本体编译并链接，`duckfn` 避开了这一步：

1. 开启 `loadable-extension` 的 `libduckdb-sys` 只针对 DuckDB **头文件**编译，不链接任何库。
2. DuckDB API 调用走一张函数指针表，宿主 DuckDB 在加载扩展并调用入口点时把这张表填好。

带来的好处是构建快、无外部依赖（交叉编译也能工作），代价是扩展必须与加载它的 DuckDB 版本匹配。
另外扩展被标记为使用 *unstable* C API，所以加载时必须加 `-unsigned`。

## 环境要求

| | |
| --- | --- |
| Rust | 1.88 及以上（crate 使用 edition 2024；下限来自 `duckfn-macro` 依赖的 `syn 3`，它需要 rustc 1.88） |
| DuckDB | 针对 v2.0.0 验证 |
| Python 3 *（可选）* | 仅 `make configure` / `make test` 流程需要，用于准备 sqllogictest 运行环境 |

## 接下来

- [快速开始](./quick-start.md) —— 编写、构建并加载一个扩展。
- [属性参考](../guide/attributes.md) —— 完整的属性与参数说明在指南里。
- [架构](../development/architecture.md) —— API table 分发到底是怎么工作的。
