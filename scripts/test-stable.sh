#!/usr/bin/env bash
#
# 「稳定区」插件的对照测试：用不带 `duckdb-1-5` / `duck_vfs` / `sql` 的 feature 集打一份插件，再拿
# 指定版本的 DuckDB 跑指定范围的 sqllogictest。
#
# 为什么可行：那一档只用 C API 的**稳定区**，产物声明 `C_STRUCT` + C API 下限 v1.2.0，于是 1.3 到 2.0
# 的引擎都收它（见 docs/docs/duckdb-versions/c-api-and-abi.md）。这就是「一份产物跨版本」的用法：同一
# 份源码既能打「与引擎小版本严格对齐」的全量产物，也能打一份跨版本的，拿后者去跑更老引擎上的用例子集。
#
# 用法：
#   scripts/test-stable.sh 1.4.0
#   scripts/test-stable.sh 1.4.0 'test/sql/functions/scalar_function.test'
#   scripts/test-stable.sh 1.4.0 'test/sql/functions/*.test' 'test/sql/types/*.test'
#
# 不给用例模式时跑**默认范围**：test/sql 下除下面的 NON_STABLE 之外的全部用例。给了模式时按模式选
# （可以给多条，`.test` 路径或 shell 通配），此时不再自动排除 —— 点名要跑的就跑。
#
# 环境变量：
#   EXTENSION_FEATURES   覆盖构建用的 feature 列表（默认 quack = 只有稳定区）
#   DUCKDB_PYTHON        覆盖 venv 的 python 路径（默认自动探测 configure/venv）
#
# Usage: scripts/test-stable.sh <duckdb version> [test pattern ...]. Without patterns it runs the
# default range: every case under test/sql except the NON_STABLE ones below. Environment:
# EXTENSION_FEATURES overrides the build's feature list (default `quack`, i.e. stable region only).
#
# 注意：跑完之后 build/debug 里是**稳定区产物**；要恢复全量产物跑 `just build`（或 `make debug`）。
# 另外 runner 的引擎版本会**就地改成你给的那个**（configure/venv 只由 configure 装一次），跑完要回到
# 默认引擎就再装一次：`configure/venv/Scripts/python.exe -m pip install --upgrade duckdb==1.5.6`。
#
# Note: afterwards build/debug holds the **stable-region artifact**; run `just build` (or `make debug`)
# to get the full one back. The runner's engine is also **changed in place** to the version you passed
# (configure/venv is only populated by `configure`); to go back to the default engine:
# `configure/venv/Scripts/python.exe -m pip install --upgrade duckdb==1.5.6`.

set -euo pipefail

cd "$(dirname "$0")/.."

