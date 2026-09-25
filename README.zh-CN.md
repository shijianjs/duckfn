<div align="center">
  <img src="https://raw.githubusercontent.com/shijianjs/duckfn/main/docs/static/img/duckfn-logo.svg" alt="duckfn logo" width="180" />
</div>

# duckfn

[English](README.md) | [简体中文](README.zh-CN.md)

[![GitHub](https://img.shields.io/badge/GitHub-181717?logo=github&logoColor=white)](https://github.com/shijianjs/duckfn)
[![Docs](https://img.shields.io/badge/docs-duckfn-14459b?logo=docusaurus&logoColor=white)](https://shijianjs.github.io/duckfn/)
[![crates.io](https://img.shields.io/crates/v/duckfn.svg)](https://crates.io/crates/duckfn)
[![docs.rs](https://docs.rs/duckfn/badge.svg)](https://docs.rs/duckfn)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/shijianjs/duckfn/blob/main/LICENSE)
[![zread](https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff)](https://zread.ai/shijianjs/duckfn)

**用纯 Rust 写 DuckDB 扩展。**

`duckfn` 是一个基于 DuckDB C Extension API 的 Rust 框架。借助一个属性宏，就能把普通的 Rust
函数变成 DuckDB 的**标量函数**、**聚合函数**、**表函数**、**COPY 函数**、SQL 宏、替换扫描（replacement scan）、类型转换，
或嵌套类型 —— 无需 C/C++ 胶水代码，也无需在本地编译 DuckDB。

- 仓库地址：<https://github.com/shijianjs/duckfn>
- 已发布 crate：[`duckfn`](https://crates.io/crates/duckfn) · [`duckfn-macro`](https://crates.io/crates/duckfn-macro)
- 基于：[`quack-rs`](https://crates.io/crates/quack-rs) · [`libduckdb-sys`](https://crates.io/crates/libduckdb-sys)
- 无需手写 `unsafe`：不需要 `unsafe fn`，函数体里也碰不到裸指针
- 无需编译 DuckDB，无需 C/C++ 代码
- 属性驱动、基于 `inventory` 的自动注册
- panic 安全：Rust panic 会转成 DuckDB 错误，不会跨 FFI 边界展开
- 宿主文件系统访问：任何回调（包括聚合函数）都能通过 DuckDB 的虚拟文件系统读写文件（`s3://`、`http(s)://`（需 httpfs）、内存文件），并提供 `duckfn::duck_vfs::read_string` / `write_string` / `append_string` 这类一行式接口，由 `duckdb-1-5` feature 提供
- 与其它 crate 互转：时间包装类型（`DuckDate`、`DuckTimestamp` / `_S` / `_Ms` / `_Ns`、`DuckTimestampTz`、`DuckTime`）与 [`chrono`](https://crates.io/crates/chrono) 双向转换，`DuckUuid` ↔ [`uuid`](https://crates.io/crates/uuid)，`DuckDecimal<W, S>` ↔ [`rust_decimal`](https://crates.io/crates/rust_decimal)（`chrono` / `uuid` / `rust_decimal` feature）—— 越界、DuckDB 的 `infinity` 与会丢位的转换都以错误返回，不会 panic，也不会静默截断
- 函数文档：在任何 `#[duck_*]` 属性上写 `description` / `comment` / `example`，由 `cli` feature 提供的命令行工具导出成 DuckDB 社区扩展文档页读取的 `function_descriptions.csv`
- 可直接复用 DuckDB 官方多平台扩展 CI

> 状态：早期 / 实验性，`1.0` 之前 API 可能变化。

## 仓库结构

本仓库是一个 Cargo workspace，而根目录同时就是 `duckfn` crate 的包根：

| 路径 | 说明 | 是否发布到 crates.io |
| --- | --- | --- |
| `/`（`duckfn`） | 运行时框架：trait、类型适配、函数注册。同时是 workspace 根。 | 是 |
| [`duckfn-macro/`](duckfn-macro/) | 过程宏：`#[duck_scalar_function]`、`#[derive(DuckStruct)]`、`#[derive(DuckEnum)]` 等。 | 是 |
| [`duckfn-quack/`](duckfn-quack/) | 使用 `duckfn` 编写的示例扩展，每个功能都配可运行的 SQL，`test/sql/` 下是它的 sqllogictest 用例。放在仓库里是为了复用官方多平台 CI。 | 否，仅作示例 —— cargo 不会打包任何含 `Cargo.toml` 的子目录，所以它只能留在仓库里 |
| [`docs/`](docs/) | Docusaurus 文档站：`docs/docs/**`（英文）与 `docs/i18n/zh-Hans/**`（简体中文）。 | 否，文档站 —— 但正文会随 `duckfn` 包一起发布 |

发布到 crates.io 的 `duckfn` 包里带着运行时、测试、本 README、许可证与整份文档源文件，所以想读
文档（包括那页可以照着跑的示例页）都不必再克隆仓库（在仓库根跑 `cargo package --list` 可以看到确切的
文件清单）。示例扩展自身的 Rust 源码不在包里：cargo 不会打包任何含 `Cargo.toml` 的子目录，所以
`duckfn-quack/` 留在这里。

## 安装

```toml
[dependencies]
duckfn = "0.0.10"

# duckfn 本身就建立在下面两个 crate 之上；需要直接使用它们的类型或 builder 时显式加上。
quack-rs = "0.16.0"
# loadable-extension：走 DuckDB 的 API 函数表分发，而不是链接 libduckdb，
# 这也是「无需在本地编译 DuckDB」的原因。
# （仅使用头文件，不链接库）
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

如果只想用宏、不要运行时，可以直接依赖
[`duckfn-macro`](https://crates.io/crates/duckfn-macro)；否则宏已由 `duckfn` 重新导出，不必额外添加。

`duckfn` 有六个可选 feature。`cli` 带来导出 `function_descriptions.csv` 的命令行工具（供 DuckDB 社区
扩展文档页使用，本仓库里的命令是 `cargo run -p duckfn_quack --bin duckfn -- function_descriptions`），
只有扩展项目的 `src/bin/duckfn.rs` 需要它。`duckdb-1-5` 用于开启 DuckDB 1.5 C API 带来的能力：1.5
新增的逻辑类型（目前是 `TIME_NS`）、COPY 函数，以及宿主文件系统访问。另外三个是互转，彼此独立：
`chrono` 让时间包装类型与 [`chrono`](https://crates.io/crates/chrono) 双向转换，`uuid` 让 `DuckUuid` 与
[`uuid`](https://crates.io/crates/uuid) 双向转换，`rust_decimal` 让 `DuckDecimal<W, S>` 与
[`rust_decimal`](https://crates.io/crates/rust_decimal) 双向转换 —— 纪元 / 128 位 / 定标整数的换算都收进
duckfn，而不是留给每个扩展各写一遍：

```toml
duckfn = { version = "0.0.10", features = ["duckdb-1-5", "chrono", "uuid", "rust_decimal"] }
```

`all` 是聚合开关，一次把上面这五个都打开。duckfn 通常就在依赖树的末端，所以直接写
`features = ["all"]` 最省事；想精简依赖树时再按上面的单项挑着开。

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

包装层、逻辑类型和注册都由宏生成，所以上面这段全是安全 Rust。唯一会出现 `unsafe` 的地方是手动注册：
`#[duck_custom_register]` 中调用 quack-rs 的 `unsafe fn register_scalar` /
`register_aggregate` / `register_table`。

`#[duck_scalar_function]` 会生成一个与函数同名的模块，里面暴露 `scalar_function_builder()`、
`scalar_overload_builder()` 等 builder，方便自行注册重载或函数集。

## 文档

完整指南 —— 每个属性及其参数、类型映射、错误模型以及可运行的示例扩展 —— 见
**<https://shijianjs.github.io/duckfn/zh-Hans/>**：

| 页面 | 内容 |
| --- | --- |
| [简介](https://shijianjs.github.io/duckfn/zh-Hans/docs/intro) | duckfn 是什么，各 crate 如何配合。 |
| [创建项目](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/create-a-project) | 从 [duckfn 扩展模板](https://github.com/shijianjs/duckfn-extension-template)或 DuckDB 官方 Rust 扩展模板起步。 |
| [项目结构约定](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/project-structure) | 几个 crate root、`error[E0583]`、以及命令行工具自己的那个 root。 |
| [安装](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/installation) | 依赖、MSRV，以及为什么不需要编译 DuckDB。 |
| [快速开始](https://shijianjs.github.io/duckfn/zh-Hans/docs/getting-started/quick-start) | 编写、构建并加载第一个扩展。 |
| [属性参考](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/attributes) | 全部属性、公共参数与手动注册。 |
| [标量函数](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/scalar-functions) | 返回形态、`NULL` 处理、批量模式、重载。 |
| [聚合函数](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/aggregate-functions) | 行处理函数、状态类型、并行聚合。 |
| [表函数](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/table-functions) | 行结构体、命名参数、流式输出。 |
| [COPY 函数](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/copy-functions) | 为 `COPY ... TO` / `COPY ... FROM` 提供自定义文件格式，基于运行时动态列。 |
| [类型转换](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/casts) | 覆盖某一对源类型/目标类型的 `CAST`。 |
| [替换扫描](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/replacement-scans) | 让 `SELECT * FROM 'data.points'` 生效。 |
| [SQL 宏](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/sql-macros) | 用 Rust 或 `.sql` 文件注册宏。 |
| [类型映射](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/types) | DuckDB 与 Rust 的类型对应、可空性规则与已知缺口。 |
| [社区扩展文档页](https://shijianjs.github.io/duckfn/zh-Hans/docs/community-extension-docs) | `description` / `comment` / `example` 三个属性，以及 DuckDB 社区扩展文档页读取的那份 CSV。 |
| [错误与 panic](https://shijianjs.github.io/duckfn/zh-Hans/docs/guide/errors-and-panics) · [架构](https://shijianjs.github.io/duckfn/zh-Hans/docs/internals/architecture) | 错误处理、宏展开、注册与适配器。 |
| [示例扩展](https://shijianjs.github.io/duckfn/zh-Hans/docs/examples/duckfn-quack) | `duckfn_quack`，随本包一起发布的示例，每个功能都配可运行的 SQL。 |
| [构建与发布](https://shijianjs.github.io/duckfn/zh-Hans/docs/build-and-release) · [贡献指南](https://shijianjs.github.io/duckfn/zh-Hans/docs/contributing) · [常见问题](https://shijianjs.github.io/duckfn/zh-Hans/docs/faq) | 本地构建、CI 与排错。 |

English docs: <https://shijianjs.github.io/duckfn/>

Rust API 文档：<https://docs.rs/duckfn>

## 协议

基于 [MIT 协议](LICENSE) 开源。
