---
title: 贡献指南
sidebar_position: 7
description: 环境准备、日常命令、测试组织方式，以及需要遵守的约定。
---

# 贡献指南

## 环境准备

| | |
| --- | --- |
| Rust | 1.86 及以上 —— workspace 设置 `rust-version = "1.86"`，使用 edition 2024。 |
| Python 3 + 网络 | 仅 `make configure` 需要，用于创建 sqllogictest 运行器的虚拟环境。 |
| `make` | 驱动 DuckDB 的 `extension-ci-tools` makefile。 |
| [`just`](https://github.com/casey/just) *（可选）* | `Justfile` 封装了常用命令。 |
| [`cargo-duckdb-ext-tools`](https://github.com/redraiment/cargo-duckdb-ext-tools) *（可选）* | `cargo install cargo-duckdb-ext-tools` 后可用 `cargo duckdb-ext build`，既不需要 `make` 也不需要 checkout submodule。 |
| DuckDB CLI | 手动加载扩展时使用，调试时也用得上。 |

`extension-ci-tools/` 是一个 git submodule，而 `Makefile` 会 include 它的 makefile，因此新克隆之后需要：

```bash
git submodule update --init --recursive
make configure
```

## Windows

`make` 要在 **Git Bash** 里运行，而不是 PowerShell 或 `cmd`：makefile 及其辅助脚本假定存在 POSIX shell。
`make` 报缺的包大多可以用 [Scoop](https://scoop.sh/) 安装：

```shell
scoop install make python
```

Cargo 与 `cargo duckdb-ext build` 在任何 shell 下都能用，所以只有 `make` 那几条目标（`make configure`、
`make test` 以及 CI 等价命令）需要 Git Bash。

## workspace 结构

| 成员 | 是否发布 | 说明 |
| --- | --- | --- |
| `/`（`duckfn`） | 是 | 运行时框架、示例扩展，同时是 workspace 根。 |
| `duckfn-macro/` | 是 | 过程宏；不依赖运行时，只依赖 `darling`、`syn`、`quote`。 |
| `src/extension/`、`test/sql/` | 随包发布，但不编译 | 示例扩展（`duckfn`）与它的 sqllogictest 用例：属于 `duckfn` 包，由 `quack` feature 打开。 |

根清单锁定 `duckfn-macro = "={{DUCKFN_VERSION}}"`，因此两个 crate 总是一起发布。

扩展项目有三个 crate root —— 见[项目结构约定](./getting-started/project-structure.md) —— 而本仓库只有
两个：没有单独的 wasm root，因为示例扩展就在本包里，编到 WebAssembly 的那份也是本包的 lib。

### 示例为什么在本包里

cargo 永远不会打包含自己 `Cargo.toml` 的子目录，所以独立成 crate 的示例扩展根本进不了 `duckfn`
的发布包。并进本包是唯一能让包里带上完整示例的做法 —— `src/extension/` 的模块树、
`src/bin/duckfn.rs` 这个命令行入口、`test/sql/` 的 sqllogictest 用例（确切清单见
[构建与发布](./build-and-release.md)）。

真正编译它的是默认关闭的 `quack` feature：`make debug` 通过根 `Makefile` 里的
`TARGET_INFO += --features quack` 把它带上，`just build`、`just build_wasm` 同理。裸跑 `cargo build`
只会静默跳过目标，交出一个没有入口符号的 cdylib，DuckDB 要到 `LOAD` 才报错。依赖 `duckfn` 的下游
完全不受影响：源码在包里，feature 关着，依赖树与以前一致。跑运行时自身的测试用
`cargo test -p duckfn`，跑全部用 `cargo test --workspace`。

### 本仓库的 lib 与插件项目的差别

示例就在 lib 里，由此产生两点与扩展项目不同的地方：

```toml
[lib]
crate-type = ["rlib", "cdylib", "staticlib"]
```

`crate-type` 不能按 target 覆写，而本包被当成多种东西使用：`cdylib` 是 DuckDB 加载的原生扩展、
`staticlib` 给 WebAssembly（那边由 `emcc` 完成最终链接，要的是 `.a`）、`rlib` 给下游。三个都列上也
正是这里不需要 `[[example]]` wasm root 的原因 —— 示例树只由 lib 编一遍，入口符号与 `inventory`
注册项都只有一份；多编一遍会把每个函数注册两次，wasm 上重复的入口符号直接链接失败。

同一件事还带来两处命名细节：

- **入口符号单独放在 `src/extension/entry.rs`**：lib 提供一份，而 CLI 链接这个 lib、又用 `#[path]`
  编进 `extension/mod.rs`，在那里再定义一次就是重复定义（Windows 上直接 `LNK2005`）。
- **CLI 的 bin 目标叫 `duckfn-cli`**（文件仍是 `src/bin/duckfn.rs`）：本包 cdylib 的产物也叫
  duckfn，Windows 上两者的 `.pdb` 会撞名。下游项目的包名不同、不会撞，所以模板里那个 bin 依旧叫
  `duckfn`。

## 日常命令

```bash
make debug                               # 构建扩展
just sql "SELECT double_it5(21);"        # 重新构建并执行一条语句
make test                                # 跑 sqllogictest 套件
just doc                                 # 生成 duckfn 的 rustdoc
```

`just test` 等价于 `make configure debug test`。

`.cargo/config.toml` 在 `x86_64-pc-windows-msvc` 上静态链接 C 运行时，除此之外不需要任何按平台的额外配置。

## 调试

扩展代码运行在 **`duckdb` 进程内**，所以调试器要附加到那个进程，而不是由 IDE 自己启动一个程序：

1. 用带调试符号的方式构建 —— `make debug`，或 `just build`（即
   `cargo duckdb-ext build -- --features quack`）。
2. 启动 DuckDB 并保持会话存活，例如 `duckdb -unsigned`。
3. 在该会话里执行 `LOAD '/path/to/my_ext.duckdb_extension';`。
4. 在 IDE 里附加到正在运行的 `duckdb` 进程 —— RustRover 见
   [附加到进程](https://www.jetbrains.com/zh-cn/help/rust/2026.2/attach-to-process.html)。
5. 在函数里打断点，然后执行调用它的 SQL，例如 `SELECT double_it(21);`。

动态库是在 `LOAD` 那一刻才进入进程的，所以更早设置的断点会从那一刻起才开始解析。调试器已附加时，
`just sql "<SQL>"` 是手动执行一条语句的快捷方式。

## 测试

用 DuckDB 官方的 sqllogictest 最实用：它通过 SQL 来验证扩展，也就是 DuckDB 真正调用扩展的方式，
而且 CI 跑的就是这一套。它需要先跑通 `make` 流程（`make configure` 做一次，之后用 `make test`）。

测试是 `test/sql/` 下的 sqllogictest 文件，与源码目录一一对应：

```
test/sql/demo/       <源文件名>.test
test/sql/functions/  <源文件名>.test
test/sql/types/      <type>_scalar_echo.test、<type>_table_echo.test
```

文件开头先声明依赖的扩展，然后成对给出 SQL 与期望输出：

```sql
require duckfn

query I
SELECT double_it5(21);
----
42

statement error
SELECT CAST('abc' AS INTEGER);
----
not an integer: "abc"
```

新增函数时请一并新增对应的 `.test` 文件：文档里引用的期望值就来自这里，它们是行为的唯一权威。
文件里用到的类型码包括 `I`（整数）、`T`（文本）、`R`（实数）以及 `IT` 这类组合；
LIST、MAP、STRUCT、ARRAY 的值统一用 `CAST(… AS VARCHAR)` 后按文本比较。

## 文档

站点在 `docs/`。`docs/docs/` 下的每个英文页面都需要在
`docs/i18n/zh-Hans/docusaurus-plugin-content-docs/current/` 的同路径下提供简体中文版本：

- 翻译正文与面向读者的 front matter（`title`、`description`）。
- `sidebar_position` 必须保持一致，两种语言的侧边栏顺序才会相同。
- 页面之间用相对文件路径互链（`./types.md`、`../guide/types.md`），这样每种语言都指向自己的页面。

```bash
cd docs
npm start                # http://localhost:3000
npm start -- --locale zh-Hans
npm run build            # 两种语言都必须通过；断链会直接让构建失败
```

## 约定

- 与周围代码保持一致风格。当前 workspace 并不满足 `cargo fmt --check`，对整个仓库跑 `cargo fmt` 会连带改写
  与你改动无关的文件 —— 只格式化你碰过的那部分。并保持 `cargo clippy` 无告警。
- 错误信息以产生它的函数名开头，例如 `dfn_table_checked: n must be >= 0`。
- 面向使用者的代码保持无 `unsafe`；唯一接受的例外是显式注册路径，那里需要
  `unsafe { c.register_scalar(…) }` 这类调用。
- 新增属性参数加到真正需要它的那个宏自己的参数结构体里（`duckfn-macro/src/<宏>.rs`）；每个宏只声明
  自己的键，也不再把自己的参数透传给 derive 宏。
- 行为变化时，更新顺序是：先改 sqllogictest 的期望值，再改引用它的文档页，最后改 README。

## 接下来

- [架构](./internals/architecture.md) —— 你要改的代码在哪里。
- [构建与发布](./build-and-release.md) —— CI 与发版流程。