if [ $# -lt 1 ]; then
    echo "用法: scripts/test-stable.sh <duckdb版本，如 1.4.0> [用例模式 ...]" >&2
    echo "usage: scripts/test-stable.sh <duckdb version, e.g. 1.4.0> [test pattern ...]" >&2
    exit 2
fi

engine="$1"
shift

# 默认构建的 feature 集：只开 `quack`（示例树这个标记），因此插件里只有稳定区的函数。
#
# The default feature set: only `quack` (the example tree's marker), so the plugin exposes the stable
# region alone.
features="${EXTENSION_FEATURES:-quack}"

# C API 下限：稳定区构建声明的是「C API 不低于此即可加载」，v1.2.0 是 quack-rs 的基线。
#
# The C API floor: a stable build declares "any engine whose C API is at or above this", and v1.2.0 is
# quack-rs' baseline.
c_api_floor="v1.2.0"

# 需要「稳定区之外」能力的用例（文件名去掉 .test）。默认范围把它们排除掉，因为它们对应的函数在稳定区
# 产物里根本不存在：
#   copy_function / copy_from_function  COPY 函数（不稳定区）
#   wrapper_types_*_echo                TIME_NS 那一条（DuckDB 1.5 才有）
#   duck_vfs / file_system / file_util  宿主文件系统（duck_vfs）
#   sql_exec                            在回调里执行 SQL（sql）
#   chrono_bridge / rust_decimal_bridge / uuid_bridge   三个互操作桥各自的 feature
#
# The cases that need capabilities beyond the stable region (file names without `.test`). The default
# range excludes them because their functions do not exist in a stable-region artifact at all.
NON_STABLE="\
copy_function \
copy_from_function \
wrapper_types_scalar_echo \
wrapper_types_table_echo \
duck_vfs \
file_system \
file_util \
sql_exec \
chrono_bridge \
rust_decimal_bridge \
uuid_bridge"

list="target/stable-tests.txt"
mkdir -p target

# 候选集：test/sql 下全部 .test（find 而不是 **，Git Bash 不一定开了 globstar）。
#
# The candidate set: every .test under test/sql (via find rather than `**`, since Git Bash may not have
# globstar on).
all_cases() {
    find test/sql -name '*.test' | sort
}

if [ $# -gt 0 ]; then
    # 点名模式：逐条按 shell 通配匹配（路径里的 `/` 必须写全，例如 test/sql/functions/*.test）。
    #
    # Named patterns: matched with shell globbing, so a pattern must spell out its directories
    # (test/sql/functions/*.test, say).
    all_cases | while IFS= read -r file; do
        for pattern in "$@"; do
            # shellcheck disable=SC2254  # 这里就是要让 $pattern 作为通配生效 / the glob is intended
            case "$file" in
                $pattern)
                    printf '%s\n' "$file"
                    break
                    ;;
            esac
        done
    done >"$list"
else
    all_cases | while IFS= read -r file; do
        base="$(basename "$file" .test)"
        case " $NON_STABLE " in
            *" $base "*)
                # 提示写到 **stderr**：这个循环的 stdout 就是清单文件（`done >"$list"`），
                # 写到 stdout 会把提示词混进清单，runner 再把它们当成用例路径（曾经的 bug：
                # 45 行 122 个词，runner 报了 `[1/122]`）。
                #
                # The notice goes to **stderr**: this loop's stdout *is* the list file
                # (`done >"$list"`), so writing it to stdout mixes the words into the list and the
                # runner then reads them as test paths (the bug this once was: 45 lines, 122 words,
                # and the runner reported `[1/122]`).
                echo "  skip (needs more than the stable API): $file" >&2
                continue
                ;;
        esac
        printf '%s\n' "$file"
    done >"$list"
fi

# 再来一道闸：清单里每一行都必须是 `test/sql/**/*.test`。这一条能立刻抓住「提示信息混进清单」
# 这类错误（runner 对读不出来的路径只是记一笔，然后照样往下跑，所以不查的话会误以为跑过了）。
#
# One more gate: every line must be a `test/sql/**/*.test`. It catches "a notice leaked into the list"
# immediately (the runner merely logs an unparsable path and carries on, so without this check a broken
# list still looks like a run).
if grep -qv '^test/sql/.*\.test$' "$list"; then
    echo "错误：清单里出现了不是用例路径的行，多半是提示信息混进去了：" >&2
    echo "error: the list contains lines that are not test paths:" >&2
    grep -v '^test/sql/.*\.test$' "$list" | head -5 >&2
    exit 1
fi

count="$(wc -l <"$list" | tr -d '[:space:]')"
if [ "$count" -eq 0 ]; then
    echo "错误：没有匹配到任何用例，检查模式是否写对 / no test case matched, check the patterns" >&2
    exit 1
fi

echo "构建 feature: $features（C API 下限 $c_api_floor）"
echo "用例: $count 条（清单写在 $list）"
echo "引擎: duckdb $engine"

# 1) 配置（建 venv、复位 platform.txt）；venv 已经存在时这一步几乎是瞬间的。
#
# 1) Configure (create the venv, reset platform.txt); with the venv already there this is near-instant.
make configure

