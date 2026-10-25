---
title: 构建与发布
sidebar_position: 2
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
just build                  # make configure && make debug（官方工具链）
just sql "SELECT double_it5(21);"          # 构建后 LOAD 并执行一条语句
just test                   # make configure debug test
just doc                    # cargo doc -p duckfn
```

`Makefile` 里有四个设置值得了解：

```make
EXTENSION_NAME=duckfn
USE_UNSTABLE_C_API=0
TARGET_DUCKDB_VERSION=v1.2.0
EXTENSION_FEATURES=quack,sql,chrono,uuid,rust_decimal
TARGET_INFO += --example $(EXTENSION_NAME) --features $(EXTENSION_FEATURES)
```

默认构建是**稳定版 C API** 构建，发行版也刻意保持这一档：`TARGET_DUCKDB_VERSION=v1.2.0` 是 **C API
下限**而不是发行版本号，所以一份产物覆盖 DuckDB 1.3.2 到 2.0.0。quack-rs 0.19 也支持把不稳定区用在 2.0
上 —— 办法是把产物 stamp 成 `C_STRUCT` + `v1.5.6` 下限，而不是 `C_STRUCT_UNSTABLE` —— 但发出去的产物
不需要这一档。两种 ABI 类型与 2.0 入口点的变化见
[DuckDB 版本兼容性](../duckdb-versions/index.md)。

`EXTENSION_NAME` 必须与 `test/extension/entry.rs` 里的 `duckfn_entrypoint!`、以及 sqllogictest 文件里的
`require` 保持一致。`TARGET_INFO` 里带 `--example $(EXTENSION_NAME)` 和 `--features $(EXTENSION_FEATURES)`：
扩展产物来自 `[[example]] duckfn` 这个 target，`EXTENSION_FEATURES` 则说明这个 target 要演示哪些能力。
`TARGET_INFO` 是 DuckDB 官方 makefile 唯一会原样拼进 `cargo build` 的变量 —— 少了它，产出的是一个没有
入口符号的 cdylib。

那份清单里的 `quack` 只是**标记**（决定示例树编不编），其余是能力清单 —— 因为示例树自己按能力分模块。
`duckdb-1-5` 与 `duck_vfs` 在不稳定区，是显式 opt-in 的。quack-rs 0.19 起它们按 `C_STRUCT` + `v1.5.6`
的 C API 下限发布（不是 `C_STRUCT_UNSTABLE` —— 2.0 会把那种产物送去一个 quack-rs 不生成的 v2 入口）。
对着当前 `Cargo.lock` 解析到的 2.0 头文件编，产物在 2.0.0 上可加载：

```bash
make USE_UNSTABLE_C_API=0 TARGET_DUCKDB_VERSION=v1.5.6 DUCKDB_TEST_VERSION=2.0.0 \
     EXTENSION_FEATURES=quack,duckdb-1-5,duck_vfs,sql,chrono,uuid,rust_decimal debug
