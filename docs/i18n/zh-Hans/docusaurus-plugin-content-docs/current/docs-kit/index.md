---
title: 总览
slug: /docs-kit
sidebar_position: 1
description: duckfn-docs-kit 是什么、包含哪些能力，以及文档站怎样接入。
---

# 总览

`duckfn-docs-kit` 是本文档站的共享基础库：可运行 SQL 块、扩展预加载、TOC 折叠控件、
首页 Web 组件与版本占位符——凡是可以从 duckfn 系扩展文档站里沉淀、不该被复制的部分，
都在这里，各自作为独立入口，由站点接进自己的 Docusaurus 配置。

本包用 TypeScript 直接封装浏览器原生 DOM（自身不依赖 React，也不引入任何 UI 框架），
面向其它扩展文档站的复用而维护。它已按可发布形态准备、稍后发布到 npm；在此之前，
本仓库以 workspace 依赖的方式消费它，所以下面这些页面的内容与你可以直接读到的源码一致。

## 包里有什么

| 能力 | 入口 | 页面 |
| --- | --- | --- |
| 可运行 SQL 块 | `duckfn-docs-kit/sql/remark` + `<dfk-sql>` 元素 | [可运行 SQL 块](./runnable-sql.md) |
| 扩展预加载 | `duckfn-docs-kit/sql/extensions` | [扩展预加载](./preloaded-extensions.md) |
| TOC 折叠控件 | `duckfn-docs-kit/toc-toggle/plugin` | [TOC 折叠](./toc-toggle.md) |
| 首页组件 | `duckfn-docs-kit`（桶文件） | [首页组件](./home-components.md) |
| 版本占位符 | `duckfn-docs-kit/remark` | [版本占位符](./version-placeholder.md) |

## 接入方式

安装本包，然后把站点需要的入口接进 `docusaurus.config.ts`：

```bash
npm install duckfn-docs-kit
```

```tsx
import {dfkExtensions} from 'duckfn-docs-kit/sql/extensions';
import {dfkTocToggle} from 'duckfn-docs-kit/toc-toggle/plugin';
import {remarkVersionPlaceholder} from 'duckfn-docs-kit/remark';
import {remarkRunnableSql} from 'duckfn-docs-kit/sql/remark';
import {DUCKFN_VERSION} from './duckfn-version';

export default {
  presets: [
    [
      'classic',
      {
        docs: {
          remarkPlugins: [
            [remarkVersionPlaceholder, {version: DUCKFN_VERSION}],
            remarkRunnableSql,
          ],
        },
      },
    ],
  ],
  plugins: [
    dfkExtensions({
      allowUnsignedExtensions: true,
      preload: [
        {
          url: 'duckdb-extensions/duckfn.duckdb_extension.wasm',
          release: {
            repository: 'shijianjs/duckfn',
            asset: 'duckfn-wasm_eh.duckdb_extension.wasm',
          },
        },
      ],
    }),
    dfkTocToggle(),
  ],
};
```

全局 CSS 只需在站点自己的样式表里引一行：

```css
/* src/css/custom.css */
@import 'duckfn-docs-kit/src/kit.css';
```

`kit.css` 聚合了 `--duckfn-*` 品牌 tokens 与那些没法待在 shadow 里的样式（TOC 折叠）；
`dfk-*` 组件自带的样式随 JS bundle 注入各自的 shadow root，无需在这里额外引。

## 站点不需要自己维护的东西

- **不需要为 kit 写 client module**：两个插件会把浏览器侧的胶水（`dfk-*` 元素注册与
  TOC 折叠）注入每个页面。
- **不需要在每个块里重复声明扩展**：一条有序预加载列表覆盖站点所文档化的扩展。
- **不需要复制页面骨架**：落地页的 hero、特性网格与链接卡片都是 Web 组件，通过 setter
  喂内容；每一页的目录折叠来自 kit 的 CSS 与注入的客户端胶水。

## 环境要求

- Docusaurus 3.x、Node ≥ 20。
- 可运行块与扩展预加载把 `@duckdb/duckdb-wasm` 固定在精确版本；该版本必须与站点分发的
  扩展 ABI 兼容，见[扩展预加载](./preloaded-extensions.md#versions-and-signing)。
