---
title: 快速开始
sidebar_position: 3
description: 编写、构建并加载一个最小的 duckfn 扩展，然后在 SQL 中调用它。
---

# 快速开始

本页从零构建一个名为 `my_ext` 的最小扩展。本仓库自带的示例扩展是 `rusty_quack`，覆盖了全部功能，
见[示例扩展](../examples/rusty-quack.md)。

如果想直接从带 CI 的可用骨架起步，见[创建项目](./create-a-project.md)。

## 1. 创建 crate

```toml title="Cargo.toml"
[package]
name = "my_ext"
version = "0.1.0"
edition = "2024"
rust-version = "1.86"

[lib]
crate-type = ["cdylib"]

[dependencies]
duckfn = "0.0.4"
quack-rs = "0.16.0"
libduckdb-sys = { version = ">=1.4.4, <2", features = ["loadable-extension"] }
```

## 2. 编写扩展

```rust title="src/lib.rs"
use duckfn::{duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult};

#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

duckfn_entrypoint!("my_ext");
```

有三个方面值得留意：

- 属性同时完成了**注册**与 FFI 包装的生成，没有单独的手动注册步骤。
- 入参 `Option<i64>`、出参 `DuckOptionResult<i64>` 是表达 `NULL` 的方式。完整的返回形态见
  [错误与 panic](../guide/errors-and-panics.md)。
- `duckfn_entrypoint!("my_ext")` 必须与扩展名一致，见下文。

## 3. 构建

DuckDB 官方流程会构建 `cdylib`、追加扩展元数据，并把产物放到 DuckDB 期望的位置：

```bash
make configure   # 只做一次：创建测试运行器所需的 Python venv
make debug       # -> build/debug/extension/my_ext/my_ext.duckdb_extension
```

`make release` 是带优化的同一套流程。两者都来自本仓库引入的 DuckDB `extension-ci-tools` makefile。

另一种方式是用 `cargo-duckdb-ext-tools` 插件直接从 Cargo 打包：

```bash
cargo duckdb-ext build   # -> target/debug/my_ext.duckdb_extension
```

本仓库用 `Justfile` 把两种流程都封装了，例如 `just duckdb_ext "SELECT double_it(21);"`。

## 4. 加载并调用

```bash
duckdb -unsigned -c "
LOAD './build/debug/extension/my_ext/my_ext.duckdb_extension';
SELECT double_it(21);
"
```

```text
42
```

| SQL | 结果 |
| --- | --- |
| `SELECT double_it(21);` | `42` |
| `SELECT double_it(NULL);` | `NULL` |
| `SELECT double_it(13);` | 报错：`unlucky input` |

扩展基于 DuckDB 的 unstable C API 构建，因此必须加 `-unsigned`。

## 入口符号名称

`duckfn_entrypoint!("my_ext")` 生成符号 **`my_ext_init_c_api`**，这正是 DuckDB 加载扩展时查找的符号。
名称不能为空，且只能包含小写 ASCII 字母、数字和下划线；不满足会在编译期报错，而符号名写错会导致扩展无法加载。

## 下一步

- [属性参考](../guide/attributes.md) —— 所有属性与宏参数集中在一处。
- [标量函数](../guide/scalar-functions.md) —— 返回形态、重载与函数集。
- [类型映射](../guide/types.md) —— 可以作为参数和返回值的 Rust 类型。
- [示例扩展](../examples/rusty-quack.md) —— 每个功能都有可运行的 SQL。