```

要用一份产物同时覆盖 1.5.6 与 2.0.0，先把 `libduckdb-sys` 钉到 `1.10506.0`：那是 546 槽的 1.5.6
布局，2.0 接受它作为前缀；2.0 头文件带的是 548 槽，对着它编的产物在 1.5.6 上会被拒，而不是派发到错的
槽位。两套完整配置见[版本数字与 ABI 兼容性](../duckdb-versions/c-api-and-abi.md)。

稳定区构建会自动跑用例里的稳定子集（`scripts/stable_test_list.py`），所以 `make test` 不会去调用
不稳定档的示例函数。要把同一份产物拿给更旧的引擎跑：

```bash
just test_stable 1.4.0        # ……再拿 DuckDB 1.4 跑一遍指定范围的用例
```

`sql` 也归在这一档：它虽然跑在回调侧，但发出的每个调用都走 `duckdb_connect`、预处理 / 绑定 / 取块与
逻辑类型，全在稳定区（[在扩展里执行 SQL](../guide/sql-execution.md)）。CI 每个 PR 都拿同一个脚本对着
DuckDB 1.3.2、1.4.5、1.5.6、2.0.0 跑一遍（`.github/workflows/StableApiPipeline.yml`），所以那份可移植性
承诺是被验证的，而不是被假定的。

## WebAssembly

wasm 构建需要先装一次 Emscripten 目标：

```bash
rustup target add wasm32-unknown-emscripten
just build_wasm
```

由于最终链接由 `emcc` 完成，该目标要的是 `staticlib` 而不是 `cdylib`。`crate-type` 不能按 target
覆写，所以这两者都放在同一个 `[[example]] duckfn` target 上（根 `Cargo.toml` 里
`crate-type = ["cdylib", "staticlib"]`）：原生构建取 cdylib，wasm 构建取 `.a`，而 lib 本身只剩一个
`rlib`。两个扩展产物因此都出自 `test/extension/` 的同一次编译 —— 这点很关键：多编一份就会把每个
函数注册两次，而且在 wasm 上重复的入口符号会直接链接失败。剩下的交给 `make`：它用 `emcc` 把归档链成
side module，再补上扩展元数据。

lib 保持纯 rlib，还有一层**给下游项目**的好处：cargo 同样照依赖的 `crate-type` 办事，cdylib 一旦写在
lib 上，每个下游项目编 wasm 时都要把 duckfn 的 cdylib 也链一遍 —— 而那条路径下 rustc 不会给 emcc 传
`-sSIDE_MODULE=2`，emcc 于是按独立模块链接，报：

```
wasm-ld: error: libstandalonewasm.a(__main_void.o): undefined symbol: main
```

扩展项目在 duckfn ≤ 0.0.16 时要在自己的 `.cargo/config.toml` 里加
`[target.wasm32-unknown-emscripten] rustflags = ["-C", "link-arg=-sSIDE_MODULE=2"]` 绕开它；
duckfn 0.0.17 起 lib 只有 rlib，这段就不需要了。

## 持续集成

`.github/workflows/MainDistributionPipeline.yml` 把主要工作交给 DuckDB 的共享工作流：

```yaml
on:
  push:
    tags: ['v*.*.*']
  workflow_dispatch:

jobs:
  duckdb-stable-build:
    uses: duckdb/extension-ci-tools/.github/workflows/_extension_distribution.yml@v2.0-cyanoptera
    with:
      duckdb_version: v2.0.0
      ci_tools_version: v2.0-cyanoptera
      extension_name: duckfn
      extra_toolchains: rust;python3
      exclude_archs: 'linux_amd64_musl'