# 2) 引擎版本：venv 只在 configure 时装一次，之后不会自己刷新，所以这里按需就地换版本。先问一遍已装的
# 版本，一致就跳过（不白下一次下载）；要换时**不加 --quiet** —— pip 自己的进度是「它在下载，不是卡
# 住」的唯一信号，慢网络下正是最容易误判的地方。
#
# 2) The engine version: the venv is only populated by `configure` and never refreshed afterwards, so a
# different version is installed in place here. The installed version is checked first and a match is
# skipped (no pointless download); when it does change, pip runs **without --quiet** — its progress is
# the only sign that it is downloading rather than stuck, which is exactly what is misread on a slow
# connection.
venv_python="${DUCKDB_PYTHON:-}"
if [ -z "$venv_python" ]; then
    for candidate in configure/venv/bin/python3 configure/venv/Scripts/python.exe; do
        if [ -x "$candidate" ]; then
            venv_python="$candidate"
            break
        fi
    done
fi
if [ -z "$venv_python" ]; then
    echo "错误：找不到 configure/venv 的 python，用 DUCKDB_PYTHON 指定" >&2
    echo "error: no python in configure/venv; point DUCKDB_PYTHON at one" >&2
    exit 1
fi

installed="$("$venv_python" -m pip show duckdb 2>/dev/null | sed -n 's/^Version: //p' | head -1)"
if [ "$installed" = "$engine" ]; then
    echo "引擎已经是 $engine，跳过安装 / engine already at $engine, skipping the install"
else
    echo "把 runner 的引擎从 ${installed:-（未装）} 换成 $engine（首次约 20MB，慢网络下会等一会）…"
    echo "switching the runner's engine from ${installed:-none} to $engine (about 20 MB the first time)…"
    # `--only-binary=:all:` 是刻意的：DuckDB 的 sdist 要从源码编 C++（十几分钟起步，而且常常编不过），
    # 碰到「那个 patch 版本没给这个 Python 预编译 wheel」时（例如 cp314 上的 1.4.0，只有 1.4.5+ 才有）
    # 与其静默开始一次源码构建，不如立刻失败，让人换个 patch 版本重试。
    #
    # `--only-binary=:all:` is deliberate: DuckDB's sdist compiles C++ from source (ten minutes and up,
    # and often fails outright). When a patch release ships no wheel for this Python — 1.4.0 on cp314,
    # say, where only 1.4.5+ has one — failing immediately beats silently starting a source build.
    if ! "$venv_python" -m pip install --upgrade --timeout 60 --only-binary=:all: "duckdb==${engine}"; then
        echo "错误：装不上 duckdb $engine —— 多半是这个 patch 版本没有本机 Python 的预编译 wheel。" >&2
        echo "      换一个 patch 版本重试（同一 minor 里挑有 wheel 的，例如 1.4.5 而不是 1.4.0）。" >&2
        echo "error: cannot install duckdb $engine — most likely that patch release has no wheel for" >&2
        echo "       this Python. Pick another patch release of the same minor (1.4.5, not 1.4.0)." >&2
        exit 1
    fi
fi

# 2) 稳定区构建：USE_UNSTABLE_C_API=0 + C API 下限，产物跨版本。
#
# 2) The stable build: USE_UNSTABLE_C_API=0 plus the C API floor, so the artifact is portable.
make USE_UNSTABLE_C_API=0 \
    TARGET_DUCKDB_VERSION="$c_api_floor" \
    EXTENSION_FEATURES="$features" \
    debug

# 3) 跑指定范围：EXTRA_TEST_ARGS 把清单交给 runner（--file-list 是空白分隔的路径列表）。
#
# 3) Run the range: EXTRA_TEST_ARGS hands the list to the runner (--file-list takes a
# whitespace-separated list of paths).
make USE_UNSTABLE_C_API=0 \
    TARGET_DUCKDB_VERSION="$c_api_floor" \
    DUCKDB_TEST_VERSION="$engine" \
    EXTENSION_FEATURES="$features" \
    test_debug \
    EXTRA_TEST_ARGS="--file-list $list"

echo "稳定区用例全部通过（引擎 $engine，feature: $features）"
echo "stable-region cases passed (engine $engine, features: $features)"
