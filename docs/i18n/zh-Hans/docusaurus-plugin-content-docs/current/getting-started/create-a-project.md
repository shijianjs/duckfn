---
title: 创建项目
sidebar_position: 1
description: 从 DuckDB 官方 Rust 扩展模板起步，用 duckfn、quack-rs 写逻辑，用 cargo-duckdb-ext-tools 构建。
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

改的时候别动这几个 crate root：`src/lib.rs`、`src/wasm_lib.rs` 与 `src/bin/duckfn.rs` 都指向同一个
`src/extension/` 模块，这是整套目录结构唯一的规则 —— 为什么、以及破了这条规则会看到什么
`error[E0583]`，见[项目结构约定](./project-structure.md)。

## 用 duckfn、quack-rs 写逻辑

`duckfn` 是本仓库提供的那一层：它建立在 [`quack-rs`](https://github.com/tomtom215/quack-rs) 之上，
用属性宏把普通 Rust 函数变成扩展函数。日常写法就是给函数加一个属性：

```rust
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    Ok(v.map(|x| x * 2))
}
```

为什么不直接用官方的 `duckdb` crate？因为它的扩展 API 只覆盖两类函数 —— 标量函数走 `vscalar`
feature，表函数走 `vtab` feature，也就是这一行的全部能力：

```toml
duckdb = { version = "~1.10505.0", features = ["loadable-extension", "vscalar"] }
```

聚合函数、SQL 宏、replacement scan、类型转换、嵌套类型都没有对应的注册接口。`duckfn` 补上的正是
这部分：同一套属性宏同时覆盖标量函数、聚合函数、表函数、SQL 宏、replacement scan 与类型转换。

需要宏没有暴露的能力（手写 `LogicalType`、向量级操作、C API 的某个角落）时再落到 `quack-rs`
—— 它是对 DuckDB C API 覆盖最全、文档最完整的绑定；它本来就在你的依赖列表里。

[同样功能的两种写法](../examples/side-by-side.md) 把官方模板与 `quack-rs` 示例里的四个函数各写了两遍，
可以直观看到差别。

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

[`templates/Justfile`](https://github.com/shijianjs/duckfn/blob/main/templates/Justfile) 是给下游项目的
精简版：复制进新项目、把 `extension_name` 改成与 `duckfn_entrypoint!` 一致，之后日常就用
`just build`、`just sql "SELECT …"`、`just repl`、`just test`。

## 什么时候仍然需要官方流程

用 Cargo 构建足以覆盖开发，但有两件事仍然依赖 `make`：

- sqllogictest 套件（`make test`）—— 这是测试 DuckDB 扩展的标准方式，比 Rust 测试框架更贴合扩展行为。
- CI，跑的是同一套 makefile。

所以常见的搭配是：先让 `make configure` 跑通一次，迭代时用 Cargo 构建，推送前跑 `make test`。
在 Windows 上，这意味着 `make` 要在 Git Bash 里运行，而不是 PowerShell —— 见[贡献指南](../contributing.md#windows)。

## 让 AI 助手写代码

如果扩展交给 AI 助手来写，就在项目里放一份 `AGENTS.md`。本仓库提供了模板：
[`templates/AGENTS.md`](https://github.com/shijianjs/duckfn/blob/main/templates/AGENTS.md)。

它要解决的问题是「依赖能带过去什么、带不过去什么」。`cargo` 会把 `duckfn` 与 `duckfn-macro`
解包到本地 registry，因此运行时与宏的实现 —— 也就是「某个属性收哪些参数、允许哪些返回形状」的
真相来源 —— 就在磁盘上，可以直接读。但发布出去的包里只有 `src/` 和 `README.md`：`docs/` 下的
用户文档、`src/extension/` 下的示例扩展、`test/sql/` 下的 sqllogictest 套件，都不会跟着依赖进入
下游项目，也不会自动进入助手的上下文。

所以模板把知识源排了序：先本仓库的本地 clone（看文档、示例与测试），再本地 registry，
文档站只作兜底；并明确要求助手查不到时就停下来问，而不是凭印象编一个属性出来。

模板刻意拆成了两层：

- [`templates/AGENTS.md`](https://github.com/shijianjs/duckfn/blob/main/templates/AGENTS.md)
  是**项目层**：复制到新项目根目录命名为 `AGENTS.md`，只填两个值 —— 这个扩展做什么、
  duckfn 的 clone 在哪。凡是能从代码里读出来的（扩展名、crate 名、duckfn 版本）一律不抄进来，
  这样它就不会变成第二份会过期的真相。
- [`templates/duckfn-conventions.md`](https://github.com/shijianjs/duckfn/blob/main/templates/duckfn-conventions.md)
  是**共享层**：知识源、硬约束、开发循环、新增函数的流程。项目层的 `AGENTS.md` 只指向它、不复制它
  —— 这正是模板不会被用成一次性的原因：duckfn 升级时，clone 里 `git pull` 一下就同步了。

## 接下来

- [安装](./installation.md) —— 把 duckfn 加进 crate。
- [快速开始](./quick-start.md) —— 编写、构建并加载扩展。
- [构建与发布](../build-and-release.md) —— 模板的 CI 在打 tag 时做了什么。
