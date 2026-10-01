---
title: DuckDB 版本兼容性
sidebar_position: 8
description: 头文件来自哪个 DuckDB、元数据里那一栏在两种 ABI 类型下各是什么意思，以及怎么让一份产物跨多个 DuckDB 发行版可用。
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

两处设置分别落在两个文件里。下面这一组是**稳定区**的写法；duckfn 自己的示例扩展在另一边
（`USE_UNSTABLE_C_API=1`、`TARGET_DUCKDB_VERSION=v1.5.6`），因为它的 `quack` 示例真的用到了不稳定区。

```toml
# Cargo.toml —— 头文件
libduckdb-sys = { version = ">=1.10500, <2", features = ["loadable-extension"] }
```

```make
# Makefile —— 声明
USE_UNSTABLE_C_API=0
TARGET_DUCKDB_VERSION=v1.2.0
```

`libduckdb-sys` 把 DuckDB 版本编码成 `1.<major*10000 + minor*100 + patch>.0`，所以 `1.10506.0` 就是
DuckDB 1.5.6，上面那句 `>=1.10500` 的意思是「DuckDB 1.5 及以上的头文件」。

没有别的什么东西会改写 `TARGET_DUCKDB_VERSION`：ci-tools 的 `set_duckdb_version` 对 C API 扩展是
no-op，社区仓的 `duckdb_version` 只决定签出哪份 DuckDB 源码、产物怎么命名、deploy 到哪个版本目录。
这一行由你自己维护 —— 见[构建与发版](./development/build-and-release.md)。

## 自己的扩展该怎么选

一个问题就能定：**你的代码碰不碰不稳定区？**

- 不稳定区里有 COPY 函数、宿主文件系统（`duckfn::duck_vfs`）、标量函数的 `bind` / `init` 槽位、
  `varargs`，以及 DuckDB 1.5 才有的逻辑类型（目前是 `TIME_NS`）。duckfn 把它们统一收在 `duckdb-1-5`
  feature 后面；`owned-connection`（宿主 VFS）会连带打开它。
- 其余都在稳定区：标量、聚合、表函数、cast、替换扫描、SQL 宏、具名 STRUCT / ENUM 类型，以及
  chrono / uuid / rust_decimal 三个桥。

| 你的代码 | `USE_UNSTABLE_C_API` | `TARGET_DUCKDB_VERSION` | 得到什么 |
| --- | --- | --- | --- |
| 只用稳定区 | `0` | 你需要的那个 C API 版本 —— 今天是 `v1.2.0` | 一份产物，通吃所有 C API 不低于它的引擎 |
| 用到不稳定区的任何一项 | `1` | 精确的发行版本号，例如 `v1.5.6` | 一份产物，只在那一个引擎上可用 |

动手前值得知道的三件事：

- **别在声明 `C_STRUCT` 的同时打开 `duckdb-1-5`。** 加载器只校验你声明了什么，于是不稳定区槽位对不上的
  引擎照样会收下这个文件 —— 而对不上暴露出来的不是加载错误，是未定义行为。两边选一边，别跨着站。
- **不稳定 API 关着的时候，ABI 检查根本不会触发。** quack-rs 用的是同一个开关
  （`abi::uses_unstable_api()` 就是 `cfg!(feature = "duckdb-1-5")`），关掉时它什么都还没问就返回
  `StableOnly`，所以不稳定那条路需要的 `QUACK_RS_TARGET_DUCKDB_VERSION` 导出在这里是多余的。反过来，
  不稳定 API **开着**的时候就不能省 —— 翻回去时记得把它补上。
- **翻开关时这两个数字要一起动。** 改回 `USE_UNSTABLE_C_API=1` 就意味着 `TARGET_DUCKDB_VERSION` 从
  C API 下限变成发行版本号（而且必须与引擎相等）。

### 稳定区的一份产物到底覆盖到哪

`v1.2.0` 是 DuckDB 1.3.2 到 1.5.5 共同停留的 C API 层级，所以一份声明它的稳定区产物在这一整段里都能
被收下。下面是用**同一份产物**实测的，每个引擎都加载成功并跑通了一次真实调用：

| 声明的 C API 下限 | 收下同一个二进制的引擎 |
| --- | --- |
| `v1.2.0` | 1.3.2、1.4.0、1.4.5、1.5.0、1.5.5、1.5.6 |

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

- **被钉住的测试运行器不等于被测的引擎。** `configure/venv` 是一次性的目录戳，`make` 之后不会再刷新里面
  那个 Python `duckdb`，落后的运行器会拒绝刚构建出来的扩展。挪过钉版之后要在原地升级它。
- **头文件的钉版与引擎是两码事。** 更新 `libduckdb-sys` 改的是「你能调用什么」，不是「谁会加载你」；
  反过来也一样。

## WebAssembly

规则完全一样，只多一条：引擎是固定在 DuckDB-Wasm 构建里的，`LOAD` 时挑不了。所以不稳定区的产物必须由
带对应发行版的那个确切 dev 构建来服务，而稳定区的产物只要求引擎不低于它的下限。本站在哪里钉这个构建，
见[预加载扩展](./docs-kit/preloaded-extensions.md)。

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