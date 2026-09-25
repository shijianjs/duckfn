---
title: 项目结构约定
sidebar_position: 2
description: 本仓库的目录结构只遵循一条规则 —— 每个 crate root 声明同一个 extension 模块；违反它会得到哪两个报错。
---

# 项目结构约定

本仓库有两个 crate root，规则只有一条：**每个 crate root 都包含 `src/extension/` 这棵**目录**
模块，谁也不要再导出谁。** 做到了，模块嵌套到任意深度都成立，命令行工具也能看见全部函数；做不到，
模块第一次多出一层就会报 `error[E0583]`。

```text
src/
├─ lib.rs              #[cfg(feature = "quack")] mod extension;        运行时与示例同处一个 crate：
│                      #[cfg(feature = "quack")] mod extension_entry;  crate-type = ["rlib", "cdylib", "staticlib"]
├─ bin/
│  └─ duckfn.rs        #[path = "../extension/mod.rs"] mod extension;  CLI（bin 目标名 `duckfn-cli`）
└─ extension/
   ├─ mod.rs           mod demo; mod functions; mod types;
   ├─ entry.rs         duckfn_entrypoint!("duckfn");
   ├─ demo/
   ├─ functions/
   └─ types/
```

两个 root 指向的都是 `src/extension/mod.rs` —— 一个**目录**模块，所以它们看到的是同一棵树。入口符号
则单独放在旁边的 `src/extension/entry.rs`，因为它只能定义一次：lib 里带着一份，而 CLI 链接了那个 lib，
所以它只包含 `extension/mod.rs`。

## 一个 lib，三种 crate-type

`crate-type` 无法按 target 区分，于是 lib 把自己需要的类型全列上，各平台取自己那份：下游要 `rlib`、
DuckDB 加载的原生扩展要 `cdylib`、WebAssembly 要 `staticlib`（那边由 `emcc` 做最终链接，要的是 `.a`）：

```toml
[lib]
crate-type = ["rlib", "cdylib", "staticlib"]
```

两个扩展产物因此都出自 `src/extension/` 的**同一份编译**，这一点很关键：多编一份就会把每个函数注册两次，
而且在 wasm 上重复的入口符号会直接链接失败。

上游模板走的是另一条路，也是「wasm 目标需要一个独立 root」时的选择：另开一个 `[[example]]`，让它的 root
自己声明 `mod extension;`。

```toml
[[example]]
# crate-type can't be (at the moment) be overriden for specific targets
path = "src/wasm_lib.rs"
crate-type = ["staticlib"]
```

这种 root 也**不是** `src/lib.rs` 的别名 —— 它自己声明 `mod extension;`。而官方上游模板写的是：

```rust
// src/wasm_lib.rs，官方模板
mod lib;
```

`error[E0583]` 就是从这来的。`mod lib;` 解析到 `src/lib.rs`，从这一刻起 `lib.rs` 就是一个**文件**
模块：它的子模块要去**它旁边**、也就是 `src/lib/` 下面找。于是写在 `src/lib.rs` 里的 `mod demo;`
会去找 `src/lib/demo.rs`，而不是 `src/extension/demo.rs`，rustc 就报：

```
error[E0583]: file not found for module `demo`
 --> src\lib.rs:3:1

error[E0583]: file not found for module `types`
 --> src\lib.rs:4:1
```

扁平的 `lib.rs` 掩盖了这个问题，嵌套才把它暴露出来。两个 root 声明同一个路径就完全绕开了它 ——
`src/extension/mod.rs` 里的 `mod demo;` 会去找
`src/extension/demo.rs` 或 `src/extension/demo/mod.rs`。

这也是那种 wasm root 只有三行的原因，以及为什么新增模块一律放进 `src/extension/`，而不是放在某个
crate root 旁边。

## 命令行工具是第二个 root

`src/bin/duckfn.rs`（用途见[社区扩展文档页](../community-extension-docs.md)）同样是一个
独立的 crate root，它的二进制里也必须带着同一棵树。它没法简单地依赖库，两个原因：

- **路径。** `src/bin/` 下的 crate root 会把 `mod extension;` 解析成 `src/bin/extension.rs`，而不是
  `src/extension/`。`#[path]` 把它指回另外两个 root 用的那个目录模块。
- **注册。** `#[duck_*]` 的文档元数据靠 `inventory` 的静态构造器收集，只有真正被链接进最终二进制的
  目标文件才会生效。只依赖库的话，链接器可能把这些模块整块丢掉，导出的 CSV 会是空的 —— 而且不报错。

所以它自己声明这个模块：

```rust
#[path = "../extension/mod.rs"]
mod extension;
```

它刻意不包含 `extension/entry.rs`：它链接的 lib 里已经定义了入口符号，再定义一次就是重复定义
（Windows 上直接 `LNK2005`）。它的 Cargo 目标名是 `duckfn-cli` 而不是文件名的 `duckfn`，因为本包的
cdylib 产物也叫 duckfn，Windows 上两者的 `.pdb` 会撞名；下游项目没有这个冲突，继续用 `duckfn` 就好。

以后项目里再加 crate root，就是同样两行，指向 `src/extension/mod.rs`，别的都不用动。

## IDE 对独立的 wasm root 标红

如果你的项目像上面那样另开了一个 wasm example，RustRover 或 rust-analyzer 会给它标红，但
`make debug`、`just build`、`cargo duckdb-ext build` 都复现不出来。

因为 IDE 默认检查**所有 target**（`cargo check --all-targets`），于是把这个 example 也按本地平台
编译了一遍 —— 那并不是它被设计来编译的平台。给整个文件加架构门控即可，在其它 target 上它会被编译
成空：

```rust
#![cfg(target_arch = "wasm32")]
#![allow(special_module_name)]

mod extension;
```

第二个属性用于消除「crate root 不叫 `lib.rs`」那条 lint 提示。

## 相关页面

- [创建项目](./create-a-project.md) —— 这套目录结构来自哪个模板。
- [构建与发布](../build-and-release.md#webassembly) —— WebAssembly 目标是怎么构建的。
- [社区扩展文档页](../community-extension-docs.md) —— `src/bin/duckfn.rs` 是干什么的。
- [问题排查](../troubleshooting.md) —— 与目录结构无关的那些问题。
