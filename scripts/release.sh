#!/usr/bin/env bash
#
# 发版辅助脚本，由 Justfile 的 release_* recipe 调用（完整流程见根目录 AGENTS.md）。
#
# 之所以有这么一个脚本，而不是把逻辑直接写在 Justfile 里：just 的 shebang recipe
# 在 Windows 上需要 cygpath 来翻译解释器路径，而 Git Bash 并不提供它。改成
# 「Justfile 里一行 `bash scripts/release.sh ...`」后，Windows（Git Bash）与
# Linux / macOS 都走同一条路径。
#
# 用法：
#   bash scripts/release.sh bump <new-version>   # 项目 + 文档的版本号全量替换
#   bash scripts/release.sh dev  <new-version>   # 只把 Cargo 清单切到开发版本
#   bash scripts/release.sh tag  <version>       # 打 tag 并推送，触发 CI 发版
set -euo pipefail

# 核对「旧版本号残留」时跳过的文件：
# Cargo.lock 由 cargo update 负责；package-lock.json 与本项目版本号无关；
# AGENTS.md 是流程说明，里面的版本号只是示例。
CHECK_EXCLUDES=(
    ':(exclude)Cargo.lock'
    ':(exclude)docs/package-lock.json'
    ':(exclude)AGENTS.md'
)

# 批量替换时额外跳过：Cargo 清单单独处理（根 Cargo.toml 是唯一出现字面版本号的地方）；
# 文档站的正文只写 {{DUCKFN_VERSION}} 占位符，版本号集中在 docs/duckfn-version.ts。
REPLACE_EXCLUDES=(
    "${CHECK_EXCLUDES[@]}"
    ':(exclude)Cargo.toml'
    ':(exclude)docs/duckfn-version.ts'
    ':(exclude)docs/docs'
    ':(exclude)docs/i18n'
)

# 文档站版本号的唯一来源，在 cmd_bump 里显式替换：
# 它可能是新建、尚未被 git 跟踪的文件，那时 git grep 找不到它。
DOC_VERSION_FILE='docs/duckfn-version.ts'

die() {
    echo "error: $*" >&2
    exit 1
}

# 读取 [workspace.package] 段的 version（注意不能匹配到 [package] 的 0.1.0）
workspace_version() {
    awk -F'"' '
        /^\[workspace\.package\]/ { in_section = 1; next }
        /^\[/                     { in_section = 0 }
        in_section && /^version *=/ { print $2; exit }
    ' Cargo.toml
}

# 最近一次版本 tag（去掉 v 前缀），即文档中示例的版本
prev_release_version() {
    local tag
    tag=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)
    echo "${tag#v}"
}

sync_lock() {
    cargo update -p duckfn -p duckfn-macro
}

# 把某个版本号字符串转换成适合放进正则的写法（只需转义点号）
sed_escape() {
    printf '%s' "$1" | sed 's/\./\\./g'
}

# 把文件里「独立出现」的版本号 $2 全部换成 $3。
#
# 不用 `sed s///`：那是子串替换，旧版本号是更长的版本号的前缀时会被改坏 ——
# 0.0.3 会命中正文里的 duckfn 历史版本引用（0.0.31 / 0.0.38 这类），
# 换成 0.1.0 就成了 0.1.01 / 0.1.08。这里要求前后都不是数字或点，
# 于是 0.0.31 不会被碰、10.0.3 也不会。
#
# Replaces standalone occurrences of version $2 with $3 in a file. A plain `sed s///` is a substring
# replacement, so an old version that happens to be a prefix of a longer one corrupts it (0.0.3 inside
# 0.0.31). Requiring a non-digit/non-dot on both sides avoids that without touching real version refs.
replace_version_in_file() {
    local file=$1 old=$2 new=$3 tmp
    tmp=$(mktemp) || die "无法创建临时文件"
    awk -v old_ver="$old" -v new_ver="$new" '
        function replace_version(line,   out, i, n, before, after, standalone) {
            if (old_ver == "") return line
            out = ""
            n = length(old_ver)
            while ((i = index(line, old_ver)) > 0) {
                before = (i > 1) ? substr(line, i - 1, 1) : ""
                after  = substr(line, i + n, 1)
                standalone = (before !~ /[0-9.]/) && (after !~ /[0-9.]/)
                out = out substr(line, 1, i - 1) (standalone ? new_ver : old_ver)
                line = substr(line, i + n)
            }
            return out line
        }
        { print replace_version($0) }
    ' "$file" > "$tmp"
    mv "$tmp" "$file"
}

