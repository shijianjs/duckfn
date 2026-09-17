#set windows-shell := ["pwsh.exe", "-NoLogo","-Command"]

set windows-shell := ["C:\\Program Files\\Git\\bin\\bash.exe", "-c"]

extension := "./target/debug/rusty_quack.duckdb_extension"

release:
  cargo build --release


ext_build:
    cargo duckdb-ext build

duckdb_ext sql: ext_build
    duckdb -unsigned -c "LOAD '{{extension}}'; {{sql}}"

duckdb_ext_debug sql: ext_build
    duckdb -unsigned -cmd "LOAD '{{extension}}'; {{sql}}"

# 这个just命令可以跑所有test/sql/**/*.text测试
test:
    make configure debug test

config_env:
	rustup override set 1.86.0
	rustup target add wasm32-unknown-emscripten
	rustup target list --installed

build_wasm:
	cargo build --release --target wasm32-unknown-emscripten --example rusty_quack

publish_macro_dry:
	cargo publish -p duckfn-macro --registry crates-io --dry-run

publish_macro:
	cargo publish -p duckfn-macro --registry crates-io

publish_dry:
	cargo publish -p duckfn --registry crates-io --dry-run

publish:
	cargo publish -p duckfn --registry crates-io

doc:
	cargo doc -p duckfn


# ==== 发版流程（完整步骤见根目录 AGENTS.md） ====

# 发版前检查：clippy 与构建必须无 warning，有问题先修再发版
release_check:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo build --workspace

# 提升版本号（项目 + 文档）：just release_bump 0.0.5
release_bump new_version:
    bash scripts/release.sh bump "{{new_version}}"

# 打 tag 并推送，触发 CI 发版：just release_tag 0.0.5
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

# 切到下一开发版本：just release_dev 0.0.6-dev.0
release_dev new_version:
    bash scripts/release.sh dev "{{new_version}}"
