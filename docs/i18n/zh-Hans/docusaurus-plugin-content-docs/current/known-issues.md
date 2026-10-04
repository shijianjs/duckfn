---
title: 已知问题
sidebar_position: 10
description: 会渗进扩展构建里的 DuckDB 上游 bug —— 目前是经 C API 读回脏数据的全 NULL 列表字面量。
---

# 已知问题

会渗进扩展构建里的上游 bug 与平台怪癖，每条都配规避办法。它们基本不是 duckfn 造成的。两个**与版本有关**
的问题 —— 官方 CI 的 wasm 构建过去把 Rust 钉在 1.86，以及为什么 `panic!` 在 WebAssembly 上随 Rust 版本表现
不同 —— 已移到 [DuckDB 版本兼容性](./duckdb-versions/index.md) 下：见
[wasm 构建工具链](./duckdb-versions/wasm-toolchain.md) 与
[Wasm 下的 panic 处理](./duckdb-versions/rust-wasm-unwinding.md)。用法层面的疑问（函数为什么没注册上、
为什么必须加 `-unsigned`）见[常见问题](./faq.md)。

与**目录结构**有关的那些 —— 几个 crate root、`error[E0583]`、IDE 对独立 wasm root 标红 —— 搬到了
[项目结构约定](./getting-started/project-structure.md)，因为那是「项目怎么搭起来」的问题，不是「哪里出了故障」。

## 全 NULL 的列表字面量传入后是脏数据

**现象。**

```sql {"type":"duckfn"}
SELECT dfn_echo_list_integer_n([NULL, NULL, NULL, NULL]);
-- [NULL, 0, 0, 0]                    期望 [NULL, NULL, NULL, NULL]
```

```sql {"type":"duckfn"}
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

## 另见

- [wasm 构建工具链](./duckdb-versions/wasm-toolchain.md) 与
  [Wasm 下的 panic 处理](./duckdb-versions/rust-wasm-unwinding.md) —— 那两个版本问题，现归于
  DuckDB 版本兼容性之下。
- [常见问题](./faq.md) —— 写函数时实际踩到的那些报错。
- [构建与发版](./development/build-and-release.md) —— 打 tag 时流水线做了什么。
- [架构](./development/architecture.md) —— 注册与派发到底怎么运作。
