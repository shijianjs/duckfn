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
just build                  # cargo duckdb-ext build -- --features quack
just sql "SELECT double_it5(21);"          # 构建后 LOAD 并执行一条语句
just test                   # make configure debug test
just doc                    # cargo doc -p duckfn
```

`Makefile` 里有四个设置值得了解：

```make
EXTENSION_NAME=duckfn
USE_UNSTABLE_C_API=1
TARGET_DUCKDB_VERSION=v1.5.5
TARGET_INFO += --features quack
```

`USE_UNSTABLE_C_API=1` 决定了产出的扩展只能在兼容版本的 DuckDB 里、并加 `-unsigned` 才能加载。
`TARGET_DUCKDB_VERSION` 指明写入元数据时针对的版本。`EXTENSION_NAME` 必须与
`src/extension/entry.rs` 里的 `duckfn_entrypoint!`、以及 sqllogictest 文件里的 `require` 保持一致。
`TARGET_INFO += --features quack` 决定示例会不会被编译：它挂在默认关闭的 `quack` feature 上，而
`TARGET_INFO` 是 DuckDB 官方 makefile 唯一会原样拼进 `cargo build` 的变量。少了这一行，产出的是一个
没有入口符号的 cdylib。

## WebAssembly

wasm 构建需要先装一次 Emscripten 目标：

```bash
rustup target add wasm32-unknown-emscripten
just build_wasm
```

由于最终链接由 `emcc` 完成，该目标需要的是 `staticlib` 而不是 `cdylib`。`crate-type` 不能按 target
覆写，于是本仓库的 lib 干脆把它被当成的东西都列上（`["rlib", "cdylib", "staticlib"]`），wasm 那边取
其中的 `.a` 用。两个扩展产物因此都出自 `src/extension/` 的同一次编译 —— 这点很关键：多编一份就会把
每个函数注册两次，而且在 wasm 上重复的入口符号会直接链接失败。剩下的交给 `make`：它用 `emcc` 把归档链
成 side module，再补上扩展元数据。扩展项目不需要这些 —— 那边 wasm 目标是单独一个 `[[example]]`
root（见[项目结构约定](./getting-started/project-structure.md)）；本仓库为什么并进 lib，见
[贡献指南](./contributing.md)。

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
      extension_name: duckfn
      extra_toolchains: rust;python3
      exclude_archs: 'linux_amd64_musl'
```

该共享工作流会为所有受支持平台（含 WebAssembly 目标）构建并测试扩展，本仓库只需要声明用哪个 DuckDB 版本、
需要哪些工具链。

### 发布

第二个作业把推送的 tag 变成 GitHub Release：

1. 下载全部 `duckfn-*-extension-*` 产物。
2. 把 `*.duckdb_extension` 与 `*.duckdb_extension.wasm` 收敛为 `duckfn-<arch>.duckdb_extension`。
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

示例扩展随本包一起发布，而不是独立 crate，所以也不会单独上传。

而且发布出去的 `duckfn` 包并不只有运行时：根 `Cargo.toml` 的 `include` 会把示例扩展
（`src/extension/**`、`src/bin/duckfn.rs`）、它的 sqllogictest 用例
（`test/sql/**/*.test`）、文档站正文（`docs/README.md`、`docs/docs/**` 与 `docs/i18n/` 下的简体
中文译文）、`demo.sh`、README 与许可证一起打进去。所以解包即得完整文档**和**一份可以直接跑的示例 ——
这正是把它们放进同一个包的意义。确切清单用 `cargo package -p duckfn --list` 查看。

示例只在打开 `quack` feature 时才参与编译（`cargo build --features quack`），所以它进包对任何依赖
`duckfn` 的下游项目都没有影响。

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
