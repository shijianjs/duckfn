---
title: 首页组件
sidebar_position: 5
description: dfk-hero / dfk-features / dfk-next-steps 三个 Web 组件——落地页的积木。
---

# 首页组件

落地页由三个 shadow-DOM Web 组件搭成：`<dfk-hero>`（logo、标题、标语、按钮、徽章）、
`<dfk-features>`（区块标题 + 卡片网格）、`<dfk-next-steps>`（链接卡片）。演示就是本站的
[首页](/)——代码展示区之上的全部内容都是这三个组件。

## 内容写在标签里

组件内部是保留模式——结构在构造函数里一次性建好，每个 setter 只改动自己持有的节点——
但输入就是普通标记。这正是重点：同一套标签放进纯 `.md` 页面也能用，你一行 JavaScript
都不用写。

标量走属性，长文案走子标签的文本：

```html
<dfk-hero logo-src="img/duckfn-logo.svg" title="duckfn" tagline="A Rust framework…">
  <dfk-action kind="primary" href="/docs/intro">Get started</dfk-action>
  <dfk-action kind="secondary" href="https://github.com/shijianjs/duckfn"
              icon="simple-icons:github">GitHub</dfk-action>
  <dfk-badge href="https://crates.io/crates/duckfn" src="…" alt="duckfn on crates.io"></dfk-badge>
</dfk-hero>

<dfk-features section-title="Why duckfn">
  <dfk-feature icon="lucide:sparkles" title="No C/C++ glue code">
    DuckDB's C types never appear in your code…
  </dfk-feature>
</dfk-features>

<dfk-next-steps section-title="Where to go next">
  <dfk-step href="/docs/intro" title="Quick start">
    Write, build and load your first extension.
  </dfk-step>
</dfk-next-steps>
```

`href` 和 `src` 写站点根路径（`/docs/intro`）即可，和 Docusaurus 的链接一样：组件自己
补当前页面的 baseUrl（`/duckfn/`，中文页面是 `/duckfn/zh-Hans/`），所以同一份标记在任何
locale、任何部署前缀下都对。绝对 URL 原样保留，已经带前缀的路径也不会重复拼接。

结构也可以用 JSON 属性——`primary`、`secondary`、`badges`、`features`、`steps`——由
`attributeChangedCallback` 解析后交给同一批 setter。这在 JSX 里最省事
（`badges={JSON.stringify(badges)}`），手写 Markdown 时则不可读（何况 MDX 不接受单引号
属性值，JSON 里的引号只能写成 `&quot;` 实体）。手写页面用子标签。

子标签（`dfk-action`、`dfk-badge`、`dfk-feature`、`dfk-step`）**不是**自定义元素，只是
数据载体：父元素在 connect 时读一次，然后渲染自己的 shadow 树。`kit.css` 把它们隐藏
（`dfk-*:defined > * { display: none }`）而不是删掉，因为子节点由 React 管理，删掉会让
fiber 树和真实 DOM 对不上；元素升级之前，它们以纯文本存在，而不是空白。

这些标签已声明进 `JSX.IntrinsicElements`，所以同一份标记放进 `.mdx` 页面也能用——本站
首页就是如此——不用 `createElement`，也不用 ref：

```tsx
<dfk-next-steps section-title="Where to go next">
  <dfk-step href="/docs/getting-started/quick-start" title="Quick start">
    Write, build and load your first extension.
  </dfk-step>
</dfk-next-steps>
```

属性只能装字符串，所以 React 页面要把译文塞进属性时先用 `translate({id, message})`
算出字符串；子标签文本则可以是任意 React 节点。本站的做法是每个语言一份页面，
文案不再走 `code.json`。

命名 setter（`setTitle`、`setBadges`……）仍然保留，也仍然是唯一操作 shadow 树的代码——
属性和子标签最后都汇入它们。导出的值类型就是 JSON 属性解析出来的形状：hero 用
`HeroLink`、`HeroAction`、`HeroBadge`，网格用 `FeatureItem`，卡片用 `NextStepItem`。

```tsx
import type {FeatureItem, HeroAction, HeroBadge, HeroLink, NextStepItem} from 'duckfn-docs-kit';
```

## 注册元素

`dfk-*` 标签需要在每次应用启动时定义一次：`registerDfkElements()`——它同时注册每个字形
都在用的官方 `<iconify-icon>` 元素。配置了扩展预加载的站点会在每个页面自动获得注册；
只使用首页组件的站点，在首页模块作用域里自己调一次即可：

```tsx
import {registerDfkElements} from 'duckfn-docs-kit';

registerDfkElements();
```

## 样式与主题

- 组件自带的样式随 JS bundle 走进各自的 shadow root（`adoptedStyleSheets`），两边互不泄漏。
- `--duckfn-*` 品牌 tokens 来自 `src/theme/tokens.css`，随 `kit.css` 一起进来；站点的
  Infima（`--ifm-*`）变量会继承进 shadow 树，暗色模式与站点配色自动生效。
- 图标就是 Iconify 名字（`'lucide:sparkles'`），由官方 `<iconify-icon>` 渲染；尺寸跟随
  `font-size`。

## 已知取舍

组件在客户端渲染，所以预渲染 HTML 是一个「语义化外壳」：子标签里的文本在、可读，但没有
样式，元素升级后会被替换。light DOM 留在原处会多一次样式切换，换来的是 hydration 零失配。
