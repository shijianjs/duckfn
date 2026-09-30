# duckfn 扩展项目的共享 Justfile 片段 —— 单一来源，**不要在各项目的副本里手改**。
#
# 各项目根 Justfile 长这样：
#
#     set windows-shell := ["C:\\Program Files\\Git\\bin\\bash.exe", "-c"]   # 机器相关，留在本地
#     import "scripts/common.just"
#     extension_name := "<你的扩展名>"          # 项目相关：共享 recipe 靠它算产物路径
#
# 需要在本项目里**覆盖**某条共享 recipe（duckfn 仓库自己给 cargo 加 --features 就是一例）时，
# 再加一行：
#
#     set allow-duplicate-recipes := true
#
# 不开这个开关，重名 recipe 会让 just 直接报错（连 `just --list` 都跑不了）；开了之后是
# 「浅层覆盖深层」—— 根文件里的定义生效，导入文件里的那份被盖掉。
#
# 改共享内容：改 duckfn 仓库（shijianjs/duckfn）里的 scripts/common.just，然后在各项目跑
#
#     just sync-common                      # 默认跟 main
#     DUCKFN_JUST_REF=vX.Y.Z just sync-common   # 也可以钉到某个已发布版本
#     just check-common                     # 只比对不写回，不一致时非零退出（发版前顺手跑一次）
#
# 副本与源逐字节相同，「这次同步改了什么」看 git diff 即可。文件里不写具体版本号、一律用 X.Y.Z
# 占位 —— 否则每次发版都会在副本里制造一行噪音。
#
# Shared justfile excerpt for duckfn extension projects: one source of truth. A project's root
# justfile imports it as `scripts/common.just` and defines `extension_name`; content changes go to the
# duckfn repository's copy and are pulled back with `just sync-common`. Overriding a shared recipe
# from the root requires `set allow-duplicate-recipes := true` (shallower definitions win).

# ==== 共享内容的来源与同步 ====

# duckfn 仓库 + git ref；换 ref 即可钉住某个已发布版本。
duckfn_repo := "shijianjs/duckfn"
duckfn_just_ref := env_var_or_default("DUCKFN_JUST_REF", "main")
_common_url := "https://raw.githubusercontent.com/" + duckfn_repo + "/" + duckfn_just_ref + "/scripts/common.just"

# duckdb 命令行。不在 PATH 里时用 `just DUCKDB=/path/to/duckdb repl` 覆盖。
duckdb := env_var_or_default("DUCKDB", "duckdb")

# 扩展构建产物路径。`extension_name` 由各项目根 Justfile 定义 —— import 是同一个作用域，
# 且 just 对顺序不敏感，所以导入文件引用导入方的变量没问题。
#
# The extension artifact path. `extension_name` comes from the importing justfile: an import shares one
# scope and just is order-insensitive, so forward references across files are fine.
ext_path := "./target/debug/" + extension_name + ".duckdb_extension"

# ==== 共享内容的同步（改完共享文件后在各项目跑） ====

# 先下载、替换，再验证一次 `just --list`：万一同步到一份坏文件，`just` 在本项目里就什么都跑不了
# （包括这条 recipe 自己），所以坏文件一律回滚。中间文件在两条失败路径上都会删掉。
#
# Pull the canonical scripts/common.just from the duckfn repository (main by default). The download is
# validated with `just --list` after the swap and rolled back when it does not parse, because a broken
# shared file would make every `just` invocation in this project fail.
#
# 把 scripts/common.just 同步成 duckfn 仓库里的那份（默认 main）
sync-common:
    curl -fsSL "{{_common_url}}" -o scripts/common.just.new || { rm -f scripts/common.just.new; echo "error: 拉取失败，本地文件未改动 / download failed, local copy untouched"; exit 1; }
    mv scripts/common.just scripts/common.just.bak
    mv scripts/common.just.new scripts/common.just
    if just --list >/dev/null 2>&1; then rm -f scripts/common.just.bak; else mv scripts/common.just.bak scripts/common.just; echo "error: 同步后的 scripts/common.just 解析失败，已回滚 / rolled back"; exit 1; fi
    @echo "scripts/common.just <- {{_common_url}}"
    @git --no-pager diff --stat -- scripts/common.just

# Compare only: fails when the local copy differs from {{duckfn_just_ref}}. Run it before a release,
# or wire it into your own CI.
#
# 只比对不写回：本地副本与共享源（默认 main）不一致时报错
check-common:
    bash -c 'set -euo pipefail; tmp="$(mktemp)"; trap "rm -f $tmp" EXIT; curl -fsSL "{{_common_url}}" -o "$tmp"; if diff -u scripts/common.just "$tmp"; then echo "scripts/common.just 与 {{duckfn_just_ref}} 一致 / in sync"; else echo "scripts/common.just 与 {{duckfn_just_ref}} 不同步，跑 just sync-common / out of sync, run just sync-common"; exit 1; fi'

# ==== 日常 ====

