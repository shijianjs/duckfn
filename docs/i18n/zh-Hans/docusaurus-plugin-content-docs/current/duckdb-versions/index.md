---
title: DuckDB 版本兼容性
sidebar_position: 1
description: duckfn 扩展与 DuckDB 版本的关系 —— C API / ABI 兼容性规则、wasm 构建工具链、以及 wasm 下的 panic 处理。
---

# DuckDB 版本兼容性

扩展从不链接 DuckDB —— 它走 C API 的函数表调用，而 DuckDB 在 `LOAD` 时决定是否收下这个文件。所以「这
个版本能不能用」其实是几个不同的问题，各有答案。本节把它们都覆盖。

## 本节包含

- **[版本数字与 ABI 兼容性](./c-api-and-abi.md)** —— 载入判定背后的三个数字（头文件发行版、元数据里的
  `TARGET_DUCKDB_VERSION`、加载它的引擎）、`C_STRUCT` 与 `C_STRUCT_UNSTABLE` 下元数据那一栏各是什么含义、
  稳定区与不稳定区的完整配置集、一份稳定产物能覆盖到哪（跨发行版、以及到 2.0）、以及如何验证一个构建。
  这是「面向哪个 DuckDB」这一侧。
- **[wasm 构建工具链](./wasm-toolchain.md)** —— 构建 `wasm_*` 产物是另一套、更绕的版本问题：CI 装的 Rust、
  钉死的 emsdk/binaryen、以及链接旗标，分处三个仓库；含确切的编译 / 链接 / 载入三种失败模式与实测的兼容窗。
- **[wasm 下的 panic 处理](./rust-wasm-unwinding.md)** —— 为什么 `panic!` / `catch_unwind` 在 wasm 上随
  `rustc` 版本表现不同。

日常「我的扩展能被哪个 DuckDB 加载」先看[版本数字与 ABI 兼容性](./c-api-and-abi.md)；当 `wasm_*` 构建或
wasm 下的 `panic!` 出问题时，再看后两页。
