.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# DuckDB 扩展名。必须与这三处保持一致：
#   test/extension/entry.rs 的 duckfn_entrypoint!("...")
#   test/sql/**/*.test   的 `require ...`
#   .github/workflows/MainDistributionPipeline.yml 的 extension_name / EXTENSION_NAME
#
# 它与 crate 名（包名就是 duckfn）一致不是巧合：原生扩展就是本包的 cdylib，产物名由 crate 名决定
# （libduckfn.so / duckfn.dll / wasm 的 libduckfn.a），而上游 rust.Makefile 是按
# lib$(EXTENSION_NAME).* 推这个名字的 —— 对齐之后这里一行平台条件都不用写。
#
# The DuckDB extension name. It has to stay in sync with duckfn_entrypoint! in
# test/extension/entry.rs, the `require` lines of test/sql/**/*.test and extension_name / EXTENSION_NAME
# in .github/workflows/MainDistributionPipeline.yml. Its matching the crate name (the package is
# `duckfn`) is no accident: the native extension *is* this package's cdylib, so the artifact is named
# after the crate (libduckfn.so / duckfn.dll / libduckfn.a for wasm), while the upstream
# rust.Makefile derives the file it copies from lib$(EXTENSION_NAME).* — aligning the two saves every
# platform-specific override.
EXTENSION_NAME=duckfn

# 构建扩展时打开的 duckfn feature（逗号分隔，原样传给 cargo 的 --features）。示例树按**能力**分模块
# （每个能力模块挂自己的 #[cfg(feature = ...)]，见 test/extension/functions/mod.rs 与 Cargo.toml 的
# `quack`），所以这里列出来的就是「这份产物演示了哪些能力」：
#
#   quack              示例树本身（标记式 feature：只决定编不编那棵树）
#   duckdb-1-5         C API 不稳定区：COPY 函数与 TIME_NS 示例
#   duck_vfs           宿主文件系统（依赖 owned-connection 与 duckdb-1-5）
#   sql                在回调里执行 SQL（只用稳定区）
#   chrono / uuid / rust_decimal   三个互操作桥的示例
#
# 换个组合就是另一份产物 —— 例如 EXTENSION_FEATURES=quack,sql 得到一份**只用稳定版 C API** 的插件
# （`sql` 跑在回调侧但每个调用都在稳定区，所以它属于这一档），配 USE_UNSTABLE_C_API=0 与
# TARGET_DUCKDB_VERSION=v1.2.0（C API 下限）打出来，1.3 到 2.0 的引擎都能加载，可以用来跑老引擎上的
# 用例子集（`just test_stable 1.4.0`，CI 每个 PR 也跑这一份）。
#
# The duckfn features a build turns on (comma-separated, passed through to cargo `--features`). The
# example tree splits itself by **capability** (each capability module carries its own
# `#[cfg(feature = ...)]` — see test/extension/functions/mod.rs and `quack` in Cargo.toml), so this
# list is exactly "which capabilities this artifact demonstrates":
#
#   quack              the example tree itself (a marker feature: it only decides whether it compiles)
#   duckdb-1-5         the unstable C API region: the COPY demos and TIME_NS
#   duck_vfs           the host file system (needs owned-connection and duckdb-1-5)
#   sql                running SQL from a callback (stable region only)
#   chrono / uuid / rust_decimal   the three bridge demos
#
# Another combination is another artifact — `EXTENSION_FEATURES=quack,sql` with USE_UNSTABLE_C_API=0 and
# TARGET_DUCKDB_VERSION=v1.2.0 (the C API floor) gives a **stable C API only** plugin (`sql` runs from a
# callback but every call it makes stays in the stable region, so it belongs to this side) that every
# engine from 1.3 to 2.0 loads, which is what runs a subset of the suite against an older engine
# (`just test_stable 1.4.0`; CI runs the same thing on every pull request).
# Default: stable C API only; the distribution keeps it that way so one artifact covers 1.3–2.0.
# The unstable region (duckdb-1-5 / duck_vfs) is opt-in and, since quack-rs 0.19, is shipped as
# `C_STRUCT` with a v1.5.6 C API floor rather than `C_STRUCT_UNSTABLE` — DuckDB 2.0 keeps the 1.5.6
# v1 layout and only appends two slots, which quack-rs' ABI check accepts. `C_STRUCT_UNSTABLE` is out:
# 2.0 would demand the v2 entrypoint (`<name>_init_c_api_v2`), which quack-rs still does not emit.
# For a local full-featured build against the 2.0 headers this Cargo.lock resolves (2.0-only, because
# those headers carry the 548-slot layout):
#   make USE_UNSTABLE_C_API=0 TARGET_DUCKDB_VERSION=v1.5.6 DUCKDB_TEST_VERSION=2.0.0 \
#        EXTENSION_FEATURES=quack,duckdb-1-5,duck_vfs,sql,chrono,uuid,rust_decimal debug
# To get one artifact for 1.5.6 *and* 2.0.0, pin libduckdb-sys to 1.10506.0 first (that is the
# build quack-rs 0.19 recommends; the layout table maps 546 slots onto 2.0's 548).
EXTENSION_FEATURES ?= quack,sql,chrono,uuid,rust_decimal