# 不带参数运行 just 时列出所有 recipe
default:
    @just --list

# 日常构建 -> target/debug/<extension_name>.duckdb_extension
build:
    cargo duckdb-ext build

# 构建后跑一条 SQL 就退出：just sql "SELECT my_greet('world')"
sql sql: build
    {{duckdb}} -unsigned -c "LOAD '{{ext_path}}'; {{sql}}"

# 构建后进入 REPL（扩展已 LOAD），手动试函数用：just repl
repl: build
    {{duckdb}} -unsigned -cmd "LOAD '{{ext_path}}';"

# 全量 release 构建（注意：与下面的 release_* 发版流程不是一回事）
release:
    cargo build --release

# 提交前检查，warning 视为错误
lint:
    cargo clippy --all-targets -- -D warnings

# ==== 函数描述 CSV ====
#
# 描述写在 #[duck_*] 属性的 description / comment / example 上，输出固定为 target/function_descriptions.csv；
# 要连没写描述的函数一起导出（文件名带 _all 后缀）：cargo run --bin duckfn -- function_descriptions --all
# 需要 src/bin/duckfn.rs 与 Cargo.toml 里的 duckfn feature "cli"。
#
# The text comes from the description / comment / example arguments of the #[duck_*] attributes and always
# lands in target/function_descriptions.csv. Add --all (file name gets an _all suffix) to include functions
# without any documentation. Needs src/bin/duckfn.rs and duckfn's "cli" feature in Cargo.toml.
#
# 生成社区扩展文档页用的 function_descriptions.csv（只做转发，逻辑在 duckfn 的 cargo CLI 里）
docs_csv:
    cargo run --bin duckfn -- function_descriptions

# ==== 文档站（docs/，Docusaurus，中英双语） ====
#
# 首次先 `just docs_install` 装依赖。CI 走 `npm ci`，本地装一次即可。
# 站点维护（目录、翻译、部署）见 docs/README.md。
#
# The documentation site (docs/, Docusaurus, English + Simplified Chinese). Run `just docs_install`
# once; CI uses `npm ci`. See docs/README.md for layout, translations and deployment.

# 装文档站依赖（只做一次）
docs_install:
    cd docs && npm install

# 构建静态站点 -> docs/build（改完文档想确认链接都还通时跑它）
#
# 前置：文档站预加载的扩展本地取自 static/duckdb-extensions/，所以先 `just build_wasm_eh`
# （或直接 `just test_wasm`）；没有那个文件构建会报错。
docs_build:
    cd docs && npm run build

# 本地预览文档站（http://localhost:3000；中文用 npm start -- --locale zh-Hans）
#
# 前置同上：先 `just build_wasm_eh` 把扩展放进 static/duckdb-extensions/。
docs_start:
    cd docs && npm start

# 补翻译占位：改了 config / src / _category_.json 之后再生一次，然后填新出现的条目
docs_translations:
    cd docs && npx docusaurus write-translations --locale zh-Hans

# ==== 官方 makefile 流程：sqllogictest 与 CI 走这条 ====

# 初始化 extension-ci-tools（生成 configure/ 与 python venv）；跑 make 流程前先来一次
ci-init:
    make configure

# 官方 debug 构建 —— sqllogictest 与 CI 走这条
ci-build: ci-init
    make debug

# 收尾的 `git clean -fdX -- test/sql` 是必要的：用例自己会往 test/sql 写临时文件
# （dfn_file_*、dfn_copy_*.tsv 等，被 .gitignore 忽略），而 duck_vfs 那条用例要求目标文件
# **不存在**才写 —— 不清掉上一次的残留，本机重跑就会失败。`-X` 只删被忽略的文件，
# tracked 的 *.test 不受影响。
#
# The trailing `git clean -fdX -- test/sql` is load-bearing: the tests write ignored scratch files
# there (dfn_file_*, dfn_copy_*.tsv, ...) and the duck_vfs case needs its target file to be absent, so
# a local re-run fails without the cleanup. `-X` removes ignored files only; tracked *.test stay.
#
# 跑 sqllogictest：test/sql/**/*.test（收尾会清掉上一次留下的临时文件）
test: ci-build
    make test
    git clean -fdX -- test/sql

# 官方 release 构建 —— CI 打 tag 时走这条
ci-release: ci-init
    make release

# ==== WebAssembly ====

# WebAssembly 构建；要求 Cargo.toml 的 [[example]] name 与 extension_name 一致
build_wasm:
    cargo build --release --target wasm32-unknown-emscripten --example {{extension_name}}

