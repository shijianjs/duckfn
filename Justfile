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

# 这个just命令可以跑所有.text测试
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