# Set to 1 to enable Unstable API (binaries will only work on TARGET_DUCKDB_VERSION, forwards compatibility will be broken)
# Note: currently extension-template-rs requires this, as duckdb-rs relies on unstable C API functionality
#
# 这不是「开不开不稳定区能力」的开关 —— 那是 EXTENSION_FEATURES 里列不列 duckdb-1-5 / duck_vfs。它只
# 决定元数据声明哪种 ABI：置 1 得到 `C_STRUCT_UNSTABLE`（把产物钉死在 TARGET_DUCKDB_VERSION 这一版），
# 而 quack-rs 0.19 起不稳定区构建应改用 `C_STRUCT` + v1.5.6 下限（见上面的 EXTENSION_FEATURES 注释），
# 所以本仓库两种产物都保持 0。
#
# This is not the "unstable capabilities" switch — that is whether EXTENSION_FEATURES lists
# duckdb-1-5 / duck_vfs. It only picks the metadata ABI: 1 stamps `C_STRUCT_UNSTABLE`, pinning the
# artifact to TARGET_DUCKDB_VERSION exactly. Since quack-rs 0.19 an unstable-region build should use
# `C_STRUCT` with a v1.5.6 floor instead (see the EXTENSION_FEATURES note above), so both artifacts
# here keep it at 0.
USE_UNSTABLE_C_API ?= 0

# Target DuckDB version
#
# 在 USE_UNSTABLE_C_API=1 下它是**发行版**（产物只在这一版引擎上加载）；在 =0 下它是**C API 下限**
# （产物在任何 C API 不低于它的引擎上加载）。稳定区构建写 v1.2.0；不稳定区构建写 v1.5.6 —— 那是
# DuckDB 2.0 仍在沿用的 v1 布局起点，1.5.6 与 2.0.0 都收。
#
# With USE_UNSTABLE_C_API=1 this is a **release** (the artifact only loads on that engine); with =0 it
# is a **C API floor** (the artifact loads on any engine whose C API is at or above it). A stable build
# writes v1.2.0; an unstable-region build writes v1.5.6, the v1 layout DuckDB 2.0 still keeps, which
# both 1.5.6 and 2.0.0 accept.
# The distribution build is stable (USE_UNSTABLE_C_API=0), so this is a C API floor, not a release:
# v1.2.0 is accepted by DuckDB 1.3 through 2.0. An unstable-region build raises the floor to v1.5.6
# instead of stamping C_STRUCT_UNSTABLE (quack-rs 0.19).
TARGET_DUCKDB_VERSION ?= v1.2.0

# quack-rs 只在不稳定路径上读这个变量（稳定区构建里 `abi::check()` 先返回 `StableOnly`），且它声明的是
# 「头文件来自哪一版发行版」，必须与 libduckdb-sys 实际解析到的版本一致。只有遗留的
# `C_STRUCT_UNSTABLE` 模式还把产物钉死在 TARGET_DUCKDB_VERSION 上，所以只在那里导出；`C_STRUCT` +
# v1.5.6 下限的构建不导出 —— 若解析到的是 2.0 头文件（548 槽），声明 v1.5.6（546 槽）反而会被 quack-rs
# 判成 DeclaredVersionMismatch。
#
# quack-rs reads this variable only on the unstable path (a stable build's `abi::check()` returns
# `StableOnly` first), and it declares *which release the headers came from*, so it has to agree with
# what libduckdb-sys actually resolved. Only the legacy `C_STRUCT_UNSTABLE` mode pins the artifact to
# TARGET_DUCKDB_VERSION, hence the export only there. A `C_STRUCT` + v1.5.6-floor build does not export
# it: if the resolved bindings are the 2.0 headers (548 slots), declaring v1.5.6 (546 slots) would be a
# DeclaredVersionMismatch.
ifeq ($(USE_UNSTABLE_C_API),1)
export QUACK_RS_TARGET_DUCKDB_VERSION=$(TARGET_DUCKDB_VERSION)
endif

