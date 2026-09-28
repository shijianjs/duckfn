---
title: TOC 折叠
sidebar_position: 4
description: 桌面版目录的折叠控件，一行插件配置即可启用。
---

# TOC 折叠

Docusaurus 对右侧目录没有任何现成开关——`themeConfig.tableOfContents` 只接受标题
级别——所以 kit 补上一个：桌面版目录顶部的折叠/展开按钮。**本页就是演示**：桌面宽度下，
“On this page”上方的那个按钮就是它。

## 接入方式

```tsx
import {dfkTocToggle} from 'duckfn-docs-kit/toc-toggle/plugin';

plugins: [dfkTocToggle()],
```

样式随 kit 的全局 CSS 一起进来：

```css
@import 'duckfn-docs-kit/src/kit.css';
```

插件会把客户端胶水注入到每个页面——初始化一次，之后每次路由切换刷新一遍——所以站点
不需要自己维护 client module。

## 行为

- 折叠后正文切换为全宽；选择会被记住（`localStorage`，键名 `duckfn:toc-collapsed`）。
- 仅在桌面宽度生效，跟随 Docusaurus 自身的目录断点；窗口缩小时按钮与布局类一并移除，
  变回来会重新检查。
- 文案跟随页面语言，内置 `en` 与 `zh-Hans`。
- 没有标题（没有目录）的页面自然什么都不显示。
