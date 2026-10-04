---
title: DuckDB 版本兼容性
sidebar_position: 8
description: 头文件来自哪个 DuckDB、元数据里那一栏在两种 ABI 类型下各是什么意思、怎么让一份产物跨多个 DuckDB 发行版可用，以及为什么构建 wasm 产物是 Rust、emsdk/binaryen 与 ci-tools 钉版之间的另一套版本耦合。
---

# DuckDB 版本兼容性

duckfn 扩展从不链接 DuckDB：它走 C API 的函数表调用，而「这个文件收不收」是 DuckDB 在 `LOAD`
时自己判定的。这个判定里涉及三个不同的数字，而人们平常说的「DuckDB 版本」只是其中一个。你在每一处
怎么选，决定了编出来的一份产物能服务一整段 DuckDB 发行版，还是只能钉在其中一个上。

## 三个数字

| 数字 | 在哪里设置 | 含义 |
| --- | --- | --- |
| **头文件来自哪个发行版** | `Cargo.toml` 里的 `libduckdb-sys`（版本落在 `Cargo.lock`） | 编译时对着哪份 `duckdb_extension.h` —— 也就是你的代码**可以调用**的 API 面。 |
| **写进元数据的那一栏** | `Makefile` 的 `TARGET_DUCKDB_VERSION`，由 `append_extension_metadata.py -dv` 写入 | `abi_type = C_STRUCT` 下按 **C API 版本**解释，`C_STRUCT_UNSTABLE` 下按**发行版本号**解释。 |
| **加载这个文件的引擎** | 用户的 DuckDB | 拿它自己的 C API 层级（或发行版）去校验你的声明。 |

workflow 里的 `duckdb_version`（以及随它导出的 `DUCKDB_VERSION`）是第四个、互不相干的旋钮：它决定
分发流水线签出哪份 DuckDB 源码、sqllogictest 跑在哪个引擎上、产物怎么命名。它**不会**进到你的扩展里。

## `C_STRUCT` 与 `C_STRUCT_UNSTABLE`

`Makefile` 里的 `USE_UNSTABLE_C_API` 决定元数据声明的是哪一种 ABI 类型，而**正因为它**，同一个
`-dv` 值才有两种意思。

**`USE_UNSTABLE_C_API=0` → `abi_type = C_STRUCT`。** 声明的是**下限**，加载器比较的是 C API 版本：

```text
The file was built for DuckDB C API version '<declared>', but we can only load extensions built for
DuckDB C API '<engine>' and lower.
```

引擎的 C API 不低于你的下限就收，具体是哪个发行版无所谓。这正是想要的可移植性，代价只是代码必须待在
C API 的稳定区里。

**`USE_UNSTABLE_C_API=1` → `abi_type = C_STRUCT_UNSTABLE`。** 这时声明的是**发行版本号**，引擎要求
逐字相等。不稳定那部分是按槽位顺序排布的 C 结构体，对着另一套槽位编出来的产物根本没法跑：一份产物，
一个引擎。

`append_extension_metadata.py` 自己的帮助文本已经把这层区分写明了：*"The DuckDB version to encode,
depending on the ABI type this encodes the duckdb version or the C API version."*

## 这两个值分别从哪来

两处设置分别落在两个文件里 —— 头文件来自 `Cargo.toml`，声明来自 `Makefile`：

```toml
# Cargo.toml —— 头文件
libduckdb-sys = { version = ">=1.10500, <2", features = ["loadable-extension"] }
```

`libduckdb-sys` 把 DuckDB 版本编码成 `1.<major*10000 + minor*100 + patch>.0`，所以 `1.10506.0` 就是
DuckDB 1.5.6，上面那句 `>=1.10500` 的意思是「DuckDB 1.5 及以上的头文件」。

没有别的什么东西会改写 `Makefile` 里这一行：ci-tools 的 `set_duckdb_version` 对 C API 扩展是
no-op，社区仓的 `duckdb_version` 只决定签出哪份 DuckDB 源码、产物怎么命名、deploy 到哪个版本目录。
它由你自己维护 —— 见[构建与发版](./development/build-and-release.md)。下一节把两种模式各自完整的配置
都列出来。

## 自己的扩展该怎么选

一个问题就能定：**你的代码碰不碰不稳定区？**