# 打出可加载的 wasm_eh 扩展 -> build/wasm_eh/extension/<extension_name>/<extension_name>.duckdb_extension.wasm
#
# 走官方 makefile 的三个目标（configure → release → move_wasm_extension）：
# cargo 出 lib<name>.a → emcc 出 side module → append_extension_metadata 出 .duckdb_extension.wasm。
#
# 这里把上游 `wasm_eh` 目标**摊开**执行，而不是 `make wasm_eh`：那条目标自己又调一次 `make`
# （`DUCKDB_PLATFORM=wasm_eh make configure release move_wasm_extension`），而 just 在 Windows 上
# 用 windows-shell 起的是**非登录** Git Bash，内层 make 的 recipe shell（/usr/bin/sh）拿到的 PATH
# 里没有 scoop 的 shims，于是报 `make: command not found`。摊开后只有一层 make，从 PowerShell / Git
# Bash / `just` 里跑都成立（登录 shell 里 PATH 完整，原来的写法只是碰巧能跑）。
#
# 前提：系统装好 emsdk 3.1.71 并把它的目录配进 PATH（`emcc` / `emcc.bat` 能直接执行）。版本必须与
# CI 一致（见 _extension_distribution.yml），否则产物能构建出来、LOAD 时报 Could not load dynamic lib。
# 不需要 source emsdk_env.sh —— emcc 已在 PATH 上就行。
#
# RUST_LIBNAME 覆盖是必须的：上游 rust.Makefile 按宿主 OS 取产物名，Windows 上会去找 <name>.dll，
# 而 wasm 产物是 lib<name>.a。少了它 make 会在拷贝那步报文件不存在。
#
# 注意：跑完 configure/platform.txt 会停在 wasm_eh；之后要跑本地原生的 make test 之前先 `make configure`。
#
# Loadable wasm_eh build via the upstream makefile targets. They are spelled out instead of calling
# `make wasm_eh`, because that upstream target re-invokes `make` and just's non-login Git Bash on
# Windows hands the inner recipe shell a PATH without the shims (`make: command not found`).
# The emsdk/emscripten on PATH must match CI (3.1.71). `RUST_LIBNAME` is the override that makes it
# work on Windows, where the makefile expects the native dll instead of the wasm target's
# lib<name>.a.
build_wasm_eh:
    DUCKDB_PLATFORM=wasm_eh make configure release move_wasm_extension RUST_LIBNAME=lib{{extension_name}}.a

# 一条命令验证文档站里的 wasm 侧示例：构建 wasm_eh 扩展 → 放进文档站的 static/ → 跑遍每个
# 可运行 SQL 块（在真实浏览器的 DuckDB-Wasm 里执行）。产物落到
# docs/static/duckdb-extensions/<extension_name>.duckdb_extension.wasm。
#
# 文档站里的可运行块本地**就用这个文件** —— 只有 GitHub Pages 那次部署才去拉 release（由
# DOCS_EXTENSION_FROM_RELEASE 区分，见 docs/docusaurus.config.ts），所以本地别单跑
# `just docs_build` / `docs_start`：static/ 下没有产物它们会直接报错。要试文档里的 SQL 就跑这条。
#
# 前提同 build_wasm_eh：emsdk 与 CI 同版本且在 PATH 上。
#
# Build the wasm_eh extension, drop it into the docs site's static directory and run every runnable
# SQL block in a real browser. Locally the docs site preloads *that* file — only the GitHub Pages
# deployment fetches the release — so this is the entry point for checking the docs on a
# workstation; `docs_build` / `docs_start` on their own fail without it.
#
# 末尾用 `cd docs && npm test` 而不是 `npm test -w docs`：这份文件在下游项目里也跑，而那些项目没有
# 根 package.json（没有 npm workspace），`-w` 会直接报错。两者在 duckfn 仓库里等价。
test_wasm: build_wasm_eh
    mkdir -p docs/static/duckdb-extensions
    cp build/wasm_eh/extension/{{extension_name}}/{{extension_name}}.duckdb_extension.wasm docs/static/duckdb-extensions/{{extension_name}}.duckdb_extension.wasm
    cd docs && npm test

# 工具链（首次）：固定 Rust 版本 + 装 wasm target
config_env:
    rustup override set 1.86.0
    rustup target add wasm32-unknown-emscripten
    rustup target list --installed

# ==== 发版流程（完整步骤见根目录 AGENTS.md） ====
#
# Release flow (the full walkthrough lives in AGENTS.md). Downloadable DuckDB extensions are
# distributed as the `.duckdb_extension` files attached to a GitHub Release; whether the crate also
# goes to crates.io is up to the project.

# 发版前检查：clippy（warning 视为错误）与构建都必须干净
release_check: lint
    cargo build --all-targets

# 提升版本号（Cargo.toml + 文档 / README / CI 注释）：just release_bump X.Y.Z
release_bump new_version:
    bash scripts/release.sh bump "{{new_version}}"

# 打 tag 并推送，触发 CI 构建与 Release 发布：just release_tag X.Y.Z
release_tag version:
    bash scripts/release.sh tag "{{version}}"

# 查看最近的 CI 运行状态
release_ci:
    gh run list --limit 5

# 切到下一开发版本（不打 tag、不发布）：just release_dev X.Y.Z-dev.0
release_dev new_version:
    bash scripts/release.sh dev "{{new_version}}"
