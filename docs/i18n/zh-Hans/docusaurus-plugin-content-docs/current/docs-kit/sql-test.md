---
title: 测试示例
sidebar_position: 4
description: 把文档站的每个可运行 SQL 块当作 Playwright 测试跑遍 —— kit 自带收集器、DuckDB-Wasm fixture 与配置 preset。
---

# 测试示例

可运行 SQL 块就是「可执行的文档」，所以它会像测试一样腐坏：扩展函数改了名、默认值变了，而唯一会
发现的地方，是读者点下 **Run** 的那一刻。kit 自带 **Playwright Test** 集成，在站点用的同一套浏览器
运行时里跑每个块，于是文档项目可以在 CI 里过一遍 —— 还能拿到框架的报告、trace 查看器与编辑器集成：

```ts
// playwright.config.mts
import {defineDuckfnDocsConfig} from 'duckfn-docs-kit/sql/playwright';
export default defineDuckfnDocsConfig();
```

```ts
// tests/docs.spec.mts
import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';
declareDocsTests();
```

然后跑 `npx playwright test`（本站接成了自己的 `npm test`，见 `docs/package.json`）。除非你的项目是
`"type": "module"`，否则请用 `.mts` 扩展名：kit 的 dist 是 ESM，CJS 项目会让 Playwright 把 import
转译成 `require()`，在 kit 的 `import.meta` 上报错。

它**故意没有挂进本仓库的 CI**：测试跑的是 wasm 版扩展，而那个产物只有 duckfn 自己的 CI 能给出
（官方流水线一次构建 9 个平台产物，约 20 分钟）。这个取舍记录在仓库根的 `AGENTS.md` 里。

## 它做了什么

1. **收集** —— 遍历内容目录，取出 info string 是可运行配置的所有围栏块（`sql/collect`）。meta 契约
   与构建期的 `sql/remark` 是同一份，因此这里拿到的正是站点发布出去的那些块：站点上不是可运行块
   的，这里也不会收。
2. **执行** —— `declareDocsTests()` 为每个块注册一条 Playwright test，按内容文件分组。每个文件一个
   page，页内各块共用该 page 的 DuckDB-Wasm 连接（每页一个实例、页内共用连接），所以一个块里的
   `CREATE` 下一个块看得见，而页与页之间互相隔离。文件内串行，文件间并行 worker。页面本身
   （`sql/harness`）直接复用站点跑的 `sql/runtime`。
3. **报告** —— 用 Playwright 自己的 reporter（`list`、含 trace 的 `html`、`junit`、`github`）；
   VS Code 测试树会列出每个块，可单独运行 / 调试。

## 故意失败的块

文档里有一半示例的最后一条语句是在演示报错，这种块必须自己声明 —— `test.fail()` 校验双向：本来会
失败的块开始成功，与本来正常的块开始失败，一样会被报出来：

```sql {"type":"duckfn","expect":"error"}
SELECT CAST('abc' AS INTEGER);  -- 报错：not an integer: "abc"
```

`"expect"` 默认是 `"ok"`，`"error"` 表示**这个块必须失败**。套件只读这个字段：块周围的正文、
块内 `-- 报错：…` 注释都是写给读者看的，不参与判定。

## 配置

配置来自 `sql/site`：显式选项优先，其次 `DFK_*` 环境变量，最后是探测到的站点布局。

| 环境变量 | 含义 |
| --- | --- |
| `DFK_SITE_DIR` | 文档站根目录（默认：当前工作目录）。 |
| `DFK_CONTENT` | 逗号分隔的内容目录，相对站点根。默认 `docs/` 加上每个 `i18n/<locale>/docusaurus-plugin-content-docs/current/`。 |
| `DFK_EXTENSION` | 要预加载的扩展：`.duckdb_extension.wasm` 路径，或绝对 `http(s)` URL。默认取 `static/duckdb-extensions/` 下的那一个文件。 |
| `DFK_ASSETS` | 逗号分隔的 `url=dir`：运行块时额外通过 HTTP 供出的本地目录（见下一节）。 |
| `DFK_PLATFORM` | DuckDB-Wasm bundle，必须与扩展的构建平台一致（默认 `eh`）。 |
| `DFK_ENGINE` | 覆盖引擎 wasm，用来钉住某个 duckdb-wasm 构建。 |
| `DFK_BROWSER` | 要驱动的 Chrome/Edge 可执行文件。默认探测系统里的 Chrome/Edge，Playwright 直接拉起它，不下载浏览器。 |
| `DFK_TIMEOUT` | 单块超时（毫秒，默认 30000）—— 卡住的块会作为失败上报，而不是把 CI 挂住。 |

`defineDuckfnDocsConfig()` 也接受这些选项（`{siteDir, contentDirs, …}`）外加一个最后合并的 `config`
字段，因此 preset 设的任何东西都能在自己的 `playwright.config.mts` 里覆盖。

## 读取站点自带的示例数据（静态资源映射）

harness 从一个短命的 loopback 服务里供出页面、引擎、worker 与扩展——除此之外什么都没有。因此，一个
读取站点自带数据文件（比如 `static/` 下的 `.tsv`）的块会拿到 404。把每个块要读的目录声明出来，服务就
一并供出：

```ts
// tests/docs.spec.mts
import {fileURLToPath} from 'node:url';
import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';

declareDocsTests({
  siteDir: fileURLToPath(new URL('..', import.meta.url)),
  baseUrl: '/my-site/',
  assets: [{url: '/my-site/data', dir: 'static/data'}],
});
```