# 测试用的 DuckDB 版本：`C_STRUCT_UNSTABLE` 的产物被钉死在精确版本上（加载时校验），所以那时它必须与
# TARGET_DUCKDB_VERSION 一致；`C_STRUCT` 产物（稳定区，以及 v1.5.6 下限的不稳定区）不受这条约束，任何
# 不低于下限的引擎都收。这里默认用当前发行版引擎 —— 它既是分发流水线构建的那版，也是 quack-rs 0.19
# 声称能跑不稳定区构建的那版。要验证更老的引擎就显式给一个：
#   make DUCKDB_TEST_VERSION=1.4.0 test_debug
#   make DUCKDB_TEST_VERSION=1.5.6 TARGET_DUCKDB_VERSION=v1.5.6 ... test_debug   # 需 1.5.6 头文件
#
# The engine the test runner uses. A `C_STRUCT_UNSTABLE` artifact is pinned to an exact release
# (verified at load time), so there it has to match TARGET_DUCKDB_VERSION. A `C_STRUCT` artifact — the
# stable build and the v1.5.6-floor unstable one — is not bound to it: any engine at or above the
# floor accepts it. This defaults to the current release engine, which is both what the distribution
# pipeline builds for and the engine quack-rs 0.19 claims for unstable-region builds. Pass an older one
# explicitly to check a range: `make DUCKDB_TEST_VERSION=1.4.0 test_debug`.
DUCKDB_TEST_VERSION ?= 2.0.0

all: configure debug

# Include makefiles from DuckDB
#
# 这个 Makefile 必须留在仓库根目录：CI 的 extension-ci-tools/scripts/ci_phase.py 一律在根目录执行
# `make configure_ci|debug|release|test_*|upload`，上游工作流不支持自定义工作目录。
#
# 示例扩展已经并进根包（test/extension/），于是 rust.Makefile 里那条裸构建命令 `cargo build` 正好命中
# 本包的 lib（rlib + cdylib），所以这里不需要覆盖任何 recipe。唯一要补的是 feature：示例树挂在标记式
# 的 `quack` 上，它要演示的能力由 EXTENSION_FEATURES 列出（两者默认关闭，下游依赖树才不受影响），
# 而上游 recipe 没给 feature 留位置 —— 只有 TARGET_INFO 会被原样拼进 `cargo build`，因此在 include
# 之后追加一次。wasm 那边稍特殊：上游默认会带
# `--example $(EXTENSION_NAME)`，而本包没有这个 example 目标，产物来自 lib 的 staticlib —— 见下面
# TARGET_INFO 的 wasm 分支，它把整条 TARGET_INFO 换掉，`--example` 随之消失。
#
# 用例目录不用改：上游默认就是 `--test-dir test/sql`，sqllogictest 现在正好回到仓库根的 test/sql/。
#
# This Makefile has to stay at the repository root: CI's extension-ci-tools/scripts/ci_phase.py
# always runs `make configure_ci|debug|release|test_*|upload` from the root, and the upstream
# workflow offers no way to set a different working directory.
#
# The example extension now lives inside the root package (test/extension/), so the two bare build
# commands in rust.Makefile — `cargo build`, plus `--example $(EXTENSION_NAME)` for wasm — hit this
# package's lib (rlib + cdylib) and its `[[example]] duckfn` respectively, which is why no recipe has
# to be overridden here. The one thing to add is the features: the example tree sits behind the marker
# `quack`, and the capabilities it should demonstrate come from EXTENSION_FEATURES (both off by
# default, so a downstream dependency tree is unaffected), while the upstream recipes leave no slot for
# features. TARGET_INFO is the single variable they splice verbatim into `cargo build` (empty
# natively, `--target`/`--example` for wasm), so it is appended to right after the includes.
#
# The test directory needs no override either: the upstream default is `--test-dir test/sql`, and the
# sqllogictest suite is now back at the repository root's test/sql/.
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

