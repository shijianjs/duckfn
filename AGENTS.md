# AGENTS.md

本文件记录本仓库的约定与维护流程，供 AI 助手与维护者参考。

## 仓库结构

本仓库是一个 Cargo workspace，而根目录同时就是 `duckfn` 运行时 crate 的包根：

| 路径 | 内容 | 是否发布 |
| --- | --- | --- |
| `/`（根） | `duckfn` 运行时：`src/`（不含 `src/extension/`）、`tests/`、`README.md`、`LICENSE`；根 `Cargo.toml` 同时是 workspace 根 | 是（crates.io） |
| `src/extension/` | 示例扩展的模块树（`demo/`、`functions/`、`types/`），默认不编译（见下） | 是（随 `duckfn` 包） |
| `src/bin/duckfn.rs` | 命令行工具入口（`[[bin]] duckfn-cli`，文件名仍是 duckfn.rs，原因见下）。WebAssembly 入口不需要单独文件 —— lib 自己产出 `staticlib`，见下 | 是（随 `duckfn` 包） |
| `test/sql/` | 示例的 sqllogictest 用例（41 个 `.test`） | 是（随 `duckfn` 包，只收 `.test`） |
| `duckfn-macro/` | 过程宏 crate | 是（crates.io） |
| `docs/` | Docusaurus 文档站：`docs/docs/**`（英）与 `docs/i18n/zh-Hans/docusaurus-plugin-content-docs/current/**`（中） | 是（随 `duckfn` 包，仅正文源文件） |
| `Makefile` / `Justfile` | 本地与 CI 的构建入口。Makefile 必须留在仓库根（CI 在根目录执行 `make`） | — |
| `extension-ci-tools/` | DuckDB 官方 CI 子模块，不要修改它的内容 | — |

示例扩展是**本包的一部分**，不再是独立 crate —— 这个区别是关键：cargo 会无条件跳过任何含
`Cargo.toml` 的子目录，独立打包的示例永远进不了 `duckfn` 的 `.crate`，而并进本包后它随包发布。

### 示例的开关：`quack` feature

示例的模块树由默认关闭的 `quack` feature 控制（`src/lib.rs` 里 `#[cfg(feature = "quack")] mod
extension;`），`[[bin]] duckfn-cli` 也写了 `required-features = ["quack"]`（wasm 没有单独的
`[[example]]` 目标，产物来自 lib 的 staticlib）：

- **下游**：`duckfn = "0.0.11"` 的依赖树与示例并入前完全一致，示例源码在包里但不参与编译。
- **本仓库**：所有构建扩展的命令都必须带上它 —— `make debug`（根 Makefile 里
  `TARGET_INFO += --features quack`）、`cargo build --features quack`、`just build`、
  `just build_wasm`。不带 feature 时 cargo 只是静默跳过目标（产出一个没有入口符号的 cdylib），
  `LOAD` 时才报错，很难查。
- `quack = ["all"]`，而 `all` **不**依赖 `quack`：`all` 是给下游用户用的，用户不需要示例与那些测试函数。

### 发布包内容

`duckfn` 发布包的内容由根 `Cargo.toml` 的 `include` 白名单决定：运行时代码与测试、示例扩展
（`src/extension/**`、`src/bin/duckfn.rs`）、`test/sql/**/*.test`、`demo.sh`、
README、LICENSE、文档站正文（英 + 中）。改这个白名单后，用 `cargo package -p duckfn --list`
核对一遍。三条容易踩的坑：

- **模式必须以 `/` 开头**。gitignore 风格里裸的 `LICENSE` / `README.md` 会匹配任意层级的同名
  文件 —— 实测会把 `docs/node_modules/**/LICENSE`、`configure/venv/**/LICENSE` 一起收进包
  （1600 多个文件）。
- **写了 `include` 就绕过 gitignore**。sqllogictest 会在 `test/sql` 下写出 `dfn_file_*`、
  `dfn_copy_*.tsv` 之类的临时文件，所以白名单只收 `*.test`；`test/sql` 里也请只留 `.test`。
- **含 `Cargo.toml` 的子目录一律被跳过**（`duckfn-macro/` 因此进不了包，这是对的：它单独发布）。

### 名字必须一致

DuckDB 扩展名 `duckfn` 必须四处一致：`src/extension/entry.rs` 的 `duckfn_entrypoint!`、根
`Makefile` 的 `EXTENSION_NAME`、CI 的 `extension_name` / `EXTENSION_NAME`，以及
`test/sql/**/*.test` 里的 `require`。它与 crate 名相同不是巧合 —— 原生扩展就是本包的 cdylib，
产物名由 crate 名决定（上游 makefile 按 `lib$(EXTENSION_NAME).*` 取产物），对齐后根 Makefile
一行平台条件都不用写。

两处因此而来的命名细节，改动前先读回来：