- 不稳定区里有 COPY 函数、宿主文件系统（`duckfn::duck_vfs`）、标量函数的 `bind` / `init` 槽位、
  `varargs`，以及 DuckDB 1.5 才有的逻辑类型（目前是 `TIME_NS`）。duckfn 把它们统一收在 `duckdb-1-5`
  feature 后面；`owned-connection`（宿主 VFS）会连带打开它。
- 其余都在稳定区：标量、聚合、表函数、cast、替换扫描、SQL 宏、具名 STRUCT / ENUM 类型，以及
  chrono / uuid / rust_decimal 三个桥。

### 稳定区：完整配置

```make
# Makefile
EXTENSION_NAME=my_extension
USE_UNSTABLE_C_API=0
TARGET_DUCKDB_VERSION=v1.2.0
```

```toml
# Cargo.toml —— ABI 上的关键是 `duckdb-1-5` 保持关闭（`cli` 只服务
# `function_descriptions` 那个 bin，与 ABI 无关）。
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["cli"] }
```

```yaml
# .github/workflows/MainDistributionPipeline.yml
      duckdb_version: v1.5.6     # 任何 C API 不低于下限的引擎都行；取最新的是稳妥选择
```

- `TARGET_DUCKDB_VERSION` 是 **C API 下限**、不是发行版本号 —— 这正是这一档换来的东西。`v1.2.0` 是
  DuckDB 1.3.2 到 1.5.5 共同停留的层级，一份产物通吃（实测见下）。
- **这里刻意不写 `export QUACK_RS_TARGET_DUCKDB_VERSION`，写了也不起作用。** quack-rs 只在不稳定那条
  路上读它：只要 `uses_unstable_api()`（即 `cfg!(feature = "duckdb-1-5")`）为假，`abi::check()` 连
  `built_against_version()` 都不会调，直接返回 `StableOnly`。
- `DUCKDB_TEST_VERSION` 同理不需要：测试运行器可以是任何更新的引擎，因为凡是 C API 不低于下限的引擎
  都会收下这份产物。

### 不稳定区：完整配置

```make
# Makefile
EXTENSION_NAME=my_extension
USE_UNSTABLE_C_API=1
TARGET_DUCKDB_VERSION=v1.5.6
export QUACK_RS_TARGET_DUCKDB_VERSION=$(TARGET_DUCKDB_VERSION)
DUCKDB_TEST_VERSION := $(patsubst v%,%,$(TARGET_DUCKDB_VERSION))
```

```toml
# Cargo.toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["duckdb-1-5"] }   # 要用 duckfn::duck_vfs 再加 "owned-connection"
```

```yaml
# .github/workflows/MainDistributionPipeline.yml
      duckdb_version: v1.5.6     # 必须与 TARGET_DUCKDB_VERSION 相等；它就是 `make test` 加载的引擎
```

- **`export` 是这行的一部分，不是装饰。** quack-rs 的构建脚本是从**环境变量**里读
  `QUACK_RS_TARGET_DUCKDB_VERSION` 的，而 `make` 不 `export` 就不会把变量放进环境。漏掉 `export`
  的结果不是报错，是 cargo 什么都看不到、值空着 —— 布局检查于是退回 quack-rs 自带的表，而那正好就是
  「引擎是个 quack-rs 还没收录的发行版」时会拒绝你的那条路。
- 这一档里 `TARGET_DUCKDB_VERSION` 是**发行版本号**，引擎必须与它逐字相等 —— 所以测试运行器的钉版
  （`DUCKDB_TEST_VERSION`）和 CI 的钉版要跟着一起动。
- 要退回稳定区，这三行带版本味的配置要一起删：`export`、`DUCKDB_TEST_VERSION` 的派生，以及 CI 那条
  「必须相等」的要求。

两种模式都躲不掉的一个坑：

- **别在声明 `C_STRUCT` 的同时打开 `duckdb-1-5`。** 加载器只校验你声明了什么，于是不稳定区槽位对不上的
  引擎照样会收下这个文件 —— 而对不上暴露出来的不是加载错误，是未定义行为。两边选一边，别跨着站。

### 稳定区的一份产物到底覆盖到哪

`v1.2.0` 是 DuckDB 1.3.2 到 1.5.5 共同停留的 C API 层级，所以一份声明它的稳定区产物在这一整段里都能
被收下。下面是用**同一份产物**实测的，每个引擎都加载成功并跑通了一次真实调用：

