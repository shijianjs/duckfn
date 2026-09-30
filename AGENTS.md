# AGENTS.md

本文件记录本仓库的约定与维护流程，供 AI 助手与维护者参考。

## 仓库结构

本仓库是一个 Cargo workspace，而根目录同时就是 `duckfn` 运行时 crate 的包根：

| 路径 | 内容 | 是否发布 |
| --- | --- | --- |
| `/`（根） | `duckfn` 运行时：`src/`、`tests/`、`README.md`、`LICENSE`；根 `Cargo.toml` 同时是 workspace 根 | 是（crates.io） |
| `test/extension/` | 示例扩展的模块树（`demo/`、`functions/`、`types/`），与 `test/sql/` 并列 —— 它是跑在真实 DuckDB 里的用例，不是 `tests/` 下的单元测试，所以放在 `src/` 之外，由 `examples/duckfn.rs` 与 `src/bin/duckfn.rs` 用 `#[path]` 挂进来；默认不编译（见下） | 是（随 `duckfn` 包） |
| `examples/duckfn.rs` | 扩展产物本体：`[[example]] duckfn`，`crate-type = ["cdylib", "staticlib"]` —— 原生扩展取 cdylib，wasm 取 `.a`；入口符号也在这里（见下）。**lib 只有 rlib**，这两个产物不放进 lib 的原因见下 | 是（随 `duckfn` 包） |
| `src/bin/duckfn.rs` | 命令行工具入口（`[[bin]] duckfn-cli`，文件名仍是 duckfn.rs，原因见下）。它只为收集 inventory 注册项而再编一遍示例树 | 是（随 `duckfn` 包） |
| `test/sql/` | 示例的 sqllogictest 用例（41 个 `.test`） | 是（随 `duckfn` 包，只收 `.test`） |
| `duckfn-macro/` | 过程宏 crate | 是（crates.io） |
| `docs/` | Docusaurus 文档站：`docs/docs/**`（英）与 `docs/i18n/zh-Hans/docusaurus-plugin-content-docs/current/**`（中） | 是（随 `duckfn` 包，仅正文源文件） |
| `Makefile` / `Justfile` / `scripts/` | 本地与 CI 的构建入口。Makefile 必须留在仓库根（CI 在根目录执行 `make`）；`Justfile` 只留本仓库特有的 recipe 与覆盖，日常命令在 `scripts/common.just`（下游共享，见下） | — |
| `extension-ci-tools/` | DuckDB 官方 CI 子模块，不要修改它的内容 | — |

示例扩展是**本包的一部分**，不再是独立 crate —— 这个区别是关键：cargo 会无条件跳过任何含
`Cargo.toml` 的子目录，独立打包的示例永远进不了 `duckfn` 的 `.crate`，而并进本包后它随包发布。

### 示例的开关：`quack` feature

示例的模块树由默认关闭的 `quack` feature 控制 —— 它由两个 target 用 `#[path]` 挂进来：
`examples/duckfn.rs`（扩展产物）与 `src/bin/duckfn.rs`（CLI），两者都写了
`required-features = ["quack"]`。`src/lib.rs` **不**挂它，lib 只是纯 rlib。

- **下游**：`duckfn = "0.0.11"` 的依赖树与示例并入前完全一致，示例源码在包里但不参与编译；而且因为
  lib 只有 rlib，下游编 wasm 时也不会被「把 duckfn 的 cdylib 也链一遍」拖下水（见下）。
- **本仓库**：所有构建扩展的命令都必须带上它 —— `make debug`（根 Makefile 里
  `TARGET_INFO += --example $(EXTENSION_NAME) --features quack`）、`cargo build --features quack`、
  `just build`、`just build_wasm`。不带 feature 时 cargo 只是静默跳过目标（产出一个没有入口符号的
  cdylib），`LOAD` 时才报错，很难查。
- `quack = ["all", "owned-connection"]`：示例把每一档可选能力都演示了一遍，包括 `duckfn::duck_vfs`，
  所以除了 `all` 还要显式带上 `owned-connection`（见下）。
- `all` 是给下游用户用的「全开」档，**不含 `owned-connection`（即 `duck_vfs`）**：宿主文件系统在
  浏览器 / DuckDB-Wasm 上不可靠（存在性判定恒真、裸写偏移错），且 native 上能用 DuckDB 自身读写 /
  系统库替代，所以不放进「全开」；需要时下游在 native 场景下单独开 `owned-connection`。`all` 也
  **不**依赖 `quack`（用户不需要示例与那些测试函数）。

### 发布包内容

