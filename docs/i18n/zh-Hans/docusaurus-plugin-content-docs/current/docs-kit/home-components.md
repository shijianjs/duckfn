---
title: 首页组件
sidebar_position: 5
description: dfk-hero / dfk-features / dfk-next-steps 三个 Web 组件——落地页的积木。
---

# 首页组件

落地页由三个 shadow-DOM Web 组件搭成：`<dfk-hero>`（logo、标题、标语、按钮、徽章）、
`<dfk-features>`（区块标题 + 卡片网格）、`<dfk-next-steps>`（链接卡片）。演示就是本站的
[首页](/)——代码展示区之上的全部内容都是这三个组件。

## 内容走 setter

它们是保留模式的元素，不是属性驱动的：结构在构造函数里一次性建好，每个 setter 只改动
自己持有的节点。在 React 里要通过 callback ref 挂载——React 19 只会把字符串 prop 协调到
自定义元素上，对象载荷撑不过 hydration：

```tsx
import {createElement} from 'react';
import type {ReactNode} from 'react';
import {DfkNextSteps, type NextStepItem} from 'duckfn-docs-kit';

function mountSteps(node: HTMLElement): void {
  const steps = node as DfkNextSteps;
  steps.setSectionTitle('Where to go next');
  steps.setSteps([
    {href: '/docs/intro', title: 'Introduction', details: 'What duckfn is.'},
  ] satisfies NextStepItem[]);
}

const element: ReactNode = createElement('dfk-next-steps', {ref: mountSteps});
```

内容类型从桶文件导入——hero 用 `HeroAction`、`HeroBadge`、`HeroLink`，网格用
`FeatureItem`，卡片用 `NextStepItem`：

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

组件在客户端渲染，所以预渲染 HTML 是空壳，hydration 之前是空的。这是刻意的：换来的是
light DOM 恒为空、hydration 零失配，代价只是首屏一次高度跳变。