| 声明的 C API 下限 | 收下同一个二进制的引擎 |
| --- | --- |
| `v1.2.0` | 1.3.2、1.4.0、1.4.5、1.5.0、1.5.5、1.5.6、2.0.0（预发布） |

DuckDB 2.0 已经很近了（见它的 release calendar），而一份声明 `v1.2.0` 的稳定区产物在 2.0 上已经能加载并
跑通 —— 把这样的扩展装进一个 2.0 预发布、调用它的函数（聚合、标量都返回了预期值）实测过。买到这份可
移植性的是那个「下限」：别把「能跑在 2.0 上」当成「是对着 2.0 编的」。

关于 2.0 有个命名坑：DuckDB 新出了**第二套 v2 C API**（`duckdb_v2.h`）—— 一套真正不同、仍在快速演进的
接口，由实验性的 `duckdb-neo` wrapper 使用。它**并不取代扩展所走的 v1 API**：可加载扩展构建在 v1 C API 上
（`libduckdb-sys` 默认的 `capi-v1` feature，`loadable-extension` 必需），而 2.0 照样加载的正是这套 v1。所以
「v2 C API 不一样」讲的是**那个新头文件**，不是 v1 扩展失去兼容。版本编码也要留意：DuckDB 2.0.0 映射成
`libduckdb-sys` 的 crate 版本 `1.20000.x`（格式 `1.<major*10000 + minor*100 + patch>.x`）—— 仍是 `1.x`，
所以 `>=1.10500, <2` 的约束会选中它。`<2` 不是拦路虎；把发布线切到 2.0 的真正闸门是它是否**正式发布**
（有稳定的 `v2.0-cyanoptera` ci-tools ref 与真实的 `duckdb-shared-libs` 资产），不是某个 crate 版本上限。

只有确实需要更新的头文件时才抬高这个下限。抬的时候填的是 **C API 版本**、不是发行版本号：在那里写
`v1.5.6` 就只剩那一个引擎肯收了。

## 自己验证一个构建

`make debug` 会把刚写进产物的元数据打出来，这是看清「你实际声明了什么」而不是「你想声明什么」最快的
办法：

```text
FIELD2 = windows_amd64
FIELD3 = v1.2.0          # 声明的版本
FIELD5 = C_STRUCT        # ABI 类型
```

然后拿这**同一份**产物去喂几个引擎 —— 本地构建的产物一律要 `-unsigned`：

```shell
duckdb -unsigned -c "LOAD '<path>/my_extension.duckdb_extension'; SELECT my_greet('world');"
```

这里有两个常见的坑：

- **被钉住的测试运行器不等于被测的引擎。** `make configure` 只会造一次 `configure/venv`，之后不会刷新
  里面那个 Python `duckdb`（那条 recipe 挂在目录上，第二次 `make` 直接跳过），落后的运行器会拒绝刚构建
  出来的扩展。要在原地升级它 —— 下面这两条就是 `base.Makefile` 自己用的路径：

```shell
configure/venv/Scripts/python.exe -m pip install --upgrade "duckdb==1.5.6"   # Windows
configure/venv/bin/python3       -m pip install --upgrade "duckdb==1.5.6"   # Linux / macOS
```

- **头文件的钉版与引擎是两码事。** 更新 `libduckdb-sys` 改的是「你能调用什么」，不是「谁会加载你」；
  反过来也一样。

## WebAssembly

规则完全一样，只多一条：引擎是固定在 DuckDB-Wasm 构建里的，`LOAD` 时挑不了。所以不稳定区的产物必须由
带对应发行版的那个确切 dev 构建来服务，而稳定区的产物只要求引擎不低于它的下限。本站在哪里钉这个构建，
见[预加载扩展](./docs-kit/preloaded-extensions.md)。

wasm_* 产物怎么被**构建**出来是另一套版本故事 —— CI 装的 Rust、钉死的 emsdk/binaryen、以及它们来自哪个
ci-tools ref —— 见下一节。

## wasm 构建工具链：Rust ↔ emsdk ↔ ci-tools

上面讲的都是产物面向**哪个 DuckDB**。构建 `wasm_*` 产物是另一套版本问题，有三个必须对齐的旋钮，而坑在于
**它们各自定在不同仓库里**：