# 覆盖 base.Makefile 的 wasm 链接：把 `-O3` 换成 `-O0`。
#
# 必须和 base 一样套 `ifneq ($(DUCKDB_WASM_PLATFORM),)` 守卫：base 只在 wasm 平台把 link_wasm_* 定义成
# emcc 命令，原生平台走 else 分支得到**空的** link_wasm_*。若无条件重定义，会把原生那条空目标也改成
# emcc，于是 linux/macos/windows 构建时报 `emcc: command not found`（Error 127）。
#
# 换 -O0 的原因：本仓库用 rust-toolchain.toml 钉 Rust 1.97.1（≥1.89），产出的 wasm 模块可能声明
# `call-indirect-overlong` 这类较新的提案，而 CI 钉死的 emsdk 3.1.71（binaryen/wasm-opt v120）解析不了，
# `-O3` 的优化步直接 `Unknown option` 失败。wasm-opt 只是优化器：`-O0` 跳过它，wasm-ld 的产物本身合法、
# DuckDB-Wasm 照样加载运行，只是没做体积优化。
#
# 这个守卫块与 rust-toolchain.toml 是一套：哪天不再钉 Rust（比如 DuckDB 2.0 的官方流水线上到 1.97、
# 且 emsdk 的 binaryen 跟上了新提案），两个一起删。
#
# Same `ifneq` guard as base.Makefile: base only defines link_wasm_* as emcc for wasm platforms and
# leaves them empty for native, so an unconditional redefinition would make native builds call `emcc`
# (command not found, Error 127). The `-O0` swap: this repository pins Rust 1.97.1 in
# rust-toolchain.toml, and Rust >= 1.89 can emit a module declaring `call-indirect-overlong`, which the
# pinned emsdk 3.1.71 (binaryen/wasm-opt v120) cannot parse — so the `-O3` post-link step dies on
# "Unknown option". wasm-opt is only an optimizer: `-O0` skips it and wasm-ld's output stays valid and
# loadable, just unoptimised. This block and rust-toolchain.toml go together — remove both when neither
# is needed.
ifneq ($(DUCKDB_WASM_PLATFORM),)
link_wasm_debug:
	emcc $(EXTENSION_BUILD_PATH)/debug/$(EXTENSION_LIB_FILENAME) -o $(EXTENSION_BUILD_PATH)/debug/$(EXTENSION_FILENAME_NO_METADATA) -O0 -g -sSIDE_MODULE=2 -sEXPORTED_FUNCTIONS="_$(EXTENSION_NAME)_init_c_api"

link_wasm_release:
	emcc $(EXTENSION_BUILD_PATH)/release/$(EXTENSION_LIB_FILENAME) -o $(EXTENSION_BUILD_PATH)/release/$(EXTENSION_FILENAME_NO_METADATA) -O0 -sSIDE_MODULE=2 -sEXPORTED_FUNCTIONS="_$(EXTENSION_NAME)_init_c_api"
endif

# 构建参数：
#   - native / macOS 交叉编译：上游给的是空值（或 `--target <triple>`），追加 feature 即可。
#   - wasm：扩展产物来自 lib 的 staticlib（名字自动就是 lib$(EXTENSION_NAME).a），所以上游给的
#     `--example $(EXTENSION_NAME)` 不再需要，改成只带 --target；IS_EXAMPLE 也清掉 —— 产物在
#     target/<target>/<profile>/ 根下，而不是 examples/ 子目录里。
#
# Build arguments:
#   - native / macOS cross-compile: the upstream value is empty (or `--target <triple>`), so the
#     feature is simply appended.
#   - wasm: the artefact is the lib's staticlib (already named lib$(EXTENSION_NAME).a), so the
#     upstream `--example $(EXTENSION_NAME)` is dropped and only `--target` is kept; IS_EXAMPLE is
#     cleared as well, because the artefact sits at the root of target/<target>/<profile>/ instead of
#     in an examples/ subdirectory.
# 扩展产物（cdylib / staticlib）来自 `[[example]] duckfn`，两个分支都要带上 `--example`
# 并把 IS_EXAMPLE 指到 examples/ —— 与上游 rust.Makefile 的默认约定一致。
#
# The extension artefact (cdylib / staticlib) comes from `[[example]] duckfn`, so both branches pass
# `--example` and point IS_EXAMPLE at examples/ — the same convention the upstream rust.Makefile uses.
ifneq ($(DUCKDB_WASM_PLATFORM),)
TARGET_INFO := --target $(TARGET) --example $(EXTENSION_NAME) --features $(EXTENSION_FEATURES)
IS_EXAMPLE := /examples
else
TARGET_INFO += --example $(EXTENSION_NAME) --features $(EXTENSION_FEATURES)
IS_EXAMPLE := /examples
endif

