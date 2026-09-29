---
title: 问题排查
sidebar_position: 10
description: 官方 CI 的 WebAssembly 构建对 Rust 1.86 的锁定（已解决）、会把全 NULL 列表字面量读坏的上游 bug，以及为什么 `panic!` 在 WebAssembly 上不可用。
---

# 问题排查

下面几件事基本都不是 duckfn 造成的，但做 WebAssembly 构建时轻易会碰到。第一条上游已修复，保留作为
背景。每一条都说清现象、原因和处理办法。

与**目录结构**有关的那些 —— 几个 crate root、`error[E0583]`、IDE 对独立 wasm root 标红 ——
搬到了[项目结构约定](./getting-started/project-structure.md)，因为那是「项目怎么搭起来」的问题，
不是「哪里出了故障」。

## 官方 CI 的 WASM 构建不再锁定 Rust 1.86（已解决）

**曾经。** 可复用分发工作流把 WebAssembly 作业写死在 Rust 1.86（`dtolnay/rust-toolchain@1.86.0`），
而所有原生作业用 stable；这个固定又在可复用工作流**内部**，扩展仓库不 fork 就无法提高。依赖图里
任何需要 1.86 之后语言特性的 crate —— 本次报告的是 `ar_archive_writer 0.5.x` 里的 let-chain ——
都只在 `wasm_*` 作业失败，而 `linux_amd64`、`osx_*`、`windows_*` 全部通过。

**现在。** 上游已修复：wasm 作业的 Rust 工具链已升到 **1.97.1**
（[`duckdb/extension-ci-tools#394`](https://github.com/duckdb/extension-ci-tools/pull/394)，关闭了
[`#385`](https://github.com/duckdb/extension-ci-tools/issues/385)）。你**不需要**再把依赖图保持成
1.86 可编译，也不必为了避开它而 `exclude_archs` 掉 wasm 变体。

**仍需检查的一点。** 这次提升在含 #394 的 `extension-ci-tools` 版本里，所以仓库引用的 ref 要足够新：
相应调高 `.github/workflows/MainDistributionPipeline.yml` 里的 `ci_tools_version`（以及 vendored 的
`extension-ci-tools` 子模块）。早于修复的 ref 会保持旧行为 —— 比如本仓库当前钉的 `@v1.5-variegata`，
其 vendored 工作流里 wasm 作业仍是 `dtolnay/rust-toolchain@1.86.0`，直到该 ref 往前推。

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

## 函数里的 `panic!` 在 WebAssembly 上不可用

**现象。** 原生 CLI（以及 `just test`）里，一个 `panic!` 的函数会报出可读的 panic 消息；
而**同一个调用**在浏览器（DuckDB-Wasm）里会失败成：

```
RangeError: Maximum call stack size exceeded
```

—— 完全看不到你写的 panic 消息。（扩展本身没被搞挂：同一连接上后面的调用仍正常；只有那一次调用
会变成栈溢出的假错。）

**原因 —— 卡在 Rust 的构建，不是 Emscripten。** 有两层必须对齐，而 duckfn 只能控其中一层：

- **DuckDB-Wasm 的 `eh` bundle** 确实启用了 Wasm 层的异常处理（exception handling），所以
  DuckDB/C++ 抛出的异常能被捕获。这一层没问题。
- **Rust 那一层**需要的是 *Rust 自己的* 展开（unwinding），而该目标并没有把它编出来。rustup
  为 `wasm32-unknown-emscripten` 提供的预编译 `std` 是以 `panic = "abort"` 构建的 —— 里面没有
  `libpanic_unwind`。所以尽管 duckfn 与 quack-rs 把每个回调都包在 `std::panic::catch_unwind` 里、
  并以 `panic = "unwind"` 构建，却没有可用的展开运行时：`panic!` 直接 abort，在 wasm 运行时里
  就表现成 `RangeError: Maximum call stack size exceeded`，而不是你的消息。原生上展开是真实存在
  的 —— 这正是为什么 `catch_unwind` 在 native 能把 panic 转成可读的 DuckDB 错误。

要让 `catch_unwind` 在 wasm 上真正生效，得带展开地重编 `std`，而目前这一步需要 nightly：
`RUSTFLAGS="-Cpanic=unwind" cargo +nightly build -Zbuild-std=std,panic_unwind …`（`-Zbuild-std`
仍仅限 nightly）。而官方 `wasm_eh` 的 CI **并没有**这么做 —— 它用 stable
`cargo build --target wasm32-unknown-emscripten` 编出 staticlib、最后才跑 `emcc`，而
`emcc -fwasm-exceptions` 救不回一个已被 Rust 编成 `abort` 的 `panic!`。这就是“安全网在 native
真、在 wasm 形同虚设”的全部原因。

可恢复那条路径不受影响：返回 `Err(duck_error("..."))`（`DuckOptionResult`）根本不展开，所以在
原生与 wasm 上都是干净可读的 `Invalid Input Error`。

**处理办法 —— 不要用 `panic!` 来报错。** 凡是调用方能响应的情形，都返回 `Err(duck_error("..."))` /
`DuckOptionResult`。`panic!` 只留给“这是必须中断的内部 bug”，并知道它在 WebAssembly 上对读者
只会呈现为一个栈溢出，而不是一条消息。以上基于本地自建的 `wasm_eh` 扩展（emscripten 3.1.71）实测。

## 相关页面

- [项目结构约定](./getting-started/project-structure.md) —— 几个 crate root、`error[E0583]`、IDE 对
  独立 wasm root 标红。
- [常见问题](./faq.md) —— 写函数时实际踩到的那些报错。
- [构建与发布](./build-and-release.md) —— 打 tag 时流水线做了什么。
- [架构](./internals/architecture.md) —— 注册与派发到底怎么运作。
