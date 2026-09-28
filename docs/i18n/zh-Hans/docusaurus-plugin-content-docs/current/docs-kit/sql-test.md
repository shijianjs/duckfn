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

本站把它接成了自己的 `npm test`（见 `docs/package.json`），CI 与本地在 `docs/` 下跑 `npm test`
就是它。

## 它做了什么

1. **收集** —— 遍历内容目录，取出 info string 是可运行配置的所有围栏块（`sql/collect`）。meta 契约
   与构建期的 `sql/remark` 是同一份，因此这里拿到的正是站点发布出去的那些块：站点上不是可运行块
   的，这里也不会收。
2. **执行** —— 在 DuckDB-Wasm 里跑每个块，并预加载站点的扩展（`sql/nodeRunner`），架构与页面一致：
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
| `--timeout <毫秒>` | 单块超时（默认 30000）—— 卡住的块会作为失败上报，而不是把 CI 挂住。 |
| `--working-dir <目录>` | 块运行时的工作目录。默认用一个临时目录、跑完即删：块可能 `COPY … TO 'a.csv'`，而 Node 上那会落到工作目录里。 |
| `--report <文件>` | 把逐块结果写成 JSON。 |
| `--quiet` | 只报告非预期失败。 |

## 为什么是这样跑的

块跑在 DuckDB-Wasm 的 **Node worker target**（`duckdb-node.cjs`）上，而不是 blocking 那个：注册期
会自行打开连接的扩展（正好就是能用文件系统的那些）会让「同一个线程里同步执行 DuckDB」的运行时死锁。
浏览器里看不到这个问题，因为那边的扩展加载发生在 worker 线程内。

扩展进入运行器的方式与进入页面完全相同：走 **http URL**，并放宽签名校验。由此带来两个约束，都源自
DuckDB 存放已拉取扩展的路径（`~/.duckdb/extensions/<host>/<URL 一级路径段>/`）：URL 必须带一层路径
段，且运行器要预先建好该目录（加载器自己的 `mkdir` 不是递归的）。Windows 上本地服务用 80 端口，好让
URL 里没有端口 —— 冒号在 Windows 路径里非法，而这个暂存目录是按 URL 命名的；其它平台用任意空闲端口。
