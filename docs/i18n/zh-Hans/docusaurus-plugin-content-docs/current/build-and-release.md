---
title: 构建与发布
sidebar_position: 6
description: 本地构建与测试命令、WebAssembly 目标，以及 DuckDB 官方流水线如何把版本 tag 变成已发布的二进制。
---

# 构建与发布

## 本地构建

`Makefile` 引入了 DuckDB 官方的 `extension-ci-tools` makefile，因此命令与所有 DuckDB 扩展一致：

| 命令 | 作用 |
| --- | --- |
| `make configure` | 创建 Python venv，记录目标平台与扩展版本。需要 Python 3 与网络，只需执行一次。 |
| `make debug` | 构建 `cdylib`、追加扩展元数据，并把产物复制到 `build/debug/extension/<name>/<name>.duckdb_extension`。 |
| `make release` | 带优化的同一套流程。 |
| `make test` | 用 debug 构建跑 sqllogictest 套件。 |
| `make clean` / `make clean_all` | 清理构建产物；`clean_all` 会连 `configure/` 一起清掉。 |

常用组合在 `Justfile` 里已经封装好：

```bash
just build                  # cargo duckdb-ext build
just sql "SELECT double_it5(21);"          # 构建后 LOAD 并执行一条语句
just test                   # make configure debug test
just doc                    # cargo doc -p duckfn
```

`Makefile` 里有两个设置值得了解：

```make
EXTENSION_NAME=duckfn_quack
USE_UNSTABLE_C_API=1
TARGET_DUCKDB_VERSION=v1.5.5
```

`USE_UNSTABLE_C_API=1` 决定了产出的扩展只能在兼容版本的 DuckDB 里、并加 `-unsigned` 才能加载。
`TARGET_DUCKDB_VERSION` 指明写入元数据时针对的版本。`EXTENSION_NAME` 必须与
`duckfn-quack/src/extension/mod.rs` 里的 `duckfn_entrypoint!`、以及 sqllogictest 文件里的
`require` 保持一致。

## WebAssembly

wasm 构建需要先装一次 Emscripten 目标：

```bash
rustup target add wasm32-unknown-emscripten
just build_wasm
```

由于最终链接由 `emcc` 完成，该目标下 crate 类型必须是 `staticlib` 而不是 `cdylib`。示例 crate 在
`duckfn-quack/Cargo.toml` 里用一个额外的 `[[example]]` 目标解决这个问题：它指向
`duckfn-quack/src/wasm_lib.rs` 并声明 `crate-type = ["staticlib"]`。那个文件自己就是一个 crate root：
和 `duckfn-quack/src/lib.rs`、CLI 的 `duckfn-quack/src/bin/duckfn.rs` 一样，它声明 `mod extension;`，
三者编译的是同一棵 `duckfn-quack/src/extension/` 树 —— 见[项目结构约定](./getting-started/project-structure.md)。

## 持续集成

`.github/workflows/MainDistributionPipeline.yml` 把主要工作交给 DuckDB 的共享工作流：

```yaml
on:
  push:
    tags: ['v*.*.*']
  workflow_dispatch:

jobs:
  duckdb-stable-build:
    uses: duckdb/extension-ci-tools/.github/workflows/_extension_distribution.yml@v1.5-variegata
    with:
      duckdb_version: v1.5.5
      ci_tools_version: v1.5-variegata
      extension_name: duckfn_quack
      extra_toolchains: rust;python3
      exclude_archs: 'linux_amd64_musl'
```

该共享工作流会为所有受支持平台（含 WebAssembly 目标）构建并测试扩展，本仓库只需要声明用哪个 DuckDB 版本、
需要哪些工具链。

### 发布

第二个作业把推送的 tag 变成 GitHub Release：

1. 下载全部 `duckfn_quack-*-extension-*` 产物。
2. 把 `*.duckdb_extension` 与 `*.duckdb_extension.wasm` 收敛为 `duckfn_quack-<arch>.duckdb_extension`。
3. 用上一个 `v*` tag 以来的提交记录生成发布说明。
4. 创建 Release；若已存在则上传覆盖。

因此发版流程就是：打 `vX.Y.Z` tag、推送、等两个作业跑完。

## 发布 crate

两个库 crate 按依赖顺序发布 —— `duckfn` 依赖 `duckfn-macro = "={{DUCKFN_VERSION}}"`，所以宏 crate 必须先上 crates.io：

```bash
just publish_dry   # 先 cargo publish -p duckfn-macro --dry-run，再 -p duckfn
just publish
```

版本来自 workspace：

```toml
[workspace.package]
version = "{{DUCKFN_VERSION}}"
rust-version = "1.86"
```

仓库里的示例扩展（`duckfn-quack/`）是 `publish = false`，永远不会作为独立 crate 上传。

不过发布出去的 `duckfn` 包并不只有运行时：根 `Cargo.toml` 的 `include` 会把文档站正文
（`docs/README.md`、`docs/docs/**` 与 `docs/i18n/` 下的简体中文译文）一起打进去，所以在 crates.io
上拿到的包带着完整文档 —— 包括那页可以照着跑的示例页。确切清单用 `cargo package -p duckfn --list`
查看。

包永远带不过去的是示例扩展本身：cargo 会跳过任何含 `Cargo.toml` 的子目录，`include` 模式也覆盖不了
这条规则，所以 `duckfn-quack/`（连同它的源码与用例）只能留在仓库里。

## 文档站

`docs/` 下的 Docusaurus 站点由 `.github/workflows/DeployDocs.yml` 在推送 `v*.*.*` tag 时部署
（和扩展构建用的是同一批 tag），也可以在 Actions 页面手动触发；普通提交不会构建它。
它从 `actions/configure-pages` 读取 Pages 地址，用 `npm run build` 构建两种语言，然后发布产物。

`github-pages` 环境带保护规则，需要在
`Settings -> Environments -> github-pages -> Deployment branches and tags` 中列出 `v*.*.*`
这个 tag 模式，部署才会被接受。

## 接下来

- [贡献指南](./contributing.md) —— 日常开发流程。
- [快速开始](./getting-started/quick-start.md) —— 最小构建流程。
