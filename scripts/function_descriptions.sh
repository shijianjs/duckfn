#!/usr/bin/env bash
#
# 生成 / 校验社区扩展文档页需要的 docs/function_descriptions.csv，
# 由 Justfile 的 docs_csv / docs_csv_check recipe 调用。
#
# 背景：DuckDB 的 C 扩展 API 没有设置函数 description / example 的接口，所以这类文本
# 只能由 community-extensions 仓的 scripts/generate_md.sh 用
# extensions/<扩展名>/docs/function_descriptions.csv 去覆盖生成出来的文档页。
# duckfn 把描述写在 #[duck_*] 属性上，加载扩展时由 duckfn 自己导出这份 CSV。
#
# 之所以单独放一个脚本：just 的 shebang recipe 在 Windows 上需要 cygpath 翻译解释器路径，
# 而 Git Bash 并不提供它（与 scripts/release.sh 同样的理由）。
#
# 用法：
#   bash scripts/function_descriptions.sh gen   [输出路径]   # 构建并导出（默认 docs/function_descriptions.csv）
#   bash scripts/function_descriptions.sh check [已提交的 CSV] # 与当前扩展对账，有差异非零退出
set -euo pipefail

# duckdb 命令行；不在 PATH 里时用 `DUCKDB=/path/to/duckdb bash scripts/...` 覆盖。
DUCKDB_BIN="${DUCKDB:-duckdb}"

# 导出开关：duckfn 在扩展加载时读它，值是 CSV 的输出路径。
DUMP_ENV='DUCKFN_DUMP_FUNCTION_DESCRIPTIONS'

DEFAULT_OUT='docs/function_descriptions.csv'

die() {
    echo "error: $*" >&2
    exit 1
}

# 扩展名从 src/extension/mod.rs 的 duckfn_entrypoint!("...") 里读，不在这里硬编码。
extension_name() {
    local name
    name=$(sed -n 's/.*duckfn_entrypoint!("\([a-z0-9_]*\)").*/\1/p' src/extension/mod.rs | head -n 1)
    [ -n "$name" ] || die "cannot read the extension name from src/extension/mod.rs"
    echo "$name"
}

# 构建扩展并导出 CSV。
cmd_gen() {
    local out="${1:-$DEFAULT_OUT}"
    local name ext_path
    name=$(extension_name)
    ext_path="target/debug/${name}.duckdb_extension"

    cargo duckdb-ext build

    mkdir -p "$(dirname "$out")"
    env "$DUMP_ENV=$out" "$DUCKDB_BIN" -unsigned -c "LOAD '${ext_path}';" >/dev/null

    [ -s "$out" ] || die "nothing was written to $out"

    # 没写 description 的函数：文档页上会是一片空白，列出来提醒补。
    local missing
    missing=$("$DUCKDB_BIN" -noheader -list -c "
        SELECT function
        FROM read_csv('$out')
        WHERE description IS NULL OR trim(description) = ''
        ORDER BY 1;" || true)

    if [ -n "$missing" ]; then
        echo
        echo "These functions have no description yet (add description = \"...\" to the attribute):"
        echo "$missing" | sed 's/^/  /'
    fi
}

# 与已提交的 CSV 对账：列出新增 / 缺失 / 文本有变化的函数，有差异非零退出（可接 CI）。
cmd_check() {
    local committed="${1:-$DEFAULT_OUT}"
    local generated='target/function_descriptions.generated.csv'

    [ -s "$committed" ] || die "$committed does not exist — run 'gen' first"

    cmd_gen "$generated"

    local report
    report=$("$DUCKDB_BIN" -noheader -list -c "
        WITH committed AS (SELECT * FROM read_csv('$committed')),
             generated AS (SELECT * FROM read_csv('$generated'))
        SELECT 'added   ' || g.function
        FROM generated g LEFT JOIN committed c ON c.function = g.function
        WHERE c.function IS NULL
        UNION ALL
        SELECT 'removed ' || c.function
        FROM committed c LEFT JOIN generated g ON g.function = c.function
        WHERE g.function IS NULL
        UNION ALL
        SELECT 'changed ' || g.function
        FROM generated g JOIN committed c ON c.function = g.function
        WHERE coalesce(g.description, '') <> coalesce(c.description, '')
           OR coalesce(g.comment, '')     <> coalesce(c.comment, '')
           OR coalesce(g.example, '')     <> coalesce(c.example, '')
        ORDER BY 1;" || true)

    if [ -n "$report" ]; then
        echo
        echo "$committed is out of date:"
        echo "$report" | sed 's/^/  /'
        echo
        echo "Run 'just docs_csv' to regenerate it."
        exit 1
    fi

    echo "$committed is up to date."
}

usage() {
    cat <<'USAGE'
Usage:
  bash scripts/function_descriptions.sh gen   [output.csv]   # build the extension and export the CSV
  bash scripts/function_descriptions.sh check [committed.csv] # fail when the CSV is out of date
USAGE
}

case "${1:-}" in
    gen)
        shift
        cmd_gen "$@"
        echo
        echo "Next: copy ${1:-$DEFAULT_OUT} into the community-extensions repo as"
        echo "      extensions/$(extension_name)/docs/function_descriptions.csv when you open the PR."
        ;;
    check) shift; cmd_check "$@" ;;
    -h | --help | '') usage ;;
    *) die "unknown command: $1" ;;
esac
