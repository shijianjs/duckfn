.PHONY: clean clean_all

PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# DuckDB 扩展名。必须与这三处保持一致：
#   duckfn-quack/src/extension/mod.rs 的 duckfn_entrypoint!("...")
#   duckfn-quack/test/sql/**/*.test   的 `require ...`
#   .github/workflows/MainDistributionPipeline.yml 的 extension_name / EXTENSION_NAME
#
# The DuckDB extension name. It has to stay in sync with duckfn_entrypoint! in
# duckfn-quack/src/extension/mod.rs, the `require` lines of duckfn-quack/test/sql/**/*.test and
# extension_name / EXTENSION_NAME in .github/workflows/MainDistributionPipeline.yml.
EXTENSION_NAME=duckfn_quack

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
# rust.Makefile 里的构建命令是裸 `cargo build`（wasm 时带 `--example $(EXTENSION_NAME)`）。示例
# crate 现在位于 duckfn-quack/，之所以不用在这里覆盖目标、也不需要任何 `-p duckfn_quack`，是因为
# 根 Cargo.toml 的 [workspace] 声明了 `default-members = ["duckfn-quack"]`：在根目录不带包选择参数
# 时，cargo 选中的正是这个成员，于是 cdylib 与 example 都能被找到。（根包 duckfn 不能写进这个
# 列表，cargo 会报 not a member。）改动那个列表前请先读回来这一条。
#
# This Makefile has to stay at the repository root: CI's extension-ci-tools/scripts/ci_phase.py
# always runs `make configure_ci|debug|release|test_*|upload` from the root, and the upstream
# workflow offers no way to set a different working directory.
#
# rust.Makefile builds with a bare `cargo build` (plus `--example $(EXTENSION_NAME)` for wasm).
# The example crate now sits in duckfn-quack/ and no target has to be overridden here, nor is a
# `-p duckfn_quack` needed anywhere: the root Cargo.toml declares
# `default-members = ["duckfn-quack"]` in its [workspace] section, so a package-less cargo
# invocation from the root selects exactly that member and finds both the cdylib and the example.
# (The root package duckfn cannot be listed there — cargo rejects it as "not a member".) Read this
# comment again before touching that list.
include extension-ci-tools/makefiles/c_api_extensions/base.Makefile
include extension-ci-tools/makefiles/c_api_extensions/rust.Makefile

# sqllogictest 用例随示例一起迁到 duckfn-quack/test/sql。
# The sqllogictest suite moved together with the example, to duckfn-quack/test/sql.
TEST_RUNNER_BASE=$(TEST_RUNNER) --test-dir duckfn-quack/test/sql $(EXTRA_EXTENSIONS_PARAM)

configure: venv platform extension_version

debug: build_extension_library_debug build_extension_with_metadata_debug
release: build_extension_library_release build_extension_with_metadata_release

test: test_debug
test_debug: test_extension_debug
test_release: test_extension_release

clean: clean_build clean_rust
clean_all: clean_configure clean
