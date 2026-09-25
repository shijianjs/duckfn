---
title: 创建项目
sidebar_position: 1
description: 从 duckfn 扩展模板或 DuckDB 官方 Rust 扩展模板起步，用 duckfn、quack-rs 写逻辑，用 cargo-duckdb-ext-tools 构建。
---

# 创建项目

动手写代码之前有三个决定：以什么为起点、用什么写、怎么构建。

## 从模板起步

下面两份模板是同一套骨架 —— DuckDB 的 CI、`extension-ci-tools`、`cdylib` crate root、sqllogictest
目录。差别只在于「为 duckfn 接好了多少」。

### duckfn 模板

[`shijianjs/duckfn-extension-template`](https://github.com/shijianjs/duckfn-extension-template)
是本项目自己的模板：官方骨架，但 duckfn 相关的固定动作已经做完了。

```shell
git clone https://github.com/shijianjs/duckfn-extension-template my_ext
cd my_ext
rm -rf .git && git init    # 可选：丢掉模板的历史，从头开始自己的仓库
just rename my_ext
```

`just rename`（即 `scripts/rename.sh`）就是这份模板存在的理由。扩展名必须在好几处同时一致，漏掉一处
不是编译错误，而是一个加载不起来的扩展 —— DuckDB 按文件名推出要查找的入口点符号，对不上只会让 `LOAD`
失败，不会给出有用的提示。脚本一次改齐这些地方：

- `Cargo.toml` 里的 `[package] name` 与 `[[example]] name`；
- `Makefile` 里的 `EXTENSION_NAME`；
- 传给 `duckfn_entrypoint!(…)` 的名字，它会成为入口点符号 `my_ext_init_c_api`；
- `Justfile` 与 CI 工作流里的 `extension_name`；
- README 与文档站里的路径示例，以及 `Cargo.lock` 里那一条。

脚本末尾会打印剩下需要人工过一遍的事情，主要就是把两个示例函数换成自己的 API。其余部分都已经在了：
依赖列表里带着 `duckfn` 且开了 `loadable-extension`、`src/lib.rs` 与 `src/wasm_lib.rs` 声明同一组
`mod`、一个标量示例函数与一个聚合示例函数（各配 sqllogictest 文件）、`Justfile`、`docs/` 下的中英双语
Docusaurus 站点（可以删，仓库里没有别的东西依赖它）、发版脚本，以及 `community-extension/` 下注册社区
扩展要提交的文件。开发循环写在它的 `README.md` 与 `DEVELOPMENT.md` 里，它的 `AGENTS.md` 也就是
[让 AI 助手写代码](#让-ai-助手写代码)那一节说的那一份。

### 官方模板

[`duckdb/extension-template-rs`](https://github.com/duckdb/extension-template-rs) 是 DuckDB 官方的
Rust 扩展模板，也是上面两份模板的共同底座。想自己接线就从这里起步；但无论从哪份模板起步，都值得知道它
的存在 —— makefile 与 CI 最终都来自这里：

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

然后改掉模板里写死的部分 —— `Makefile` 里的 `EXTENSION_NAME`、要构建 WebAssembly 时的 `[[example]]`
目标、传给 `duckfn_entrypoint!` 的名字 —— 这正是 `just rename` 自动化的那张清单，另外还有它当时还不
知道的 `Justfile` 与 CI 条目。

无论从哪份模板起步，都别动这几个 crate root：`src/lib.rs`、`src/wasm_lib.rs` 与 `src/bin/duckfn.rs`
都指向同一个 `src/extension/` 模块，这是整套目录结构唯一的规则 —— 为什么、以及破了这条规则会看到什么
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

[duckfn 模板](#duckfn-模板)用一份
[`Justfile`](https://github.com/shijianjs/duckfn-extension-template/blob/main/Justfile) 把两种流程都
包了一层：`just build`、`just sql "SELECT …"`、`just repl`、`just test`、`just ci-release`，以及文档站
与发版相关的 recipe。`just rename` 已经把 `extension_name` 改成与 `duckfn_entrypoint!` 一致。

## 什么时候仍然需要官方流程

用 Cargo 构建足以覆盖开发，但有两件事仍然依赖 `make`：

- sqllogictest 套件（`make test`）—— 这是测试 DuckDB 扩展的标准方式，比 Rust 测试框架更贴合扩展行为。
- CI，跑的是同一套 makefile。

所以常见的搭配是：先让 `make configure` 跑通一次，迭代时用 Cargo 构建，推送前跑 `make test`。
在 Windows 上，这意味着 `make` 要在 Git Bash 里运行，而不是 PowerShell —— 见[贡献指南](../contributing.md#windows)。

## 让 AI 助手写代码

如果扩展交给 AI 助手来写，就在项目里放一份 `AGENTS.md`。[duckfn 模板](#duckfn-模板)里已经有一份：
把两个占位符填上 —— 这个扩展做什么、duckfn 的 clone 在哪 —— 这一步就结束了。从别处起步的话，把那份
文件复制过去：[`AGENTS.md`](https://github.com/shijianjs/duckfn-extension-template/blob/main/AGENTS.md)。

它要解决的问题是「依赖能带过去什么、带不过去什么」。`cargo` 会把 `duckfn` 与 `duckfn-macro`
解包到本地 registry，因此运行时与宏的实现 —— 也就是「某个属性收哪些参数、允许哪些返回形状」的
真相来源 —— 就在磁盘上，可以直接读。但发布出去的包里只有 `src/` 和 `README.md`：`docs/` 下的
用户文档、`src/extension/` 下的示例扩展、`test/sql/` 下的 sqllogictest 套件，都不会跟着依赖进入
下游项目，也不会自动进入助手的上下文。

所以它把知识源指向本仓库的本地 clone，并明确要求助手查不到时就停下来问，而不是凭印象编一个属性出来：
文档、示例扩展与 sqllogictest 都只在那份 clone 里，两个占位符里有一个是它的路径，原因就在这里。

凡是能从代码里读出来的（扩展名、crate 名、duckfn 版本）一律不抄进去，这样它就不会变成第二份会过期的
真相。

## 接下来

- [安装](./installation.md) —— 把 duckfn 加进 crate。
- [快速开始](./quick-start.md) —— 编写、构建并加载扩展。
- [构建与发布](../build-and-release.md) —— 模板的 CI 在打 tag 时做了什么。
