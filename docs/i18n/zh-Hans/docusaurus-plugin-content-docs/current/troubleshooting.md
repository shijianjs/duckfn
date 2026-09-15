---
title: 问题排查
sidebar_position: 9
description: IDE 误报 WebAssembly 入口文件、嵌套模块时的 E0583、官方 CI 锁定的 Rust 1.86，以及会把全 NULL 列表字面量读坏的上游 bug。
---

# 问题排查

下面四件事都不是 duckfn 造成的，但你迟早会碰到。每一条都说清现象、原因和处理办法。

## IDE 在 wasm 入口文件上报错

**现象。** RustRover 或 rust-analyzer 给 `src/wasm_lib.rs` 标红，但 `make debug`、`just build`、
`cargo duckdb-ext build` 都复现不出来。

**原因。** `Cargo.toml` 把这个文件注册成了 example：

```toml
[[example]]
# crate-type can't be (at the moment) be overriden for specific targets
path = "src/wasm_lib.rs"
crate-type = ["staticlib"]
```

`crate-type` 无法按 target 区分，而两个 target 需要的值不同 —— 本地编译要 `cdylib`，WebAssembly 要
`staticlib`，所以模板多带了一个只在交叉编译时才构建的 crate root：

```shell
just build_wasm     # cargo build --release --target wasm32-unknown-emscripten --example rusty_quack
```

而 IDE 默认检查**所有 target**（`cargo check --all-targets`），于是把这个 example 也按本地平台编译了一遍
—— 那并不是它被设计来编译的平台。

**解决。** 给整个文件加架构门控：在其它 target 上它会被编译成空，报错随即消失：

```rust
#![cfg(target_arch = "wasm32")]
#![allow(special_module_name)]

mod extension;
```

第二个属性用于消除「crate root 不叫 `lib.rs`」那条 lint 提示。

## 嵌套模块时报 E0583

**现象。** 模块一旦多出一层就编不过 —— 例如 `src/` 下新增第二级时：

```
error[E0583]: file not found for module `demo`
 --> src\lib.rs:3:1

error[E0583]: file not found for module `types`
 --> src\lib.rs:4:1
```

**原因。** 官方模板让 WebAssembly 入口重新导出 native 入口：

```rust
// src/wasm_lib.rs，官方模板
mod lib;
```

`mod lib;` 解析到 `src/lib.rs`，从这一刻起 `lib.rs` 就是一个**文件**模块：它的子模块要去**它旁边**、
也就是 `src/lib/` 下面找。于是写在 `src/lib.rs` 里的 `mod demo;` 会去找 `src/lib/demo.rs`，而不是
`src/extension/demo.rs`，rustc 就在那行 `mod` 上报 E0583。扁平的 `lib.rs` 掩盖了这个问题，嵌套才把它暴露出来。

**解决：让两个入口保持一致。** 两个 crate root 声明同一个路径，所有模块都放在 `src/extension/` 下：

```
src/
├─ lib.rs              mod extension;                 native，crate-type = ["cdylib"]
├─ wasm_lib.rs         mod extension;                 wasm example，crate-type = ["staticlib"]
└─ extension/
   ├─ mod.rs           mod demo; mod functions; mod types;
   │                   duckfn_entrypoint!("rusty_quack");
   ├─ demo/
   ├─ functions/
   └─ types/
```

`mod extension;` 解析到 `src/extension/mod.rs` —— 这是一个**目录**模块，所以两个入口看到的是同一棵树，
`src/extension/mod.rs` 里的 `mod demo;` 会去找 `src/extension/demo.rs` 或 `src/extension/demo/mod.rs`。
任意层级嵌套都成立，因为不再有任何路径相对于文件模块解析。

**规则。** 两个入口保持逐行一致，不要相互再导出；`duckfn_entrypoint!` 写在 `src/extension/mod.rs`，
其余模块全部挂在它下面。本仓库就是这么做的，这也是 `src/lib.rs` 与 `src/wasm_lib.rs` 各自只有三行的原因。

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

- [常见问题](./faq.md) —— 写函数时实际踩到的那些报错。
- [构建与发布](./build-and-release.md) —— 打 tag 时流水线做了什么。
- [架构](./internals/architecture.md) —— 注册与派发到底怎么运作。
