---
title: 问题排查
sidebar_position: 9
description: 官方 CI 的 WebAssembly 作业锁定的 Rust 1.86，以及会把全 NULL 列表字面量读坏的上游 bug。
---

# 问题排查

下面两件事都不是 duckfn 造成的，但你迟早会碰到。每一条都说清现象、原因和处理办法。

与**目录结构**有关的那些 —— 几个 crate root、`error[E0583]`、IDE 对独立 wasm root 标红 ——
搬到了[项目结构约定](./getting-started/project-structure.md)，因为那是「项目怎么搭起来」的问题，
不是「哪里出了故障」。

## 官方 CI 的 WASM 构建锁定在 Rust 1.86

**现象。** 打版本 tag 时，分发流水线把各原生平台都构建出来了，但 WebAssembly 那个作业失败 —— 有时甚至
在你自己的 crate 开始编译之前就失败：

```
error[E0658]: `let` expressions in this position are unstable
  --> ar_archive_writer-0.5.0/src/archive_writer.rs:591:20
```

**原因。** 本仓库调用的可复用工作流
（`duckdb/extension-ci-tools/.github/workflows/_extension_distribution.yml@v1.5-variegata`，接在
`.github/workflows/MainDistributionPipeline.yml` 里）把 WebAssembly 工具链写死了：

```yaml
- name: Setup Rust for cross compilation
  uses: dtolnay/rust-toolchain@1.86.0
  with:
    targets: wasm32-unknown-emscripten
```

只有 wasm 作业被固定版本，而且固定在这个可复用工作流内部，扩展仓库不 fork 就无法提高版本。依赖图里任何需要
1.86 之后语言特性的 crate —— 本次报告的是 `ar_archive_writer 0.5.x` 里的 let-chain —— 都会在那里失败，
而 `linux_amd64`、`osx_*`、`windows_*` 全部通过。

**现状。** 上游 issue：
[`duckdb/extension-ci-tools#385`](https://github.com/duckdb/extension-ci-tools/issues/385)，
截至 2026-09-15 仍未关闭，提出的两种修法（提高版本锁定，或把工具链版本暴露成工作流输入）都还没合并。
在此之前，要么让依赖图保持能被 1.86 编译，要么在不发布 wasm 时用 `exclude_archs`（`wasm_eh` 等 wasm
变体）跳过这些目标。

## 全 NULL 的列表字面量传入后是脏数据

**现象。**

```sql
SELECT dfn_echo_list_integer_n([NULL, NULL, NULL, NULL]);
-- [NULL, 0, 0, 0]                    期望 [NULL, NULL, NULL, NULL]

SELECT dfn_echo_map_varchar_integer_n(map(['a', 'b'], [NULL, NULL]));
-- {a=NULL, b=0}                     期望 {a=NULL, b=NULL}
```

那些 0 是未初始化内存：换一个进程运行，打印出来的可能是别的值。

**原因。** 这是 DuckDB 上游的 bug，不是 duckfn 的转换错误 ——
[`duckdb/duckdb#25616`](https://github.com/duckdb/duckdb/issues/25616)。该列表的 child 向量仍然是
*constant* NULL 向量：逻辑长度是 4，但背后只有一个物理元素。DuckDB 内部代码会先把它展平
（`UnifiedVectorFormat`、`Vector::Flatten()`），但 C API 只暴露了 `duckdb_vector_get_data()` 与
`duckdb_vector_get_validity()`，既没有向量表示（vector representation），也没有按逻辑索引访问的接口 ——
扩展按逻辑索引去读 child 向量时，从第二个元素起就读到了缓冲区之外。

**触发条件。** 仅限**每个元素**都是未标注类型 `NULL` 的字面量：

| 表达式 | 结果 |
| --- | --- |
| `[NULL, NULL, NULL, NULL]` | 脏数据 |
| `[NULL, NULL, NULL, NULL]::INTEGER[]` | 脏数据 —— 给整个列表标类型没用 |
| `[NULL, NULL::INTEGER, NULL, NULL]` | 正常 |
| `[NULL::INTEGER, NULL::INTEGER, NULL::INTEGER, NULL::INTEGER]` | 正常 |
| `[1, 1, 1, 1]` | 正常 |
| 来自表或表达式计算结果的列表 | 正常 |

**规避办法。** 给至少一个**元素**标类型（`[NULL, NULL::INTEGER, NULL, NULL]`），而不是给列表标类型；
或者不要直接传字面量，让值由查询产生。已确认影响 DuckDB v1.5.4 与 v1.5.5，上游仍未修复。

## 相关页面

- [项目结构约定](./getting-started/project-structure.md) —— 几个 crate root、`error[E0583]`、IDE 对
  独立 wasm root 标红。
- [常见问题](./faq.md) —— 写函数时实际踩到的那些报错。
- [构建与发布](./build-and-release.md) —— 打 tag 时流水线做了什么。
- [架构](./internals/architecture.md) —— 注册与派发到底怎么运作。
