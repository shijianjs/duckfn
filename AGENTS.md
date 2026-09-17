# AGENTS.md

本文件记录本仓库的维护流程，供 AI 助手与维护者参考。目前只包含发版流程。

## 发版流程

版本号形如 `X.Y.Z`（例如 `0.0.4`）。一次完整的发版 =
提升版本号 → 提交并打 tag → 等 CI 产出 Release → 发布到 crates.io → 切回下一开发版本。

只有**正式版本**才打 tag；`0.0.5-dev.0` 这类预发布版本留在分支上，不打 tag、不发布。

### 0. 前置检查

- `cargo clippy --workspace --all-targets` 与 `cargo build --workspace` 无 warning；有则先修好再提交。
- 需要时先跑测试：`just test`（等价 `make configure debug test`，需在 Git Bash 中执行 `make` 部分）。
- `git status` 干净，`main` 已与远程同步。

### 1. 提升版本号

把仓库中所有版本号 `X.Y.Z` 替换为新版本，涉及：

| 文件 | 需要同步的内容 |
| --- | --- |
| `Cargo.toml` | `[workspace.package] version = "X.Y.Z"` |
| `duckfn/Cargo.toml` | `duckfn-macro = { version = "=X.Y.Z", path = "../duckfn-macro" }` |
| `Cargo.lock` | `duckfn` / `duckfn-macro` 的 `version`（由 cargo 命令生成，不要手改） |
| `README.md`、`README.zh-CN.md` | 安装示例 `duckfn = "X.Y.Z"` |
| `duckfn/README.md`、`duckfn/README.zh-CN.md` | 安装示例、`duckdb-1-5` feature 示例 |
| `docs/docs/**` | `build-and-release.md`、`contributing.md`、`getting-started/installation.md`、`getting-started/quick-start.md`、`guide/copy-functions.md` |
| `docs/i18n/zh-Hans/docusaurus-plugin-content-docs/current/**` | 上面各页面对应的中文版 |
| `.github/workflows/MainDistributionPipeline.yml` | 注释里的示例 tag（如 `v0.0.4`） |

改完两个 `Cargo.toml` 后同步 `Cargo.lock`：

```bash
cargo update -p duckfn -p duckfn-macro
```

核对没有遗漏（`git grep` 命中 `docs/package-lock.json` 属无关内容，可忽略）：

```bash
git grep -n "X\.Y\.Z"
```

> 根目录示例扩展 `rusty_quack` 的版本（`0.1.0`）与 `duckfn` 的版本无关，不要改动。

### 2. 提交并打 tag

```bash
git add -A
git commit -m "chore(release): 发布 vX.Y.Z" \
  -m "- 将 workspace 版本号从 A.B.C 升级到 X.Y.Z" \
  -m "- 同步 duckfn 对 duckfn-macro 的精确依赖版本及文档中的版本示例"
git tag vX.Y.Z
git push github main
git push github vX.Y.Z
```

推送 tag 会触发两个 workflow：

- `Main Extension Distribution Pipeline`（`.github/workflows/MainDistributionPipeline.yml`）：构建各平台扩展，并为该 tag 创建（或更新）GitHub Release。
- `Deploy Docs`：构建并部署文档站。

### 3. 等 CI 全绿

```bash
gh run list --limit 5
gh run watch <run-id>
```

失败就修到成功为止。若已推送的 tag 需要重发（修复后重新指向新的提交）：

```bash
git push github --delete vX.Y.Z   # 删除远程 tag
git tag -f vX.Y.Z                 # 本地 tag 指向修复后的提交
git push github vX.Y.Z            # 重新推送
```

> 删除/移动已发布的 tag 会影响已有的 GitHub Release，谨慎操作。

### 4. 发布到 crates.io

两个 crate 按依赖顺序发布，`duckfn-macro` 必须先上线：

```bash
just publish_macro_dry   # cargo publish -p duckfn-macro --registry crates-io --dry-run
just publish_macro       # cargo publish -p duckfn-macro --registry crates-io
# cargo 会自动等待 duckfn-macro X.Y.Z 在 crates.io 上可用

just publish_dry         # cargo publish -p duckfn --registry crates-io --dry-run
just publish             # cargo publish -p duckfn --registry crates-io
```

若 `duckfn` 的 dry-run 报
`failed to select a version for the requirement duckfn-macro = "=X.Y.Z"`，
说明宏包还没在 crates.io 索引里可见，稍等片刻重试即可。

### 5. 切到下一开发版本

发布完成后，把 workspace 版本设为下一个预发布版本，避免与已发布版本冲突：

```bash
# Cargo.toml      → version = "0.0.5-dev.0"
# duckfn/Cargo.toml → duckfn-macro = { version = "=0.0.5-dev.0", path = "../duckfn-macro" }
cargo update -p duckfn -p duckfn-macro
cargo metadata --no-deps --offline --format-version 1 > /dev/null   # 校验依赖解析
```

这一步只动 `Cargo.toml`、`duckfn/Cargo.toml` 和 `Cargo.lock`：文档与 README 中的示例
始终指向最新**已发布**版本，不打 tag、不发布。

## 相关文档

- [`docs/docs/build-and-release.md`](docs/docs/build-and-release.md)：面向读者的构建与发布说明。
- [`docs/docs/contributing.md`](docs/docs/contributing.md)：本地开发流程与约定。