cmd_bump() {
    local new=$1 dev doc tag files f v

    dev=$(workspace_version)
    [ -n "$dev" ] || die "无法从 Cargo.toml 读取 [workspace.package] version"

    tag=$(prev_release_version)
    doc=${tag#v}

    if [ "$dev" = "$new" ] && [ "$doc" = "$new" ]; then
        die "版本号已经是 ${new}"
    fi

    # Cargo 清单里的版本号 = 工作区当前版本。
    # 根 Cargo.toml 既是 workspace 根、又是 duckfn 包的清单，也是全仓唯一出现字面版本号
    # 的地方（[workspace.package] 的 version，以及 duckfn-macro 的精确 pin）。
    if [ "$dev" != "$new" ]; then
        echo "Cargo 版本 ${dev} -> ${new}"
        replace_version_in_file Cargo.toml "$dev" "$new"
    fi

    # 文档 / README / CI 注释里的版本号 = 最近一次 tag 的版本。
    # docs/docs 与 docs/i18n 只写占位符，真正的版本号集中在 DOC_VERSION_FILE。
    if [ -n "$doc" ] && [ "$doc" != "$new" ]; then
        echo "文档 / CI 版本 ${doc} -> ${new}（取自 ${tag}）"
        mapfile -t files < <(git grep -l -F -- "$doc" -- . "${REPLACE_EXCLUDES[@]}")
        for f in "${files[@]}"; do
            replace_version_in_file "$f" "$doc" "$new"
            echo "  updated $f"
        done

        if [ -f "$DOC_VERSION_FILE" ]; then
            replace_version_in_file "$DOC_VERSION_FILE" "$doc" "$new"
            echo "  updated $DOC_VERSION_FILE"
        fi
    fi

    sync_lock

    echo
    echo "残留的旧版本号（应为空）："
    for v in "$dev" "$doc"; do
        [ -n "$v" ] || continue
        [ "$v" = "$new" ] && continue
        # 与替换同样的边界口径：只报「独立出现」的旧版本号，别把 0.0.31 这类
        # 更长的版本号误报成 0.0.3 的残留。
        e=$(sed_escape "$v")
        git grep -n -E -- "(^|[^0-9.])${e}([^0-9.]|$)" -- . "${CHECK_EXCLUDES[@]}" || true
    done

    echo
    git --no-pager diff --stat
}

cmd_dev() {
    local new=$1 old

    old=$(workspace_version)
    if [ -z "$old" ] || [ "$old" = "$new" ]; then
        die "当前版本为 ${old:-未知}，无需切换"
    fi

    replace_version_in_file Cargo.toml "$old" "$new"
    sync_lock

    git --no-pager diff --stat
    echo "已切换到开发版本 ${new}"
}

cmd_tag() {
    local version=$1

    if ! git diff --quiet || ! git diff --cached --quiet; then
        die "工作区不干净，先提交再打 tag"
    fi

    git tag "v${version}"
    git push github main
    git push github "v${version}"
    echo "已推送 v${version}，用 just release_ci 查看进度"
}

usage() {
    echo "usage: release.sh {bump|dev|tag} <version>" >&2
    exit 2
}

action=${1:-}
[ $# -gt 0 ] && shift

case "$action" in
    bump)
        [ $# -eq 1 ] || usage
        cmd_bump "$1"
        ;;
    dev)
        [ $# -eq 1 ] || usage
        cmd_dev "$1"
        ;;
    tag)
        [ $# -eq 1 ] || usage
        cmd_tag "$1"
        ;;
    *)
        usage
        ;;
esac
