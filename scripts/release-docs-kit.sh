#!/usr/bin/env bash
#
# duckfn-docs-kit（npm 包）的发版辅助脚本，由根 Justfile 的 release_kit_* recipe 调用，
# 完整流程见 duckfn-docs-kit/CONVENTIONS.md。
#
# 与 scripts/release.sh 分开：那个发的是 crates.io 上的两个 crate、打 v*.*.* tag，会触发
# 扩展构建与文档站部署；这个发的是 npm 包、打 docs-kit-v* tag，不触发任何 workflow。
# 两条流程各管各的版本号，互不影响。
#
# 用法：
#   bash scripts/release-docs-kit.sh bump  <new-version>   # 提升版本号（正式版本 X.Y.Z）
#   bash scripts/release-docs-kit.sh dev   <new-version>   # 切到下一开发版本（X.Y.Z-dev.N）
#   bash scripts/release-docs-kit.sh tag   <version>       # 打 docs-kit-v<version> tag 并推送
#   bash scripts/release-docs-kit.sh guard                 # 发布前检查（npm publish 之前跑）
set -euo pipefail

MANIFEST='duckfn-docs-kit/package.json'
LOCKFILE='package-lock.json'
WORKSPACE='duckfn-docs-kit'
TAG_PREFIX='docs-kit-v'

die() {
    echo "error: $*" >&2
    exit 1
}

usage() {
    echo "usage: release-docs-kit.sh {bump|dev|tag} <version> | guard" >&2
    exit 2
}

# 版本号只出现在两处：包清单的 version，以及根 lockfile 里该 workspace 的条目。
#
# 这里直接改 JSON，而不是走 `npm version` / `npm install`：它们会顺手 reify 整个 workspace，
# 触发对 registry 的 fetch（本机被 EALLOWREMOTE 拦下），结果是版本号改了、lockfile 没跟着动，
# 留下一个不一致的中间态。JSON 重写是幂等的：两个文件的既有格式就是 2 空格缩进 + LF。
read_version() {
    node -p "require('./${MANIFEST}').version"
}

write_version() {
    node -e '
const fs = require("fs");
const [manifest, lockfile, workspace, version] = process.argv.slice(1);
const pkg = JSON.parse(fs.readFileSync(manifest, "utf8"));
pkg.version = version;
fs.writeFileSync(manifest, JSON.stringify(pkg, null, 2) + "\n");
const lock = JSON.parse(fs.readFileSync(lockfile, "utf8"));
const entry = lock.packages?.[workspace];
if (!entry) throw new Error(`${lockfile} 里没有 ${workspace} 条目`);
entry.version = version;
fs.writeFileSync(lockfile, JSON.stringify(lock, null, 2) + "\n");
' "$MANIFEST" "$LOCKFILE" "$WORKSPACE" "$1"
}

# 只有正式版本才打 tag、才发 npm：npm 会把预发布版本也挂到 latest 上。
is_release_version() {
    [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

cmd_bump() {
    local new=$1 old

    is_release_version "$new" || die "发 npm 的版本号形如 X.Y.Z（${new}）"
    old=$(read_version)
    [ "$old" != "$new" ] || die "版本号已经是 ${new}"

    echo "版本 ${old} -> ${new}"
    write_version "$new"

    echo
    echo "残留的旧版本号（应为空）："
    # 两份说明里的版本号只是流程示例（根 AGENTS.md 与 kit 的 CONVENTIONS.md）；
    # lockfile 已由 write_version 同步过。
    git grep -n -F -- "$old" -- . \
        ':(exclude)AGENTS.md' \
        ':(exclude)duckfn-docs-kit/CONVENTIONS.md' \
        ':(exclude)package-lock.json' || true

    echo
    git --no-pager diff --stat
}

cmd_dev() {
    local new=$1 old

    old=$(read_version)
    [ "$old" != "$new" ] || die "版本号已经是 ${new}"

    case "$new" in
        *-*) ;;
        *) echo "note: ${new} 不含预发布后缀，开发版本的约定写法是 X.Y.Z-dev.0" >&2 ;;
    esac

    echo "版本 ${old} -> ${new}"
    write_version "$new"

    echo
    git --no-pager diff --stat
}

cmd_tag() {
    local version=$1

    is_release_version "$version" || die "只有正式版本才打 tag（${version}）"
    [ "$(read_version)" = "$version" ] \
        || die "${MANIFEST} 里是 $(read_version)，先 just release_kit_bump ${version}"

    if ! git diff --quiet || ! git diff --cached --quiet; then
        die "工作区不干净，先提交再打 tag"
    fi

    git tag "${TAG_PREFIX}${version}"
    git push github main
    git push github "${TAG_PREFIX}${version}"
    echo "已推送 ${TAG_PREFIX}${version}"
    echo "它不匹配 CI 的 v*.*.* 过滤器，不会触发扩展构建与文档站部署；下一步 just release_kit_publish"
}

cmd_guard() {
    local version

    version=$(read_version)
    is_release_version "$version" \
        || die "当前是开发版本 ${version}，发出去会被挂到 npm 的 latest 上；先 just release_kit_bump <X.Y.Z>"

    if ! git diff --quiet || ! git diff --cached --quiet; then
        die "工作区不干净：发布内容会与仓库里的提交对不上"
    fi

    git rev-parse -q --verify "refs/tags/${TAG_PREFIX}${version}" >/dev/null \
        || die "本地没有 ${TAG_PREFIX}${version} tag，先 just release_kit_tag ${version}"

    if npm view "duckfn-docs-kit@${version}" version >/dev/null 2>&1; then
        die "npm 上已经有 duckfn-docs-kit@${version}：同一版本不能覆盖，只能发新版本"
    fi

    echo "即将发布 duckfn-docs-kit@${version}"
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
    guard)
        [ $# -eq 0 ] || usage
        cmd_guard
        ;;
    *)
        usage
        ;;
esac
