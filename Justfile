# duckfn 运行时仓库自己的 Justfile。
#
# 日常命令（build / sql / repl / lint / test / docs_* / ci-* / release_* …）都在
# scripts/common.just 里 —— 那份是**共享源**，下游扩展项目（duckfn-extension-template 与由它
# 生成的项目）各自 import 一份同样的副本，用 `just sync-common` 同步；改共享内容改的就是这里。
#
# 本文件只留三类东西：
#   1. 机器相关的设置与项目相关的变量（windows-shell、extension_name）；
#   2. 需要**覆盖**共享 recipe 的少数几条（示例挂在 `quack` feature 上，命令得带上它）；
#   3. 本仓库特有、下游用不到的 recipe（crates.io 发布、duckfn-docs-kit 的 npm 发版）。
#
# 覆盖同名 recipe 必须显式允许（见下面的 set）：不开的话 just 在解析期就报
# 「recipe ... is redefined」，连 `just --list` 都跑不了。

# Windows 下 recipe 交给 Git Bash 执行；按自己的 Git 安装路径调整。
# 这是机器相关的路径，所以留在本地文件里，不放进共享文件。
set windows-shell := ["C:\\Program Files\\Git\\bin\\bash.exe", "-c"]

# 共享 recipe 允许在本文件里覆盖（浅层覆盖深层：以下定义生效）。
set allow-duplicate-recipes := true

import "scripts/common.just"

# 示例扩展名（根包 duckfn 的 cdylib 就是它），与 test/extension/entry.rs 里
# duckfn_entrypoint!("...")、根 Makefile 的 EXTENSION_NAME、CI 的 extension_name 一致
extension_name := "duckfn"

# 本仓库的示例扩展与 CLI 都挂在默认关闭的 `quack` feature 上，涉及 cargo 的命令都得显式带上它
# （不带的话 cargo 只是静默跳过目标，产物没有入口符号，LOAD 时才报错）。
_quack := "--features quack"

# ==== 覆盖共享 recipe ====

# 日常构建 -> target/debug/duckfn.duckdb_extension
build:
    cargo duckdb-ext build -- {{_quack}}

# 全量 release 构建
release:
    cargo build --release {{_quack}}

# 提交前检查：这是全 workspace 的 crate，所以 --workspace；示例、CLI 都在 `quack` 上，所以 --all-features
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# 生成 function_descriptions.csv（本仓库的 CLI 目标名是 duckfn-cli，理由见 Cargo.toml 的 [[bin]]）
docs_csv:
    cargo run {{_quack}} --bin duckfn-cli -- function_descriptions

# WebAssembly 构建：产物是 `examples/duckfn.rs` 的 staticlib（libduckfn.a），与下游一样走
# --example（见 Cargo.toml 的 [[example]]）；`quack` 是示例树与 CLI 的开关。
build_wasm:
    cargo build --release --target wasm32-unknown-emscripten --example {{extension_name}} {{_quack}}

# 发版前检查：clippy（warning 视为错误）与全 feature 构建都必须干净
release_check: lint
    cargo build --workspace --all-features

# ==== 本仓库特有：crates.io 发布 ====

# 生成 duckfn 的 rustdoc
doc:
    cargo doc -p duckfn

# 发布两个 crate 到 crates.io（duckfn-macro 必须先上线）
release_publish:
    just publish_macro_dry
    just publish_macro
    just publish_dry
    just publish

publish_macro_dry:
    cargo publish -p duckfn-macro --registry crates-io --dry-run

publish_macro:
    cargo publish -p duckfn-macro --registry crates-io

publish_dry:
    cargo publish -p duckfn --registry crates-io --dry-run

publish:
    cargo publish -p duckfn --registry crates-io

# ==== 本仓库特有：duckfn-docs-kit（npm 包）发版 ====
#
# 与上面的 release_* 完全独立：那个发 crates.io 上的 crate、打 v*.*.* tag（会触发扩展构建与
# 文档站部署），这个发 npm 包、打 docs-kit-v* tag（触发 .github/workflows/PublishDocsKit.yml，
# 由它在 CI 里用可信发布 —— trusted publishing / OIDC —— 发布到 npm，仓库里不存 npm token）。
#
# This is independent of the release_* recipes above: those publish the crates.io crates under a
# v*.*.* tag (triggering the extension build and the docs deployment), while these publish the npm
# package under a docs-kit-v* tag, which triggers .github/workflows/PublishDocsKit.yml — the CI job
# that publishes through npm's trusted publishing (OIDC). No npm token is stored anywhere.

# 发版前检查：构建 + 类型检查 + 预览 npm 包里会装进什么
release_kit_check:
    npm run build -w duckfn-docs-kit
    npm run typecheck -w duckfn-docs-kit
    npm pack -w duckfn-docs-kit --dry-run

# 提升版本号（只动 duckfn-docs-kit/package.json 与根 package-lock.json）：just release_kit_bump X.Y.Z
release_kit_bump new_version:
    bash scripts/release-docs-kit.sh bump "{{new_version}}"

# 打 docs-kit-v* tag 并推送 —— 这一步就是发布：just release_kit_tag X.Y.Z
release_kit_tag version:
    bash scripts/release-docs-kit.sh tag "{{version}}"

# 切到下一开发版本（参数形如 X.Y.Z-dev.0）：just release_kit_dev X.Y.Z-dev.0
release_kit_dev new_version:
    bash scripts/release-docs-kit.sh dev "{{new_version}}"

# 查看最近一次 npm 发布流水线（PublishDocsKit.yml）的运行状态
release_kit_ci:
    gh run list --workflow=PublishDocsKit.yml --limit 5

# 在本地发布到 npm（回退路径：正常流程由 PublishDocsKit.yml 在 CI 里发；先 npm login）
release_kit_publish: release_kit_guard
    npm publish -w duckfn-docs-kit

# 只打包不推送，演练一遍
release_kit_publish_dry:
    npm publish -w duckfn-docs-kit --dry-run

release_kit_guard:
    bash scripts/release-docs-kit.sh guard