- `url` 是文件可被访问的**根相对**前缀：站点的 `baseUrl` 加上目录名，也就是部署后站点真正提供的
  路径。它必须是一个能前面接上页面 URL 的前缀，因为 DuckDB-Wasm 什么都不按相对路径解析（见下面的
  占位符）：

  ```sql
  SELECT count(*) AS n FROM read_csv_auto('{{DFK_BASE_URL}}data/samples.tsv');
  ```

- `dir` 相对站点根（`siteDir`），这也是为什么 `assets` 旁仍要 `siteDir`。
- `baseUrl` 是 `{{DFK_BASE_URL}}` 展开成的值，必须与上面那些前缀一致。harness 没有 locale，
  所以一个前缀要同时服务所有语言的块。
- 路径被限制在 `dir` 之内：URL 里的 `..` 永远到不了外层，且只会供出普通文件。

Playwright 选项、CLI 参数与环境变量表达的是同一件事：`duckfn-sql-verify` 上写
`--asset /my-site/data=static/data`（可重复）加 `--base-url /my-site/`，或
`DFK_ASSETS=/my-site/data=static/data` 与 `DFK_BASE_URL=/my-site/`。

## `{{DFK_BASE_URL}}`：数据 URL 为什么需要占位符

DuckDB-Wasm 跑在一个 base URL 为 `blob:` 的 Worker 里，因此**任何**相对路径都不会拿页面当基准去解析。
下面两种写法都会以同一个 `IO Error: No files found that match the pattern` 失败：

```sql
SELECT * FROM read_csv_auto('data/samples.tsv');    -- 在内存文件系统里找
SELECT * FROM read_csv_auto('/data/samples.tsv');   -- 同样是路径，不是 URL
```

只有绝对的 `http(s)` URL 才会走 HTTP 文件系统，而部署前缀恰恰是构建期替换无法知道的那一部分（GitHub Pages、
`docusaurus serve`、harness 的随机 loopback 端口各不相同）。所以有两个占位符，由 `sql/runtime` 在
`execute()` 里、SQL 交给 DuckDB 之前展开：

| 占位符 | 展开成 | 用来写 |
| --- | --- | --- |
| `{{DFK_ORIGIN}}` | `https://example.github.io` | 不在站点 baseUrl 下的 URL（外部数据集）。 |
| `{{DFK_BASE_URL}}` | `https://example.github.io/my-site/` —— origin 加上**当前页面**的 baseUrl | 站点自己的文件，即 `static/` 下的东西。 |

站点自己的文件一律用 `{{DFK_BASE_URL}}`，**绝不把前缀写死在块里**：Docusaurus 会把 `static/` 复制进
*每个 locale* 的输出，于是同一个 `data/samples.tsv` 在英文页是 `/my-site/data/samples.tsv`、在中文页是
`/my-site/zh-Hans/data/samples.tsv` —— 写死前缀的块在一个语言下能跑、另一个语言下 404。baseUrl 由
`dfkExtensions` 注入的配置标签带进浏览器（按 locale 本地化），harness 侧则来自 `baseUrl` /
`DFK_BASE_URL` / `--base-url`。

站点与 harness 共用 `execute()` 这一个入口，所以在 CI 里验证过的块与读者跑的是同一个块。`<dfk-sql>`
在填编辑器时也解析这两个占位符，所以读者看到（并可以复制）的是真正会去抓的那个 URL，而不是占位符。

## 命令行回退

需要无框架运行时，kit 仍提供 `duckfn-sql-verify`，它与 Playwright 路径共用收集器、harness 与配置解析：

```bash
duckfn-sql-verify --site . --asset /my-site/data=static/data
```

它依赖 `playwright-core`（kit 的依赖），块行为与声明不符时返回非零退出码。想要报告与 IDE 支持时，
优先用 Playwright Test。

## 为什么是这样跑的

块跑在**真实浏览器里的 DuckDB-Wasm** 中——也就是读者拿到的环境——而不是 Node。这是有意为之：
旧的 Node worker 读不了远程 `http(s)` 数据（每个远程数据示例都报 `IO Error`），所以依赖真实
远程文件的块以前只能靠人工在浏览器里验证。把套件搬到浏览器里跑，就把这件事变成了常规。

浏览器是用 **Playwright** 驱动的，协议、导航、自动等待、超时与崩溃处理都交给这个成熟库，而不是自己写
一个驱动。它直接用 `launchOptions.executablePath` 拉起**系统里的 Chrome/Edge**，所以运行期不下载浏览器。
其余东西全部本地供出，所以能离线跑：引擎（`duckdb-*.wasm` 及其 worker 脚本）来自 `node_modules`，
扩展来自 `static/duckdb-extensions/`，harness 页面从一个 loopback http 服务把它们取过来。经 http
加载扩展与站点上的做法一模一样，没有任何端口 / 暂存目录约束（那是 Node worker 才有的东西）。

```sql
-- 像这样的远程读取，是 Node worker 从来做不到、浏览器却能做成的。
-- （这里不是可运行块：拉取它需要网络，而套件是离线跑的。）
SELECT count(*) AS n FROM read_csv_auto('https://example.com/data/smallest.csv');
```

### 文件系统类示例是例外

浏览器的裸文件系统不是忠实的 POSIX 层：DuckDB-Wasm 会把一个从未写过的文件当“能打开”、返回零
填充的字节，所以 `dfn_file_exists` 对不存在的东西也报 `true`，`COPY … TO` / `append` 也不会按你预期的
方式落字节。DuckDB 的 C API 没有可供扩展纠正这一点的存在性接口，所以这是平台限制而非 bug——
依赖文件系统的示例保持**普通（非可运行）代码块**，并在页面上注明原因。原生构建（以及原生用例
`test/sql/functions/duck_vfs.test`）则行为正确。