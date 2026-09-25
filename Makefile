.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# DuckDB 扩展名。必须与这三处保持一致：
#   src/extension/entry.rs 的 duckfn_entrypoint!("...")
#   test/sql/**/*.test   的 `require ...`
#   .github/workflows/MainDistributionPipeline.yml 的 extension_name / EXTENSION_NAME
#
# 它与 crate 名（包名就是 duckfn）一致不是巧合：原生扩展就是本包的 cdylib，产物名由 crate 名决定
# （libduckfn.so / duckfn.dll / wasm 的 libduckfn.a），而上游 rust.Makefile 是按
# lib$(EXTENSION_NAME).* 推这个名字的 —— 对齐之后这里一行平台条件都不用写。
#
# The DuckDB extension name. It has to stay in sync with duckfn_entrypoint! in
# src/extension/entry.rs, the `require` lines of test/sql/**/*.test and extension_name / EXTENSION_NAME
# in .github/workflows/MainDistributionPipeline.yml. Its matching the crate name (the package is
# `duckfn`) is no accident: the native extension *is* this package's cdylib, so the artifact is named
# after the crate (libduckfn.so / duckfn.dll / libduckfn.a for wasm), while the upstream
# rust.Makefile derives the file it copies from lib$(EXTENSION_NAME).* — aligning the two saves every
# platform-specific override.
EXTENSION_NAME=duckfn

# Set to 1 to enable Unstable API (binaries will only work on TARGET_DUCKDB_VERSION, forwards compatibility will be broken)
# Note: currently extension-template-rs requires this, as duckdb-rs relies on unstable C API functionality
USE_UNSTABLE_C_API=1

# Target DuckDB version
TARGET_DUCKDB_VERSION=v1.5.5

all: configure debug

# Include makefiles from DuckDB
#
# 这个 Makefile 必须留在仓库根目录：CI 的 extension-ci-tools/scripts/ci_phase.py 一律在根目录执行
# `make configure_ci|debug|release|test_*|upload`，上游工作流不支持自定义工作目录。
#
# 示例扩展已经并进根包（src/extension/），于是 rust.Makefile 里那两条裸构建命令 —— `cargo build`，
# wasm 时再加 `--example $(EXTENSION_NAME)` —— 正好分别命中本包的 lib（rlib + cdylib）与
# `[[example]] duckfn`，所以这里不需要覆盖任何 recipe。唯一要补的是 feature：示例挂在 `quack` 上
# （默认关闭，下游依赖树才不受影响），而上游 recipe 没给 feature 留位置 —— 只有 TARGET_INFO 会被
# 原样拼进 `cargo build`（native 为空，wasm 是 --target/--example），因此在 include 之后追加一次。
#
# 用例目录不用改：上游默认就是 `--test-dir test/sql`，sqllogictest 现在正好回到仓库根的 test/sql/。
#
# This Makefile has to stay at the repository root: CI's extension-ci-tools/scripts/ci_phase.py
# always runs `make configure_ci|debug|release|test_*|upload` from the root, and the upstream
# workflow offers no way to set a different working directory.
#
# The example extension now lives inside the root package (src/extension/), so the two bare build
# commands in rust.Makefile — `cargo build`, plus `--example $(EXTENSION_NAME)` for wasm — hit this
# package's lib (rlib + cdylib) and its `[[example]] duckfn` respectively, which is why no recipe has
# to be overridden here. The one thing to add is the feature: the example sits behind `quack` (off by
# default, so a downstream dependency tree is unaffected) and the upstream recipes leave no slot for
# features. TARGET_INFO is the single variable they splice verbatim into `cargo build` (empty
# natively, `--target`/`--example` for wasm), so it is appended to right after the includes.
#
# The test directory needs no override either: the upstream default is `--test-dir test/sql`, and the
# sqllogictest suite is now back at the repository root's test/sql/.
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

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
ifneq ($(DUCKDB_WASM_PLATFORM),)
TARGET_INFO := --target $(TARGET) --features quack
IS_EXAMPLE :=
else
TARGET_INFO += --features quack
endif

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

test: test_debug
test_debug: test_extension_debug
test_release: test_extension_release

clean: clean_build clean_rust
clean_all: clean_configure clean
