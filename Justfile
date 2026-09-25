# duckfn workspace 的 Justfile。
#
# 日常开发走 Cargo（just build / just sql / just repl），发版流程见根目录 AGENTS.md。
# 下游扩展项目要的是精简版：duckfn-extension-template 仓库里的 Justfile。
#
# Windows 下 recipe 交给 Git Bash 执行；按自己的 Git 安装路径调整。
set windows-shell := ["C:\\Program Files\\Git\\bin\\bash.exe", "-c"]

# 示例扩展名（duckfn-quack crate），与 duckfn-quack/src/extension/mod.rs 里
# duckfn_entrypoint!("...")、根 Makefile 的 EXTENSION_NAME 一致
extension_name := "duckfn_quack"

# duckdb 命令行；不在 PATH 里时用 `just DUCKDB=/path/to/duckdb repl`
duckdb := env_var_or_default("DUCKDB", "duckdb")

ext_path := "./target/debug/" + extension_name + ".duckdb_extension"

# 不带参数运行 just 时列出所有 recipe
default:
    @just --list

# 构建示例扩展 -> target/debug/duckfn_quack.duckdb_extension
# 示例 crate 在 duckfn-quack/ 下，构建产物仍落在 workspace 根的 target/ 里
build:
    cargo duckdb-ext build --manifest-path duckfn-quack/Cargo.toml

# 构建后跑一条 SQL 就退出：just sql "SELECT double_it(21);"
sql sql: build
    {{duckdb}} -unsigned -c "LOAD '{{ext_path}}'; {{sql}}"

# 构建后进入 REPL（扩展已 LOAD）：just repl
repl: build
    {{duckdb}} -unsigned -cmd "LOAD '{{ext_path}}';"

# 全量 release 构建（注意：与下面的 release_* 发版流程不是一回事）
release:
    cargo build --release

# clippy，warning 视为错误
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# 跑 duckfn-quack/test/sql/**/*.test（等价 make configure debug test）
test: ci-build
    make test

# WebAssembly 构建
build_wasm:
    cargo build -p duckfn_quack --release --target wasm32-unknown-emscripten --example {{extension_name}}

# 工具链（首次）：固定 Rust 版本 + 装 wasm target
config_env:
    rustup override set 1.86.0
    rustup target add wasm32-unknown-emscripten
    rustup target list --installed

# 生成 duckfn 的 rustdoc
doc:
    cargo doc -p duckfn

# 生成社区扩展文档页用的 function_descriptions.csv（只做转发，逻辑在 cargo CLI 里）
# 描述写在 #[duck_*] 属性上；要连没写描述的函数一起导出：
#   cargo run -p duckfn_quack --bin duckfn -- function_descriptions --all
# 根包 duckfn 没有同名的 bin，所以必须带 -p，否则在根目录会解析失败。
docs_csv:
    cargo run -p duckfn_quack --bin duckfn -- function_descriptions

# ==== 官方 makefile 流程：sqllogictest 与 CI 走这条 ====

# 初始化 extension-ci-tools（生成 configure/ 与 python venv）；只需一次
ci-init:
    make configure

ci-build: ci-init
    make debug

ci-release: ci-init
    make release

# ==== 发版流程（完整步骤见根目录 AGENTS.md） ====

# 发版前检查：clippy 与构建必须无 warning，有问题先修再发版
release_check: lint
    cargo build --workspace

# 提升版本号（项目 + 文档）：just release_bump 0.0.10
release_bump new_version:
    bash scripts/release.sh bump "{{new_version}}"

# 打 tag 并推送，触发 CI 发版：just release_tag 0.0.10
release_tag version:
    bash scripts/release.sh tag "{{version}}"

# 查看最近的 CI 运行状态
release_ci:
    gh run list --limit 5

# 发布两个 crate 到 crates.io（duckfn-macro 必须先上线）
release_publish:
    just publish_macro_dry
    just publish_macro
    just publish_dry
    just publish

# 切到下一开发版本（参数形如 X.Y.Z-dev.0）：just release_dev <新版本>
# 只动根 Cargo.toml 与 Cargo.lock
release_dev new_version:
    bash scripts/release.sh dev "{{new_version}}"

publish_macro_dry:
    cargo publish -p duckfn-macro --registry crates-io --dry-run

publish_macro:
    cargo publish -p duckfn-macro --registry crates-io

publish_dry:
    cargo publish -p duckfn --registry crates-io --dry-run

publish:
    cargo publish -p duckfn --registry crates-io
