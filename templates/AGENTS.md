<!--
AGENTS.md 模板：复制到你的 DuckDB 扩展项目根目录，命名为 AGENTS.md。
只需把下面两个 {{...}} 填好。填完**保留**这两行（不是一次性的）：
以后 clone 路径或项目目标变了，还在这里改。
-->

# AGENTS.md

这是一个用 [duckfn](https://crates.io/crates/duckfn) 写的 DuckDB 扩展（loadable extension）。

## 项目事实（唯一需要人维护的一段）

- 这个扩展做什么：{{PROJECT_GOAL}}
- duckfn 仓库在本机的 clone：`{{DUCKFN_REPO}}`

> 扩展名、crate 名、duckfn 版本**不要抄到这里**：
> 扩展名读 `src/extension/mod.rs` 里的 `duckfn_entrypoint!("...")`（也是 `Makefile` 的 `EXTENSION_NAME`），
> crate 名与 duckfn 版本读 `Cargo.toml`。它们本来就在代码里，复制一份只会变成第二份会过期的真相。

## 动手前先读

**`{{DUCKFN_REPO}}/templates/duckfn-conventions.md`** —— 知识源（先读哪、再读哪）、
硬约束、开发循环、新增函数的完整流程都在这份文件里。

它由 duckfn 仓库维护，本文件**只引用、不复制**，所以 duckfn 升级时不需要重做本文件，
只要 `git -C {{DUCKFN_REPO}} pull`（或 `checkout` 到对应 tag）。

本机还没有 clone 时先来一份（文档、示例扩展、sqllogictest 范例都在里面，
而且它们不会随依赖进入项目）：

```shell
git clone https://github.com/shijianjs/duckfn
```

## 升级 duckfn 时

1. 改 `Cargo.toml` 里的 duckfn 版本，`cargo update -p duckfn -p duckfn-macro`。
2. `git -C {{DUCKFN_REPO}} fetch --tags && git -C {{DUCKFN_REPO}} checkout v<新版本>`，
   让文档与示例跟依赖对齐；约定文件随这次切换一起更新。
3. 本文件不用改。