# 用例范围：默认是 --test-dir test/sql（整棵树），EXTRA_TEST_ARGS 原样追加给 runner，用来只跑一部分
# 用例（老引擎上跑稳定区子集就是这么跑的，见 `just test_stable`）：
#
#   make test_debug EXTRA_TEST_ARGS="--file-path test/sql/functions/scalar_function.test"
#   make test_debug EXTRA_TEST_ARGS="--file-list target/stable-tests.txt"   # 空白分隔的路径清单
#
# runner（duckdb-sqllogictest-python 的 python_runner.py）的参数是 --file-path / --test-dir /
# --file-list；给了 --file-path 或 --file-list 就按列出来的文件跑（--test-dir 仍作为工作目录传下去）。
#
# The test range: `--test-dir test/sql` (the whole tree) by default, with EXTRA_TEST_ARGS appended
# verbatim so a run can cover a subset — that is how the stable-region subset runs on an older engine
# (see `just test_stable`). The runner's own options are --file-path / --test-dir / --file-list
# (duckdb-sqllogictest-python's python_runner.py): with --file-path or --file-list it runs exactly the
# files listed, while --test-dir still names the working directory.
EXTRA_TEST_ARGS ?=
TEST_RUNNER_BASE := $(TEST_RUNNER) --test-dir test/sql $(EXTRA_EXTENSIONS_PARAM) $(EXTRA_TEST_ARGS)

# 只有不含不稳定区能力的产物才跑稳定子集：判据是 EXTENSION_FEATURES 里有没有 duckdb-1-5* / duck_vfs，
# 而不是 USE_UNSTABLE_C_API —— quack-rs 0.19 起不稳定区构建也 stamp C_STRUCT（v1.5.6 下限），那个变量
# 不再是它的标志。`scripts/test-stable.sh` 自己传 EXTRA_TEST_ARGS，因此绕过这段。
#
# filter 按空白分词，而 EXTENSION_FEATURES 是逗号分隔的一整个词，所以先把逗号换成空格再 filter。
#
# Only an artifact without unstable-region capabilities runs the stable subset. The test is whether
# EXTENSION_FEATURES names duckdb-1-5* / duck_vfs, not USE_UNSTABLE_C_API: since quack-rs 0.19 an
# unstable-region build is also stamped C_STRUCT (v1.5.6 floor), so that variable no longer marks it.
# `scripts/test-stable.sh` passes its own EXTRA_TEST_ARGS and therefore bypasses this. filter splits on
# whitespace while EXTENSION_FEATURES is one comma-separated word, hence the comma-to-space step.
comma := ,
EXTENSION_FEATURE_WORDS := $(subst $(comma), ,$(EXTENSION_FEATURES))
ifeq ($(strip $(filter duckdb-1-5% duck_vfs,$(EXTENSION_FEATURE_WORDS))),)
STABLE_TEST_LIST := target/stable-default-tests.txt

.PHONY: stable_test_list
stable_test_list:
	@$(PYTHON_VENV_BIN) scripts/stable_test_list.py $(STABLE_TEST_LIST)

test_extension_debug_internal: stable_test_list
test_extension_release_internal: stable_test_list

ifeq ($(strip $(EXTRA_TEST_ARGS)),)
TEST_RUNNER_DEBUG := $(TEST_RUNNER_BASE) --external-extension build/debug/$(EXTENSION_NAME).duckdb_extension --file-list $(STABLE_TEST_LIST)
TEST_RUNNER_RELEASE := $(TEST_RUNNER_BASE) --external-extension build/release/$(EXTENSION_NAME).duckdb_extension --file-list $(STABLE_TEST_LIST)
endif
endif

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

test: test_debug
test_debug: test_extension_debug
test_release: test_extension_release

clean: clean_build clean_rust
clean_all: clean_configure clean