`duckfn` 发布包的内容由根 `Cargo.toml` 的 `include` 白名单决定：运行时代码与测试、示例扩展
（`test/extension/**`、`src/bin/duckfn.rs`）、`test/sql/**/*.test`、`demo.sh`、
README、LICENSE、文档站正文（英 + 中）。改这个白名单后，用 `cargo package -p duckfn --list`
核对一遍。三条容易踩的坑：

- **模式必须以 `/` 开头**。gitignore 风格里裸的 `LICENSE` / `README.md` 会匹配任意层级的同名
  文件 —— 实测会把 `docs/node_modules/**/LICENSE`、`configure/venv/**/LICENSE` 一起收进包
  （1600 多个文件）。
- **写了 `include` 就绕过 gitignore**。sqllogictest 会在 `test/sql` 下写出 `dfn_file_*`、
  `dfn_copy_*.tsv` 之类的临时文件，所以白名单只收 `*.test`；`test/sql` 里也请只留 `.test`。
- **含 `Cargo.toml` 的子目录一律被跳过**（`duckfn-macro/` 因此进不了包，这是对的：它单独发布）。

### 名字必须一致

DuckDB 扩展名 `duckfn` 必须四处一致：`test/extension/entry.rs` 的 `duckfn_entrypoint!`、根
`Makefile` 的 `EXTENSION_NAME`、CI 的 `extension_name` / `EXTENSION_NAME`，以及
`test/sql/**/*.test` 里的 `require`。它与 crate 名相同不是巧合 —— 原生扩展就是本包 `[[example]]`
产出的 cdylib（wasm 那份是同一个 target 的 staticlib），产物名由 crate 名决定（上游 makefile 按
`lib$(EXTENSION_NAME).*` 取产物），对齐后根 Makefile 一行平台条件都不用写。

两处因此而来的命名细节，改动前先读回来：

- **入口符号单独一个文件**（`test/extension/entry.rs`）：由 `examples/duckfn.rs` 声明一次 —— 那个
  target 同时产出原生 cdylib 与 wasm staticlib，两边都要它。CLI 也编同一棵模块树（`#[path]` 那套），
  但它是 executable、用不上入口符号，而且多一份定义就是重复定义（Windows 上 LNK2005），所以它只包含
  `extension/mod.rs`。
- **CLI 的 bin 目标叫 `duckfn-cli`**（文件仍是 `src/bin/duckfn.rs`）：扩展产物那个 target 叫
  `duckfn`、产物也叫 duckfn，Windows 上两者的 `.pdb` 会撞名（cargo 报 output filename collision）。
  下游项目的包名不同，不会撞，所以模板里那个 bin 依旧叫 `duckfn`。
- **扩展产物为什么是 `[[example]]` 而不是写进 `[lib]` 的 `crate-type`**：cargo 照依赖的
  `crate-type` 办事，cdylib / staticlib 一旦写在 lib 上，**每个下游项目**编 `wasm32-unknown-emscripten`
  时都要把 duckfn 的 cdylib 也链一遍；那条路径下 rustc 不给 emcc 传 `-sSIDE_MODULE=2`，emcc 按独立
  模块链接、去找一个并不存在的 main，直接报
  `libstandalonewasm.a(__main_void.o): undefined symbol: main`。挪到 `[[example]]` 后 lib 只剩 rlib：
  下游既不用多链一个 cdylib，也不用为 wasm 加任何 rustflags（duckfn ≤ 0.0.16 的下游仍需
  `[target.wasm32-unknown-emscripten] rustflags = ["-C","link-arg=-sSIDE_MODULE=2"]` 绕开）。改这块时
  记得同步根 `Makefile` 的 `TARGET_INFO`（要带 `--example $(EXTENSION_NAME)`）与 `IS_EXAMPLE`。

## 共享的 justfile：`scripts/common.just`

`scripts/common.just` 是**下游扩展项目共享的那份 recipe**，也是唯一的源：`duckfn-extension-template`
与由它生成的项目（业务插件等）各自 `import "scripts/common.just"`，副本逐字节相同。本仓库自己也
import 同一份（`Justfile` 里写明 `set allow-duplicate-recipes := true`，再覆盖几条）。

- **改命令只改这里**（`build` / `sql` / `repl` / `lint` / `test` / `docs_*` / `ci-*` /
  `build_wasm*` / `test_wasm` / `release_*` …），然后在各项目跑 `just sync-common` 拉回副本：
  `raw.githubusercontent.com/shijianjs/duckfn/<ref>/scripts/common.just`，默认 ref 是 `main`，
  `DUCKFN_JUST_REF=vX.Y.Z just sync-common` 可钉到某个已发布版本。`just check-common` 只比对不写回，
  不一致时非零退出（下游可以挂进自己的 CI）。