```

该共享工作流会为所有受支持平台（含 WebAssembly 目标）构建并测试扩展，本仓库只需要声明用哪个 DuckDB 版本、
需要哪些工具链。

### 稳定区矩阵

那条流水线打的始终是**全量** feature 清单，所以它覆盖不到稳定区那份产物 —— 也就是打算「装进一段版本
范围、而不是某一个引擎」的那份（见[版本号与 ABI](../duckdb-versions/c-api-and-abi.md)）。
`.github/workflows/StableApiPipeline.yml` 把这块补上：每个 PR、每个版本 tag 以及
手动触发，都用 `USE_UNSTABLE_C_API=0` + `TARGET_DUCKDB_VERSION=v1.2.0` + `EXTENSION_FEATURES=quack,sql`
打一份产物，再装进 DuckDB 1.3.2、1.4.5、1.5.6、2.0.0 跑一遍稳定区子集。最新的那个引擎是对照组 —— 它挂了说明
坏的是稳定区构建本身，而不是版本范围。每条 leg 就是一次 `scripts/test-stable.sh`（与 `just test_stable`
同一个入口）；运行器镜像钉在 `ubuntu-24.04`，因为 venv 里的 Python 决定能装哪个 `duckdb` wheel。

### 发布

第二个作业把推送的 tag 变成 GitHub Release：

1. 下载全部 `duckfn-*-extension-*` 产物。
2. 把 `*.duckdb_extension` 与 `*.duckdb_extension.wasm` 收敛为 `duckfn-<arch>.duckdb_extension`。
3. 用上一个 `v*` tag 以来的提交记录生成发布说明。
4. 创建 Release；若已存在则上传覆盖。

因此发版流程就是：打 `vX.Y.Z` tag、推送、等三个作业跑完 —— 产物、GitHub Release，以及下面的 crate。

## 发布 crate

同一个 tag 也会把两个库 crate 发到 crates.io，作为那条流水线的第三个作业。它的触发条件比上面两个
作业更严：只认版本 tag 的 **push**，手动 `workflow_dispatch`（即使在 `main` 上）不构建、不发版、
更不会发 crate —— crates.io 上的版本删不掉、只能 yank，这道门值得守住。

鉴权用 [crates.io 可信发布](https://crates.io/docs/trusted-publishing)：由
`rust-lang/crates-io-auth-action` 拿 workflow 的 OIDC token 换一个 30 分钟有效的发布 token，
仓库里不存长期有效的 `CARGO_REGISTRY_TOKEN`。

顺序很重要 —— `duckfn` 依赖 `duckfn-macro = "={{DUCKFN_VERSION}}"`，而发布时这个依赖是从 registry
解析的（不看本地 path），所以作业先发 `duckfn-macro`，等它在 sparse index 里可见，再发 `duckfn`。

CI 用不了时才走本地：

```bash
just release_publish   # publish_macro_dry → publish_macro → publish_dry → publish
```

版本来自 workspace：

```toml
[workspace.package]
version = "{{DUCKFN_VERSION}}"
rust-version = "1.88"
```

示例扩展随本包一起发布，而不是独立 crate，所以也不会单独上传。

而且发布出去的 `duckfn` 包并不只有运行时：根 `Cargo.toml` 的 `include` 会把示例扩展
（`test/extension/**`、`src/bin/duckfn.rs`）、它的 sqllogictest 用例
（`test/sql/**/*.test`）、文档站正文（`docs/README.md`、`docs/docs/**` 与 `docs/i18n/` 下的简体
中文译文）、`demo.sh`、README 与许可证一起打进去。所以解包即得完整文档**和**一份可以直接跑的示例 ——
这正是把它们放进同一个包的意义。确切清单用 `cargo package -p duckfn --list` 查看。

示例只在打开 `quack` feature 时才参与编译（例如 `cargo build --features quack,duck_vfs` —— 标记加上它要
演示的能力），所以它进包对任何依赖 `duckfn` 的下游项目都没有影响。

## 文档站

`docs/` 下的 Docusaurus 站点由 `.github/workflows/DeployDocs.yml` 部署：它在上面那条多平台构建
**跑完之后**触发，而不是在推 tag 的那一刻；也可以在 Actions 页面手动触发。普通提交不会构建它。
等流水线是必要的：部署出去的页面预加载的是**已发布**的 wasm 扩展，所以 Release 必须先存在 —— 在
推 tag 时部署会和构建抢跑，拉到的还是上一个 release。

它从 `actions/configure-pages` 读取 Pages 地址，用 `npm run build` 构建两种语言，然后发布产物。
本地相反：同一个站点用的是 `just build_wasm_eh` 产出的扩展，不碰 release（`just test_wasm` 会
构建它并跑一遍示例）。

`github-pages` 环境带保护规则，需要在
`Settings -> Environments -> github-pages -> Deployment branches and tags` 中列出允许部署的 ref ——
现在是分支 `main`（`workflow_run` 跑在默认分支上），加上手动触发时用的 `v*.*.*` tag。

## 接下来

- [贡献指南](./contributing.md) —— 日常开发流程。
- [快速开始](../getting-started/quick-start.md) —— 最小构建流程。
