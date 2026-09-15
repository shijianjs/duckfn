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
| DuckDB CLI | 手动加载扩展时使用。 |

`extension-ci-tools/` 是一个 git submodule，而 `Makefile` 会 include 它的 makefile，因此新克隆之后需要：

```bash
git submodule update --init --recursive
make configure
```

## workspace 结构

| 成员 | 是否发布 | 说明 |
| --- | --- | --- |
| `duckfn/` | 是 | 运行时框架。 |
| `duckfn-macro/` | 是 | 过程宏；不依赖运行时，只依赖 `darling`、`syn`、`quote`。 |
| `/`（`rusty_quack`） | 否（`publish = false`） | 示例扩展，放在根目录以便复用 DuckDB 官方 CI。 |

`duckfn/` 锁定 `duckfn-macro = "=0.0.2"`，因此两个 crate 总是一起发布。

## 日常命令

```bash
make debug                               # 构建扩展
just duckdb_ext "SELECT double_it5(21);" # 重新构建并执行一条语句
make test                                # 跑 sqllogictest 套件
just doc                                 # 生成 duckfn 的 rustdoc
```

`just test` 等价于 `make configure debug test`。

`.cargo/config.toml` 在 `x86_64-pc-windows-msvc` 上静态链接 C 运行时，除此之外不需要任何按平台的额外配置。

## 测试

测试是 `test/sql/` 下的 sqllogictest 文件，与源码目录一一对应：

```
test/sql/demo/       <源文件名>.test
test/sql/functions/  <源文件名>.test
test/sql/types/      <type>_scalar_echo.test、<type>_table_echo.test
```

文件开头先声明依赖的扩展，然后成对给出 SQL 与期望输出：

```sql
require rusty_quack

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

- 跑 `cargo fmt`，并保持 `cargo clippy` 无告警。
- 错误信息以产生它的函数名开头，例如 `dfn_table_checked: n must be >= 0`。
- 面向使用者的代码保持无 `unsafe`；唯一接受的例外是显式注册路径，那里需要
  `unsafe { c.register_scalar(…) }` 这类调用。
- 新增属性参数统一加到 `duckfn-macro/src/attr_args.rs` 里那个结构体上 —— 属性宏与
  `#[derive(DuckStruct)]` 共用它。
- 行为变化时，更新顺序是：先改 sqllogictest 的期望值，再改引用它的文档页，最后改 README。

## 接下来

- [架构](./internals/architecture.md) —— 你要改的代码在哪里。
- [构建与发布](./build-and-release.md) —— CI 与发版流程。
