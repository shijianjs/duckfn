---
title: 测试示例
sidebar_position: 4
description: 在 CI 里跑遍文档站的所有可运行 SQL 块 —— kit 自带收集器、DuckDB-Wasm 运行器与 duckfn-sql-verify 命令。
---

# 测试示例

可运行 SQL 块就是「可执行的文档」，所以它会像测试一样腐坏：扩展函数改了名、默认值变了，而唯一会
发现的地方，是读者点下 **Run** 的那一刻。kit 自带站点用的同一个运行器，于是文档项目可以在 CI 里
把每个块都过一遍：

```bash
duckfn-sql-verify --site .
```

本站把它接成了自己的 `npm test`（见 `docs/package.json`），在 `docs/` 下跑 `npm test` 就是全部
流程。它**故意没有挂进本仓库的 CI**：测试跑的是 wasm 版扩展，而那个产物只有 duckfn 自己的 CI 能
给出（官方流水线一次构建 9 个平台产物，约 20 分钟）。这个取舍记录在仓库根的 `AGENTS.md` 里。

## 它做了什么

1. **收集** —— 遍历内容目录，取出 info string 是可运行配置的所有围栏块（`sql/collect`）。meta 契约
   与构建期的 `sql/remark` 是同一份，因此这里拿到的正是站点发布出去的那些块：站点上不是可运行块
   的，这里也不会收。
2. **执行** —— 在**无头浏览器**里的 DuckDB-Wasm 中跑每个块，并预加载站点的扩展（`sql/browserRunner`
   驱动浏览器，它加载的页面 `sql/harness` 直接复用站点跑的 `sql/runtime`），架构与页面一致：
   每页一个实例、页内各块共用一条连接（所以一个块里的 `CREATE` 下一个块看得见），而页与页之间互相
   隔离。
3. **报告** —— 打印失败项，有失败就返回非零退出码（`sql/verify`）。

## 故意失败的块

文档里有一半示例的最后一条语句是在演示报错，这种块必须自己声明 —— 校验是双向的：本来会失败的块开始
成功，与本来正常的块开始失败，一样会被报出来：

```sql {"type":"duckfn","expect":"error"}
SELECT CAST('abc' AS INTEGER);  -- 报错：not an integer: "abc"
```

`"expect"` 默认是 `"ok"`，`"error"` 表示**这个块必须失败**。套件只读这个字段：块周围的正文、
块内 `-- 报错：…` 注释都是写给读者看的，不参与判定。

## 选项

| 选项 | 含义 |
| --- | --- |
| `--site <dir>` | 文档站根目录（默认：当前工作目录）。 |
| `--content <dir>` | 相对站点根的内容目录，可重复。默认 `docs/` 加上每个 `i18n/<locale>/docusaurus-plugin-content-docs/current/`。 |
| `--extension <路径\|URL>` | 要预加载的扩展：`.duckdb_extension.wasm` 路径，或绝对 `http(s)` URL。默认取 `static/duckdb-extensions/` 下的那一个文件。 |
| `--platform <eh\|mvp>` | DuckDB-Wasm bundle，必须与扩展的构建平台一致（默认 `eh`，也就是当前浏览器里 `selectBundle()` 会选的那个）。 |
| `--engine <路径>` | 覆盖引擎 wasm，用来钉住某个 duckdb-wasm 构建。 |
| `--browser <路径>` | 要驱动的 Chrome/Edge 可执行文件。默认探测系统里的 Chrome/Edge，或读环境变量 `DFK_BROWSER`。`playwright-core` 直接拉起它，不下载浏览器。 |
| `--timeout <毫秒>` | 单块超时（默认 30000）—— 卡住的块会作为失败上报，而不是把 CI 挂住。 |
| `--report <文件>` | 把逐块结果写成 JSON。 |
| `--quiet` | 只报告非预期失败。 |

## 为什么是这样跑的

块跑在**真实浏览器里的 DuckDB-Wasm** 中——也就是读者拿到的环境——而不是 Node。这是有意为之：
旧的 Node worker 读不了远程 `http(s)` 数据（每个远程数据示例都报 `IO Error`），所以依赖真实
远程文件的块以前只能靠人工在浏览器里验证。把套件搬到浏览器里跑，就把这件事变成了常规。

浏览器是用 **Playwright**（`playwright-core`）驱动的，协议、导航、自动等待、超时与崩溃处理都交给这个成熟
库，而不是自己写一个驱动。特意用 `playwright-core` 这个包，因为**它不会自动下浏览器**——直接用
`executablePath` 拉起系统里的 Chrome/Edge。其余东西全部本地供出，所以能离线跑：引擎（`duckdb-*.wasm` 及其
 worker 脚本）来自 `node_modules`，扩展来自 `static/duckdb-extensions/`，harness 页面从一个
 loopback http 服务把它们取过来。经 http 加载扩展与站点上的做法一模一样，没有任何端口 / 暂存目录
约束（那是 Node worker 才有的东西）。

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
