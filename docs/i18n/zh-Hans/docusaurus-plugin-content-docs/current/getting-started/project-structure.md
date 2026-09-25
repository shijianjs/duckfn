---
title: 项目结构约定
sidebar_position: 2
description: 本仓库的目录结构只遵循一条规则 —— 每个 crate root 声明同一个 extension 模块；违反它会得到哪两个报错。
---

# 项目结构约定

本仓库有三个 crate root，规则只有一条：**每个 crate root 都声明 `mod extension;`，谁也不要
再导出谁。** 做到了，模块嵌套到任意深度都成立，IDE 不再对 WebAssembly 那个文件标红，命令行
工具也能看见全部函数；做不到，模块第一次多出一层就会报 `error[E0583]`。

```text
duckfn-quack/src/
├─ lib.rs              mod extension;                       native，crate-type = ["cdylib"]
├─ wasm_lib.rs         mod extension;                       wasm example，crate-type = ["staticlib"]
├─ bin/
│  └─ duckfn.rs        #[path = "../extension/mod.rs"]
│                      mod extension;                       CLI，cargo run -p duckfn_quack --bin duckfn
└─ extension/
   ├─ mod.rs           mod demo; mod functions; mod types;
   │                   duckfn_entrypoint!("duckfn_quack");
   ├─ demo/
   ├─ functions/
   └─ types/
```

三个 root 指向的都是 `duckfn-quack/src/extension/mod.rs` —— 一个**目录**模块，所以三者看到的是同一
棵树，`duckfn_entrypoint!` 也只出现在一个地方。

## 两个入口保持逐行一致

`crate-type` 无法按 target 区分，而两个 target 需要的值不同：本地编译要 `cdylib`，WebAssembly 要
`staticlib`。所以 `duckfn-quack/Cargo.toml` 多带了一个 crate root，注册成 example：

```toml
[[example]]
# crate-type can't be (at the moment) be overriden for specific targets
path = "src/wasm_lib.rs"
crate-type = ["staticlib"]
```

这个文件**不是** `duckfn-quack/src/lib.rs` 的别名 —— 它自己声明 `mod extension;`。而官方上游模板写的是：

```rust
// src/wasm_lib.rs，官方模板
mod lib;
```

`error[E0583]` 就是从这来的。`mod lib;` 解析到 `src/lib.rs`，从这一刻起 `lib.rs` 就是一个**文件**
模块：它的子模块要去**它旁边**、也就是 `src/lib/` 下面找。于是写在 `src/lib.rs` 里的 `mod demo;`
会去找 `src/lib/demo.rs`，而不是 `duckfn-quack/src/extension/demo.rs`，rustc 就报：

```
error[E0583]: file not found for module `demo`
 --> src\lib.rs:3:1

error[E0583]: file not found for module `types`
 --> src\lib.rs:4:1
```

扁平的 `lib.rs` 掩盖了这个问题，嵌套才把它暴露出来。两个 root 声明同一个路径就完全绕开了它 ——
`duckfn-quack/src/extension/mod.rs` 里的 `mod demo;` 会去找
`duckfn-quack/src/extension/demo.rs` 或 `duckfn-quack/src/extension/demo/mod.rs`。

这也是 `duckfn-quack/src/lib.rs` 与 `duckfn-quack/src/wasm_lib.rs` 各自只有三行的原因，以及为什么
新增模块一律放进 `duckfn-quack/src/extension/`，而不是放在某个 crate root 旁边。

## 命令行工具是第三个 root

`duckfn-quack/src/bin/duckfn.rs`（用途见[社区扩展文档页](../community-extension-docs.md)）同样是一个
独立的 crate root，它的二进制里也必须带着同一棵树。它没法简单地依赖库，两个原因：

- **路径。** `src/bin/` 下的 crate root 会把 `mod extension;` 解析成 `src/bin/extension.rs`，而不是
  `duckfn-quack/src/extension/`。`#[path]` 把它指回另外两个 root 用的那个目录模块。
- **注册。** `#[duck_*]` 的文档元数据靠 `inventory` 的静态构造器收集，只有真正被链接进最终二进制的
  目标文件才会生效。只依赖库的话，链接器可能把这些模块整块丢掉，导出的 CSV 会是空的 —— 而且不报错。

所以它自己声明这个模块：

```rust
#[path = "../extension/mod.rs"]
mod extension;
```

以后项目里再加 crate root，就是同样两行，指向 `duckfn-quack/src/extension/mod.rs`，别的都不用动。

## IDE 对 `src/wasm_lib.rs` 标红

RustRover 或 rust-analyzer 给 `duckfn-quack/src/wasm_lib.rs` 标红，但 `make debug`、`just build`、
`cargo duckdb-ext build` 都复现不出来。

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
- [社区扩展文档页](../community-extension-docs.md) —— `duckfn-quack/src/bin/duckfn.rs` 是干什么的。
- [问题排查](../troubleshooting.md) —— 与目录结构无关的那些问题。
