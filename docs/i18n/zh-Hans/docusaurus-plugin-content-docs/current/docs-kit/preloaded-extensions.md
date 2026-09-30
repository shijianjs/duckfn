---
title: 扩展预加载
sidebar_position: 3
description: 每一页都加载 DuckDB-Wasm 扩展——一条有序预加载列表，构建期从 GitHub release 拉取，按 sha256 缓存。
---

# 扩展预加载

可运行 SQL 块可以按块声明扩展（见[可运行 SQL 块](./runnable-sql.md)），但一个只文档化
单一扩展的站点，不该让每个示例都写一遍扩展名。`dfkExtensions` 插件改为接收一条**有序
预加载列表**：

- dev/build 启动时，把列表里每个 GitHub release 来源拉进站点的静态目录——本地缓存，
  仅在 release 资产变化时重新下载；
- 把解析后的列表注入每个页面；
- kit 的运行时在 DuckDB 初始化期间按序加载——页面打开（只要页上有可运行块）就在后台
  开始，所以第一次点 **执行** 很快。

块里直接调用扩展即可，无需声明：

```sql {"type":"duckfn"}
SELECT version() AS engine, double_it5(21) AS doubled;
```

## 接入方式

```tsx
import {dfkExtensions} from 'duckfn-docs-kit/sql/extensions';

plugins: [
  dfkExtensions({
    // CI 构建的 release 资产没有 DuckDB 的签名密钥——与本地开发用
    // `duckdb -unsigned` 是同一个原因。
    allowUnsignedExtensions: true,
    preload: [
      // 本站文档化的扩展：同源提供。带 release 时构建期去该仓库的最新 release 取资产；
      // 去掉 release 就是直接用 static/ 下放好的文件。
      {
        url: 'duckdb-extensions/duckfn.duckdb_extension.wasm',
        release: {
          repository: 'shijianjs/duckfn',
          asset: 'duckfn-wasm_eh.duckdb_extension.wasm',
        },
      },
    ],
  }),
],
```

拉取在 `npm start` 与生产构建时都会发生 —— 但只针对带 `release` 的条目。只写 `{url}` 的条目会被
原样放过，所以一个站点可以本地用自己的构建产物、只在部署时才切到 release：duckfn 自己的站点就是
这么做（用 `DOCS_EXTENSION_FROM_RELEASE` 区分，见它的 `docs/README.md`），因此本地跑文档从不接触
release。

## 三种来源

1. **扩展名**——`'json'`：运行时对官方仓库执行 `LOAD json`。
2. **名字 + 仓库**——`{name: 'h3', repository: 'community'}`：仓库可以是 `community`、
   `core` 或 URL。运行时先用 `INSTALL … FROM` 记录来源，再 `LOAD` 名字。在 WebAssembly
   上 `INSTALL` 不落盘——没有可安装的持久存储——它只记录 `LOAD` 该从哪里取；也正因如此，
   这条记录只附着在这一个扩展上，不会污染之后的加载。
3. **文件**——`{url: …, release?: …}`：由站点自己分发 wasm 文件。带 `release` 时，构建期
   从该 GitHub 仓库的**最新** release 拉取资产到 `static/<url>`（与运行时加载的路径同一个）；
   不带 `release` 时，文件由手工放在 `static/<url>`，构建只检查它在不在。`url` 也可以是
   绝对 `http(s)` URL。

## GitHub release 来源

对 `{url, release}` 条目，插件会：

- 调 GitHub API 取仓库最新 release，按名字找资产（找不到会报错并列出可用名字）；
- 用资产的 sha256——GitHub 自带的 `digest` 字段——与本地缓存比对，**只有不同才下载**；
- 下载后按该 digest 校验，写入 `<siteDir>/.cache/duckfn-docs-kit/` 并拷贝到 `static/<url>`；
- 网络不可用时降级为使用缓存并给出警告，离线开发不中断（CI 每次全新环境、无缓存，
  会直接失败）。

`GITHUB_TOKEN`（或插件的 `token` 选项）可以解除匿名 API 的速率限制；公开仓库在普通
开发机上不需要它。

## 文件名的契约

文件名**第一个 `.` 之前**的文字就是 DuckDB 查入口符号用的名字——`duckfn.duckdb_extension.wasm`
经 `duckfn_init_c_api` 加载。release 资产带平台后缀（`duckfn-wasm_eh.duckdb_extension.wasm`），
所以落盘时要改名：`url` 里的目标文件名说了算；插件与运行时都会校验“首个 `.` 之前”必须是
合法的扩展标识符。

## 版本与签名 {/* #versions-and-signing */}

- **平台**：预加载的文件必须与运行时 bundle 的 WebAssembly 平台一致。让
  `selectBundle()` 自动选择的站点提供 `wasm_eh` 资产；本站就是这样。
- **DuckDB 版本**：WebAssembly 扩展只能被 C API 兼容的 DuckDB-Wasm 构建加载。站点把
  `@duckdb/duckdb-wasm` 固定在精确版本，其内置引擎与 CI 构建扩展所用的 `duckdb_version`
  一致；任何一侧变动时，回到这些页面重新跑一遍可运行块即可验证。
- **签名**：第三方 release 资产没有用 DuckDB 的密钥签名，因此预加载它的站点需要
  `allowUnsignedExtensions: true`（相当于 CLI 的 `-unsigned`）。社区扩展是签过名的，
  无需打开。

## 它落在页面的哪里

插件把解析后的列表作为唯一一个 JSON `<script>` 标签注入每个页面：

```json
{"allowUnsignedExtensions":true,"preload":[{"url":"/duckdb-extensions/duckfn.duckdb_extension.wasm"}]}
```

标签的 `id="dfk-sql-runtime"` 保持不变，调试时可以直接在页面源码里查看。列表格式不对时，
最先运行查询的块会以可读错误呈现 DuckDB 初始化失败。
