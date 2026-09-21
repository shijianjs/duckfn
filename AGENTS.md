# AGENTS.md

本文件记录本仓库的约定与维护流程，供 AI 助手与维护者参考。

## 仓库约定

### 临时文件放到 target/

生成的临时文件（脚本、数据、日志、一次性验证代码等）一律放到 `target/` 下，
不要放在仓库根目录或其它已跟踪的目录里。`target/` 已被 git 忽略，不会污染工作区，
用完顺手删掉。

### 文本文件一律用 LF

所有新增或修改的文本文件使用 LF（`\n`）换行，不要 CRLF（`\r\n`）。

任务结束时，对本次新增的文本文件**机械地跑一遍替换命令即可，不需要先检测**
里面是否真的有 CRLF：

```powershell
# PowerShell：逐个文件把 CRLF 换成 LF（保持 UTF-8 无 BOM）
foreach ($f in @('path/to/new-file.md', 'path/to/new-script.sh')) {
    $p = Join-Path (Get-Location) $f
    $c = [IO.File]::ReadAllText($p)
    [IO.File]::WriteAllText($p, ($c -replace "`r`n", "`n"), [System.Text.UTF8Encoding]::new($false))
}
```

```bash
# Git Bash / Linux / macOS
sed -i 's/\r$//' path/to/new-file.md path/to/new-script.sh
```

> 仓库开启了 `core.autocrlf`，所以 `git diff` 偶尔会提示 "LF will be replaced by CRLF"，
> 那是检出到工作区时的行为，提交进仓库的内容始终是 LF。

## 发版流程

发版命令都在 `Justfile` 里（`just --list` 可查），实际逻辑在 `scripts/release.sh`。
放进脚本而不是直接写进 Justfile，是因为 just 的 shebang recipe 在 Windows 上需要
`cygpath` 翻译解释器路径，而 Git Bash 并不提供它。

## 发版流程

版本号形如 `X.Y.Z`（例如 `0.0.5`）。一次完整的发版 =
提升版本号 → 提交并打 tag → 等 CI 产出 Release → 发布到 crates.io → 切回下一开发版本。

只有**正式版本**才打 tag；`0.0.6-dev.0` 这类预发布版本留在分支上，不打 tag、不发布。

### 命令速查

| 步骤 | 命令 |
| --- | --- |
| 0. 前置检查 | `just release_check` |
| 1. 提升版本号 | `just release_bump 0.0.5` |
| 2. 提交并打 tag | `git commit …` 后 `just release_tag 0.0.5` |
| 3. 查看 CI | `just release_ci` |
| 4. 发布 crate | `just release_publish` |
| 5. 切开发版本 | `just release_dev 0.0.6-dev.0` |

### 0. 前置检查

```bash
just release_check   # cargo clippy --workspace --all-targets -- -D warnings 与 cargo build --workspace
just test            # 需要时（等价 make configure debug test，make 部分要在 Git Bash 里跑）
```

有 warning 先修好再提交。确认 `git status` 干净、`main` 已与远程同步。

### 1. 提升版本号

```bash
just release_bump 0.0.5
```

脚本做两件事，并打印每个被改动的文件：

- **Cargo 文件**：取工作区当前版本（开发版本，如 `0.0.5-dev.0`）→ `0.0.5`，
  涉及 `Cargo.toml` 与 `duckfn/Cargo.toml`。
- **文档 / README / CI 注释**：取**最近一次 tag** 的版本（如 `0.0.4`）→ `0.0.5`。
  涉及的文件由 `git grep` 自动找出，不需要维护清单：
  - `README.md`、`README.zh-CN.md`
  - `duckfn/README.md`、`duckfn/README.zh-CN.md`
  - `.github/workflows/MainDistributionPipeline.yml`：注释里的示例 tag
  - `docs/duckfn-version.ts`：文档站版本号的唯一来源

  文档站的正文（`docs/docs/**`、`docs/i18n/**`）不再出现具体版本号，只写
  `{{DUCKFN_VERSION}}` 占位符，由 `docs/plugins/remark-version-placeholder.ts`
  在构建时替换成 `docs/duckfn-version.ts` 里的值。

最后用 `cargo update -p duckfn -p duckfn-macro` 同步 `Cargo.lock`，并打印残留的旧版本号
（应当为空）以及 `git diff --stat`。

> 根目录示例扩展 `rusty_quack` 的版本（`0.1.0`）与 `duckfn` 的版本无关，脚本不会碰它。
> `Cargo.lock`、`docs/package-lock.json` 与本文件被排除在替换之外。

### 2. 提交并打 tag

```bash
git add -A
git commit -m "chore(release): 发布 vX.Y.Z" \
  -m "- 将 workspace 版本号从 A.B.C 升级到 X.Y.Z" \
  -m "- 同步 duckfn 对 duckfn-macro 的精确依赖版本及文档中的版本示例"
just release_tag 0.0.5     # 打 tag v0.0.5，推送 main 与 tag
```

`release_tag` 会先检查工作区是否干净。推送 tag 会触发两个 workflow：

- `Main Extension Distribution Pipeline`（`.github/workflows/MainDistributionPipeline.yml`）：构建各平台扩展，并为该 tag 创建（或更新）GitHub Release。
- `Deploy Docs`：构建并部署文档站。

### 3. 等 CI 全绿

```bash
just release_ci            # gh run list --limit 5
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

```bash
just release_publish   # publish_macro_dry → publish_macro → publish_dry → publish
```

两个 crate 按依赖顺序发布，`duckfn-macro` 必须先上线（`cargo publish` 会自动等待它在索引里可见）。

若 `duckfn` 的 dry-run 报
`failed to select a version for the requirement duckfn-macro = "=X.Y.Z"`，
说明宏包还没在 crates.io 索引里可见，稍等片刻重试即可。

#### 网络报错就原样重试，不要动代理

`cargo publish`（以及 `git push`）访问 crates.io / GitHub 时可能报：

```
warning: spurious network error (N tries remaining): [35] SSL connect error
error: ... (schannel: failed to receive handshake, SSL/TLS connection failed)
```

这是**间歇性**的，**原样重试一两次即可通过**（实测 `just release_publish` 重跑即两个 crate 全部
上传成功，`git push` 也是第二次成功；过程中 cargo 自己会打印若干 `spurious network error` 后继续，属正常）。

**不要擅自更改网络 / 代理状态**：

- 本机 `git` 是**全局配置了代理**的，`github.com` 不走代理基本用不了 —— 动代理设置会直接
  让 `release_tag` 的推送失败。
- 代理一般比直连更不稳定，遇到报错先重试，不要改用或切换代理去"绕"。

### 5. 切到下一开发版本

```bash
just release_dev 0.0.6-dev.0
```

这一步只动 `Cargo.toml`、`duckfn/Cargo.toml` 和 `Cargo.lock`：文档与 README 中的示例
始终指向最新**已发布**版本，不打 tag、不发布。

## 相关文档

- [`scripts/release.sh`](scripts/release.sh)：`release_bump` / `release_dev` / `release_tag` 的实际实现。
- [`docs/duckfn-version.ts`](docs/duckfn-version.ts) 与 [`docs/plugins/remark-version-placeholder.ts`](docs/plugins/remark-version-placeholder.ts)：文档站的版本占位符机制。
- `templates/`：给下游扩展项目的两层模板。`AGENTS.md` 是项目层（复制过去只填两个值：项目目标与 duckfn clone 路径），
  `duckfn-conventions.md` 是共享层（知识源、硬约束、开发循环），由本仓库维护。
  项目层只引用共享层、不复制内容，因此 duckfn 升级时下游 `git pull` 即可，不用重做模板。
  另有 `Justfile` 可直接复制到新项目根目录，只改 `extension_name` 一处。
- [`docs/docs/build-and-release.md`](docs/docs/build-and-release.md)：面向读者的构建与发布说明。
- [`docs/docs/contributing.md`](docs/docs/contributing.md)：本地开发流程与约定。