| 旋钮 | 谁定的 | 控制什么 |
| --- | --- | --- |
| wasm 构建安装的 **Rust 工具链** | 你调用的 ci-tools ref（`uses: .../_extension_distribution.yml@<ref>` 与 `ci_tools_version` 输入） | `cargo build` 收不收你的依赖树 |
| **emsdk / binaryen**（`3.1.71` → `wasm-opt` v120） | ci-tools 的 `setup-emsdk` 步 | `emcc` 的后链接优化器认识哪些 wasm 特性 |
| **链接旗标**（`-O3` vs `-O0`） | **你的 `Makefile`**（`link_wasm_release` / `link_wasm_debug`） | 那个优化器到底跑不跑 |

前两个由官方流水线钉死、基本动不了：emsdk 版本必须与 DuckDB-Wasm 构建里的引擎一致，否则产出的扩展
`LOAD` 不进来。

### 两个互相独立的失败点

**1 · 编译阶段（`cargo build`）——依赖的 MSRV 高于被钉的 Rust。**
可复用工作流为 wasm job 安装一个指定的 Rust。历史上它落后于 native job：native 用 1.97.1，而 wasm
的 "Build Wasm module" 步仍钉着 1.86.0。凡是依赖要求更新 rustc 的树，都会在 `cargo build` 就失败 ——

```text
error: rustc 1.86.0 is not supported by the following packages:
  <crate> requires rustc 1.89
```

