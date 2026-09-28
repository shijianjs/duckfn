---
title: 版本占位符
sidebar_position: 6
description: 整个文档树只维护一个版本号——构建期由 remark 插件替换。
---

# 版本占位符

一次发版应该只动一个文件，而不是几十个：文档里到处可以写占位符令牌，
`remarkVersionPlaceholder` 在构建站点时把它替换成真实版本号。下面这个块就是活的——
值来自本站的版本文件：

```toml
duckfn = "{{DUCKFN_VERSION}}"
duckfn-macro = "{{DUCKFN_VERSION}}"
```

## 接入方式

```tsx
import {remarkVersionPlaceholder} from 'duckfn-docs-kit/remark';
import {DUCKFN_VERSION} from './duckfn-version';

// presets -> classic -> docs
remarkPlugins: [
  [remarkVersionPlaceholder, {version: DUCKFN_VERSION}],
],
```

- `version`——要替换进去的字符串，例如 `0.0.13`。
- `placeholder`——站点想用别的令牌时覆盖默认值（默认：
  <code>&#123;&#123;DUCKFN_<wbr/>VERSION&#125;&#125;</code>）。

## 替换规则

- 只做纯文本替换，作用于 `text`、`inlineCode` 与 `code` 节点——包括代码块，所以上面的
  演示显示的是真实版本号。
- MDX 表达式节点与 ESM 节点原样保留，MDX 里的 JavaScript 不会被改写。
- 替换发生在任何内容渲染之前，令牌本身不会出现在页面上。