- 各项目根 `Justfile` 只留三类东西：机器相关的 `set windows-shell`、项目相关的 `extension_name`、
  以及本项目特有的 recipe（模板的 `rename`、本仓库的 `publish_*` / `release_kit_*` / `doc`）。
- **覆盖共享 recipe 必须显式开 `set allow-duplicate-recipes := true`**：不开这个开关，重名 recipe 会让
  just 在解析期直接报错，连 `just --list` 都跑不了。本仓库就是这么覆盖 `build` / `release` /
  `build_wasm` / `lint` / `docs_csv` / `release_check` 的（示例与 CLI 挂在 `quack` feature 上）。
- 共享文件里**不写具体版本号**，用 `X.Y.Z` 占位：否则下游副本会被各自的 `scripts/release.sh`
  换个版本号，每次 `just sync-common` 都白白多出一行 diff。
- 它也**不进 crate 包**：根 `Cargo.toml` 的 `include` 白名单里没有 `scripts/`。

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


## 文档站可运行 SQL 的测试

文档里的可运行块（````sql {"type":"duckfn"}````）由 `duckfn-docs-kit` 的 `duckfn-sql-verify`
在 **真实浏览器里的 DuckDB-Wasm** 中真跑一遍（用 `playwright-core` 直驱系统 Chrome/Edge，不下载浏览器），
本站已接成 `npm test`：

```bash
npm test -w docs        # 等价于在 docs/ 下 npm test
```

它收集 `docs/docs/**` 与每个 `i18n/<locale>/…/current/**` 里的可运行块，用站点预加载的扩展
（`docs/static/duckdb-extensions/duckfn.duckdb_extension.wasm`）执行，**每页一个新实例、页内共用
连接**（页内可以依赖前一个块建的宏/表，页与页隔离）。「故意报错」的块靠自己的 meta 声明
（`{"type":"duckfn","expect":"error"}`，默认 `ok`）：期望是数据，不能靠对 SQL 注释做字符串匹配，
而且校验是双向的 —— 声明会失败却跑成功同样会被报出来。块周围的正文与 `-- error:` 注释只写给读者看，
不参与判定。

跑不通或结果不对时，先看这几条（完整版见 `duckfn-docs-kit/CONVENTIONS.md`，下游向的用法说明见
`duckfn-docs-kit/AGENTS.md`，面向读者的说明见 `docs/docs/docs-kit/sql-test.md`）：

- **执行环境是真实浏览器**（DuckDB-Wasm，由 `playwright-core` 驱动），不再是 Node worker。这样才与读者点「Run」时
  的环境一致：浏览器能读远程 `http(s)` 数据（Node worker 读不了、一律 `IO Error: No files found`），
  扩展也按同源 http URL 正常 `LOAD`，没有旧方案的 80 端口 / `~/.duckdb` 暂存目录那套约束。
- **全离线**：引擎（`duckdb-*.wasm` 与 worker 脚本）从 `node_modules/@duckdb/duckdb-wasm/dist` 本地
  serve，扩展用 `docs/static/duckdb-extensions/` 里的副本；浏览器用 `playwright-core` 的 `executablePath`
  拉系统已装的 Chrome/Edge（所以**不下载浏览器**）。找不到时用 `--browser <path>` 或环境变量 `DFK_BROWSER` 指定。
  用 `playwright-core` 而非 `playwright`：前者不会自动下浏览器，恰好适配离线。
- **harness 复用站点运行时**：`sql/harness.ts` 直接调用 `sql/runtime.ts` 的 `DuckDBRuntime`，只是把
  引擎来源从 jsDelivr CDN 换成本地 serve（`init({bundle})`），所以校验走的代码路径与页面渲染一致。
- 平台要配对：默认 `--platform eh` 对应站点预加载的 `duckfn-wasm_eh.duckdb_extension.wasm`。
- `docs/.cache/`（已 git 忽略、不删）存着 DuckDB-Wasm 与扩展 wasm 的本地副本，便于离线排查。

关于**文件系统类示例**：浏览器里 DuckDB-Wasm 的裸文件系统不忠实 —— 打开不存在的文件也「成功」、
读回零填充垃圾，`dfn_file_exists` 等在 wasm 上恒真，写 / 追加字节序也不对（这是平台限制，duckfn
侧无 C API 存在性接口可修，详见 `src/duck_vfs` 模块文档与 `test/sql/functions/duck_vfs.test`）。
所以**落盘 / `output_dir` 相关示例保持普通代码块**（非可运行），页面上注明原因。