- **入口符号单独一个文件**（`src/extension/entry.rs`）：原生 lib 与 wasm 目标各声明一次，而 CLI
  虽然也编同一棵模块树（`#[path]` 那套），却链接了本包的 lib —— lib 里已经有一份入口符号，再定义
  一次就是重复定义（Windows 上 LNK2005），所以它只包含 `extension/mod.rs`。
- **CLI 的 bin 目标叫 `duckfn-cli`**（文件仍是 `src/bin/duckfn.rs`）：本包 cdylib 的产物也叫
  duckfn，Windows 上两者的 `.pdb` 会撞名（cargo 报 output filename collision）。下游项目的包名
  不同，不会撞，所以模板里那个 bin 依旧叫 `duckfn`。

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



### 尽量用成熟三方库实现，不要自己造轮子

写任何「通用」逻辑之前先问一句：这件事是不是已经有 crate（或 std API）在做


## 发版流程

发版命令都在 `Justfile` 里（`just --list` 可查），实际逻辑在 `scripts/release.sh`。
放进脚本而不是直接写进 Justfile，是因为 just 的 shebang recipe 在 Windows 上需要
`cygpath` 翻译解释器路径，而 Git Bash 并不提供它。

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
just release_check   # cargo clippy --workspace --all-targets --all-features -- -D warnings 与 cargo build --workspace --all-features
just test            # 需要时（等价 make configure debug test，make 部分要在 Git Bash 里跑）
```

有 warning 先修好再提交。确认 `git status` 干净、`main` 已与远程同步。

### 1. 提升版本号

```bash
just release_bump 0.0.5
```

脚本做两件事，并打印每个被改动的文件：

- **Cargo 文件**：取工作区当前版本（开发版本，如 `0.0.5-dev.0`）→ `0.0.5`。
  只涉及根 `Cargo.toml` —— 它既是 workspace 根、又是 `duckfn` 包的清单，也是全仓唯一出现
  字面版本号的地方（`[workspace.package]` 的 `version`，以及 `duckfn-macro` 的精确 pin）。
- **文档 / README / CI 注释**：取**最近一次 tag** 的版本（如 `0.0.4`）→ `0.0.5`。
  涉及的文件由 `git grep` 自动找出，不需要维护清单：
  - `README.md`、`README.zh-CN.md`
  - `Justfile`：注释里的示例命令
  - `.github/workflows/MainDistributionPipeline.yml`：注释里的示例 tag
  - `docs/duckfn-version.ts`：文档站版本号的唯一来源

  文档站的正文（`docs/docs/**`、`docs/i18n/**`）不再出现具体版本号，只写
  `{{DUCKFN_VERSION}}` 占位符，由 `docs/plugins/remark-version-placeholder.ts`
  在构建时替换成 `docs/duckfn-version.ts` 里的值。

最后用 `cargo update -p duckfn -p duckfn-macro` 同步 `Cargo.lock`，并打印残留的旧版本号
（应当为空）以及 `git diff --stat`。

> 示例扩展（`src/extension/**`、`test/sql/**`）与本 crate 同属一个包，没有独立版本号，
> 发版脚本自然会把它们一起带上；`Cargo.lock`、`docs/package-lock.json` 与本文件被排除在替换之外。

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

### 5. 切到下一开发版本

```bash
just release_dev 0.0.6-dev.0
```

这一步只动根 `Cargo.toml` 和 `Cargo.lock`：文档与 README 中的示例
始终指向最新**已发布**版本，不打 tag、不发布。

## 相关文档

- [`README.md`](README.md) / [`README.zh-CN.md`](README.zh-CN.md)：`duckfn` 的 crate README（根 README，同时在 GitHub 首页与 crates.io 上展示）。
- [`src/extension/`](src/extension/) 与 [`test/sql/`](test/sql/)：随包发布的示例扩展与 sqllogictest 用例；[`demo.sh`](demo.sh) 是一组可直接跑的 `just sql` 示例。
- [`scripts/release.sh`](scripts/release.sh)：`release_bump` / `release_dev` / `release_tag` 的实际实现。
- [`docs/duckfn-version.ts`](docs/duckfn-version.ts) 与 [`docs/plugins/remark-version-placeholder.ts`](docs/plugins/remark-version-placeholder.ts)：文档站的版本占位符机制。
- [duckfn-extension-template](https://github.com/shijianjs/duckfn-extension-template)：给下游扩展项目的
  脚手架，骨架、CI、sqllogictest、文档站与发版脚本都已就位；克隆后 `just rename <新扩展名>` 一次改齐
  所有需要一致的名字。原先放在本仓库 `templates/` 下的那套模板已由该仓库取代。
- [`docs/docs/build-and-release.md`](docs/docs/build-and-release.md)：面向读者的构建与发布说明。
- [`docs/docs/contributing.md`](docs/docs/contributing.md)：本地开发流程与约定。
