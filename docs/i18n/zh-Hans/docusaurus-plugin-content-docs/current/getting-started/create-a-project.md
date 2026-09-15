---
title: 创建项目
sidebar_position: 1
description: 从 DuckDB 官方 Rust 扩展模板起步，用 quack-rs 写逻辑，用 cargo-duckdb-ext-tools 构建。
---

# 创建项目

动手写代码之前有三个决定：以什么为起点、用什么写、怎么构建。

## 从官方模板起步

[`duckdb/extension-template-rs`](https://github.com/duckdb/extension-template-rs) 是 DuckDB 官方的
Rust 扩展模板，也是新扩展最合适的起点：

- 自带完整的 GitHub Actions 流水线：为 DuckDB 支持的每个平台构建并测试扩展，打版本 tag 时发布二进制。
  这部分不需要自己写。
- 已经接好 `extension-ci-tools`，提供 `make configure` / `make debug` / `make release` / `make test` ——
  与 DuckDB 自身用的是同一套流程，也是 sqllogictest 运行器需要的流程。
- 你正在读的这个仓库就是一个换了函数的同类模板。

```shell
git clone --recurse-submodules https://github.com/duckdb/extension-template-rs my_ext
cd my_ext
```

`extension-ci-tools` 是 git submodule，所以克隆时要带 `--recurse-submodules`，或首次构建前先执行
`git submodule update --init --recursive`。

然后改掉模板里写死的部分：`Makefile` 里的 `EXTENSION_NAME`、要构建 WebAssembly 时的 `[[example]]`
目标，以及传给 `duckfn_entrypoint!` 的名字。

改的时候让两个 crate root 保持一致：`src/lib.rs` 与 `src/wasm_lib.rs` 必须声明同样的模块，官方模板那种
`mod lib;` 再导出一旦遇到嵌套模块就会报 `error[E0583]` —— 见[问题排查](../troubleshooting.md#嵌套模块时报-e0583)。

## 用 quack-rs 写逻辑

`duckfn` 建立在 [`quack-rs`](https://github.com/tomtom215/quack-rs) 之上 —— 它是对 DuckDB C API
覆盖最全、文档最完整的绑定。当属性宏没有暴露的能力（手写 `LogicalType`、向量级操作、C API 的某个角落）
成为必需时，要用的就是它；它本来就在你的依赖列表里。

## 用 cargo-duckdb-ext-tools 构建

日常开发用
[`redraiment/cargo-duckdb-ext-tools`](https://github.com/redraiment/cargo-duckdb-ext-tools) 直接从 Cargo 打包：

```shell
cargo install cargo-duckdb-ext-tools   # 只需安装一次
cargo duckdb-ext build                 # -> target/debug/my_ext.duckdb_extension
```

它是一个全局 `cargo` 子命令，因此**不给项目增加任何依赖**，也不要求模板那条 `make` 流程先跑通：

```shell
duckdb -unsigned -c "
LOAD './target/debug/my_ext.duckdb_extension';
SELECT double_it(21);
"
```

本仓库用 `Justfile` 封装了两种流程，见[快速开始](./quick-start.md#3-构建)。

## 什么时候仍然需要官方流程

用 Cargo 构建足以覆盖开发，但有两件事仍然依赖 `make`：

- sqllogictest 套件（`make test`）—— 这是测试 DuckDB 扩展的标准方式，比 Rust 测试框架更贴合扩展行为。
- CI，跑的是同一套 makefile。

所以常见的搭配是：先让 `make configure` 跑通一次，迭代时用 Cargo 构建，推送前跑 `make test`。
在 Windows 上，这意味着 `make` 要在 Git Bash 里运行，而不是 PowerShell —— 见[贡献指南](../contributing.md#windows)。

## 接下来

- [安装](./installation.md) —— 把 duckfn 加进 crate。
- [快速开始](./quick-start.md) —— 编写、构建并加载扩展。
- [构建与发布](../build-and-release.md) —— 模板的 CI 在打 tag 时做了什么。