它**故意不挂本仓库的 CI**：测试跑的是 wasm 版扩展，而那个产物只有 **duckfn 自己的 CI** 能给出
（官方流水线一次构建 9 个平台产物，约 20 分钟；本机虽然能用 `just build_wasm_eh` 打一份，但它要求
本机 emsdk 与 CI 同版本才加载得了，`docs/static/` 与 `docs/.cache/` 里的仍是取自 release 的副本）。挂进流水线就等于「改一行文档」也要等一轮扩展
构建，而文档的改动频率远高于插件发版，耦合不划算。需要在本地跑时 `npm test -w docs`（约 30 秒；
首次会从 release 取一次扩展文件）。

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
  - `Justfile`：注释里的示例命令（`scripts/common.just` 里的示例一律写 `X.Y.Z` 占位，
    因此它不在替换范围内，下游副本也不会因为发版而漂移）
  - `.github/workflows/MainDistributionPipeline.yml`：注释里的示例 tag
  - `docs/duckfn-version.ts`：文档站版本号的唯一来源

  文档站的正文（`docs/docs/**`、`docs/i18n/**`）不再出现具体版本号，只写
  `{{DUCKFN_VERSION}}` 占位符，由 `docs/plugins/remark-version-placeholder.ts`
  在构建时替换成 `docs/duckfn-version.ts` 里的值。

最后用 `cargo update -p duckfn -p duckfn-macro` 同步 `Cargo.lock`，并打印残留的旧版本号
（应当为空）以及 `git diff --stat`。

> 示例扩展（`test/extension/**`、`test/sql/**`）与本 crate 同属一个包，没有独立版本号，
> 发版脚本自然会把它们一起带上；`Cargo.lock`、`docs/package-lock.json` 与本文件被排除在替换之外。

### 2. 提交并打 tag

```bash
git add -A
git commit -m "chore(release): 发布 vX.Y.Z" \
  -m "- 将 workspace 版本号从 A.B.C 升级到 X.Y.Z" \
  -m "- 同步 duckfn 对 duckfn-macro 的精确依赖版本及文档中的版本示例"
just release_tag 0.0.5     # 打 tag v0.0.5，推送 main 与 tag
```

`release_tag` 会先检查工作区是否干净。推 tag 触发的是 `Main Extension Distribution Pipeline`
（`.github/workflows/MainDistributionPipeline.yml`）：构建各平台扩展，并为该 tag 创建（或更新）
GitHub Release。`Deploy Docs` 不直接挂在 tag 上 —— 它用 `workflow_run` 监听这条流水线，等它整条
成功跑完（含 Release 创建）之后再构建并部署文档站，这样站点预加载的 wasm 就是本次发布的产物，
而不是上一个 release。

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
- [`test/extension/`](test/extension/) 与 [`test/sql/`](test/sql/)：随包发布的示例扩展与 sqllogictest 用例；[`demo.sh`](demo.sh) 是一组可直接跑的 `just sql` 示例。
- [`scripts/release.sh`](scripts/release.sh)：`release_bump` / `release_dev` / `release_tag` 的实际实现。
- [`scripts/common.just`](scripts/common.just)：下游扩展项目共享的 Justfile 片段（唯一的源，各项目
  `import` 一份副本、用 `just sync-common` 同步；本仓库自己也 import 它）。
- [`docs/duckfn-version.ts`](docs/duckfn-version.ts) 与 [`docs/plugins/remark-version-placeholder.ts`](docs/plugins/remark-version-placeholder.ts)：文档站的版本占位符机制。
- [duckfn-extension-template](https://github.com/shijianjs/duckfn-extension-template)：给下游扩展项目的
  脚手架，骨架、CI、sqllogictest、文档站与发版脚本都已就位；克隆后 `just rename <新扩展名>` 一次改齐
  所有需要一致的名字。原先放在本仓库 `templates/` 下的那套模板已由该仓库取代。
- [`docs/docs/build-and-release.md`](docs/docs/build-and-release.md)：面向读者的构建与发布说明。
- [`docs/docs/contributing.md`](docs/docs/contributing.md)：本地开发流程与约定。
- [`docs/docs/docs-kit/sql-test.md`](docs/docs/docs-kit/sql-test.md)、
  [`duckfn-docs-kit/AGENTS.md`](duckfn-docs-kit/AGENTS.md)（随包发布的下游向用法说明）与
  [`duckfn-docs-kit/CONVENTIONS.md`](duckfn-docs-kit/CONVENTIONS.md)（本包开发约定）：文档站可运行
  SQL 的测试 —— 用法与平台约束（真实浏览器 / `playwright-core`、本地 serve 离线、系统 Chrome/Edge、文件系统类示例的限制）。