—— 根本到不了 `emcc`。这就是 ci-tools 的 [issue #385](https://github.com/duckdb/extension-ci-tools/issues/385)，
由 [PR #394](https://github.com/duckdb/extension-ci-tools/pull/394) 修复（把 wasm 的 Rust 抬到 1.97.1、与 native 统一）。
可动的杠杆分两层：把 `ci_tools_version` / `@ref` 抬到含该修复的 ref（但这只作用于**你自己的发布流水线**）；
**或者**保持钉住的 ci-tools 不变、用**仓库根部的 `rust-toolchain.toml`**（`channel = "1.89.0"`，并在
`targets = [...]` 里带上 wasm target）从仓库侧覆盖被安装的工具链。`dtolnay/rust-toolchain` 那步只是设 rustup
**default**，而 `rust-toolchain.toml` 优先级更高，cargo 于是用上新版 Rust；且这文件随检出走，连社区注册表的
构建也一并修好（见下）。

**2 · 链接阶段（`wasm-opt`）——现代 Rust 产出的特性旧 binaryen 解析不了。**
较新的 Rust（`>=1.89`）为 `wasm32-unknown-emscripten` 编出的模块会声明一批 post-MVP 的 target-feature
——`exception-handling`、`bulk-memory-opt`，以及模块大到一定程度（函数类型多 → indirect-call 表超长）时的
`call-indirect-overlong`。`emcc` 读这些声明并转成 `--enable-*` 传给 `wasm-opt`。emsdk 3.1.71 带的是 binaryen
v120，认不全这些新名字，于是 `emcc -O3` 挂：

```text
Unknown option '--enable-bulk-memory-opt'
emcc: error: '...wasm-opt ... --enable-call-indirect-overlong ...' failed (returned 1)
```

`wasm-opt` 只是**优化器**；`wasm-ld` 的模块本身合法、在 DuckDB-Wasm 里加载正常（浏览器运行时支持这些提案，
只是 binaryen v120 不会改写它们）。所以在保留完整功能的前提下，出路是**跳过优化器**：把链接覆盖成 `-O0`，
写在 `include` **之后**以压过 `base.Makefile` —— 并且要和 base 一样套 `ifneq ($(DUCKDB_WASM_PLATFORM),)` 守卫，
否则连（空的）原生目标也被重定义，linux/macos/windows 构建会报 `emcc: command not found`：

```make
# Makefile，在 include extension-ci-tools/.../base.Makefile 之后
ifneq ($(DUCKDB_WASM_PLATFORM),)
link_wasm_release:
	emcc $(EXTENSION_BUILD_PATH)/release/$(EXTENSION_LIB_FILENAME) -o $(EXTENSION_BUILD_PATH)/release/$(EXTENSION_FILENAME_NO_METADATA) -O0 -sSIDE_MODULE=2 -sEXPORTED_FUNCTIONS="_$(EXTENSION_NAME)_init_c_api"
endif
```

等被钉的 emsdk 里 binaryen 认得了这些特性（即 DuckDB 抬高了 wasm 的 emsdk），就该删掉这个覆盖退回 `-O3`。

### CI 不读你的 `extension-ci-tools` 子模块

子模块只是**本地** `just build` / `make` / `just test_wasm` 用的 makefile 来源。GitHub CI 会**忽略**你子模块钉的
commit：它按 `uses: ...@<ref>` 取可复用工作流，又按 `ci_tools_version` 把 `extension-ci-tools` 重新 checkout 到
同一路径。所以你本地跑的那套 wasm 工具链可能和 CI（及注册表）用的不一致 —— 把本地子模块 ref、`@ref`、
`ci_tools_version` 三者对齐，否则「本地绿」不等于「CI 绿」。

### 社区注册表的构建用的是它自己的钉法

`duckdb/community-extensions/.github/workflows/build.yml` 以**它自己**的 ci-tools ref / `ci_tools_version` 默认值
调 `_extension_distribution.yml`，只额外传 `override_repository` + `override_ref`（你的仓库与提交 SHA）。所以你
项目里的 `ci_tools_version` **既传不到注册表、也左右不了它的 Rust 选择** —— 一旦注册表钉的 Rust 低于你的依赖
MSRV，它的 wasm job 会以同样的方式失败。抬 `ci_tools_version` 帮不上，但**仓库里的 `rust-toolchain.toml`**（见上）
**却能传到注册表**，因为它随 `override_ref` 一起被检出。于是项目侧的杠杆有二：用 `rust-toolchain.toml` 钉住 Rust，
或在 `description.yml` 里排除 wasm：

```yaml
extension:
  excluded_platforms: "wasm_mvp;wasm_eh;wasm_threads"
```

等注册表的 ci-tools 越过 #394 后再删掉这行。见[社区扩展文档页](./community-extension-docs.md)。

### 排查手法

- 逐字节对比两个 `.a` 里的特性字符串（`bulk-memory-opt`、`call-indirect-overlong`、`exception-handling`），
  看是哪份产物在要求更新的 binaryen。
- 确认每个目录真正在用的 Rust：`rustc --version`、`rustup override list`。
- CI 侧：`gh run view <id> --json jobs` 看各平台结论，`gh api .../actions/jobs/<job>` 看失败的**步骤名**
  （编译 vs 链接），整轮跑完后再 `gh run view --job=<id> --log` 取日志。

一个实例：`duckfn_statrs` 包的是 `statrs 0.19` / `nalgebra 0.35`，需要 rustc ≥ 1.89。在 `v1.5-variegata` 钉法下
wasm job 卡在 #385 的 1.86。最终采用的做法**保持 `v1.5-variegata` 不变**、从**仓库自身**覆盖 Rust —— 根目录
`rust-toolchain.toml`（`channel = "1.89.0"` + wasm target，优先级高于 dtolnay 设的 rustup default）**加上**上面的
`-O0` 覆盖（旧 binaryen → 跳过优化器）。已用 `workflow_dispatch` 实测全绿：native 与三个 wasm 都构建通过、
且没动任何 Release。（更早一版是借 `ci_tools_version: main` 拿 1.97.1；改用仓库内文件更佳，因为它能一并
传播到注册表。）wasm 产物未经优化（约 1.5 MB）。

## Release 资产 vs 社区仓

`LOAD '<release 资产的 url>'` 取回的正是你点名的那个文件，所以「一份产物通吃」完全是你那条声明的功劳。
社区仓走的是另一条路：它按**客户端上报的** DuckDB 版本发放对应的构建，目录树由 registry 自己填 —— 见
[社区扩展文档页](./community-extension-docs.md)。那条路因此是「按版本分派」，不是「跨版本容错」，无论你
声明了什么。

## 另见

- [安装](./getting-started/installation.md) —— duckfn 每个 feature 各自对 DuckDB 提什么要求。
- [文件系统](./guide/file-system.md) 与 [COPY 函数](./guide/copy-functions.md) —— 住在不稳定区的两项能力。
- [构建与发版](./development/build-and-release.md) —— 两条构建路径、发版流程，以及 CI 那几个旋钮与这里的
  关系。
- [已知问题](./known-issues.md) —— 与版本无关的上游 bug。