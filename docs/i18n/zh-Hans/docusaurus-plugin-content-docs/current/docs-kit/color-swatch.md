---
title: 色块
sidebar_position: 8
description: 内容整体是一个颜色值的行内代码，由 rehype 插件直接画上该颜色。
---

# 色块

`#E69F00` 放在行内代码里，看起来和 `x_axis.wrap` 一模一样：Infima 把它们都画成灰色小标签。
`remarkColorSwatch` 插件把颜色铺到文字背后，于是色板表或主题的颜色清单读起来就是颜色本身：

| 色板 | N | 颜色 |
| --- | --- | --- |
| `wong` | 8 | `#E69F00` `#56B4E9` `#009E73` `#F0E442` `#0072B2` `#D55E00` `#CC79A7` `#000000` |
| `tol_bright` | 7 | `#4477AA` `#EE6677` `#228833` `#CCBB44` `#66CCEE` `#AA3377` `#BBBBBB` |
| `category10` | 10 | `#1f77b4` `#ff7f0e` `#2ca02c` `#d62728` `#9467bd` `#8c564b` `#e377c2` `#7f7f7f` `#bcbd22` `#17becf` |

那些单元格就是普通的行内代码——页面里没有任何标记。前景色在黑白之间按**对比度**（WCAG 比值）
选取，所以 `#F0E442` 用深色字、`#0072B2` 用浅色字，两种配色模式下都是如此。

## 配置

```tsx
import {rehypeColorSwatch} from 'duckfn-docs-kit/color-swatch/rehype';

// presets -> classic -> docs
rehypePlugins: [
  rehypeColorSwatch,
],
```

它跑在 **rehype** 阶段，而不是 remark：色块是落在成品 HTML 上的 `style` *属性*，而 React——
负责渲染被标记的 `<code>` 所变成的 JSX 元素——不接受字符串形式的 `style` prop
（`The 'style' prop expects a mapping from style properties to values`）。

只有一个选项：

- `scan` —— 给**所有**内容整体是颜色值的行内代码上色（默认 `true`）。某个页面里行内代码经常
  长得像颜色，就关掉它，改用手动标记：

  ```tsx
  rehypePlugins: [[rehypeColorSwatch, {scan: false}]],
  ```

## 会上哪些颜色

扫描接受 [`colord`](https://www.npmjs.com/package/colord) 开箱即解析的一切——
`#rgb`、`#rgba`、`#rrggbb`、`#rrggbbaa`、`rgb()` / `rgba()`、`hsl()` / `hsla()`：

| 你写 | 结果 |
| --- | --- |
| `` `#E69F00` `` | 一个色块 |
| `` `#0072B2` `` · `` `#0af` `` | 一个色块 |
| `` `rgb(0, 114, 178)` `` · `` `hsl(210, 100%, 35%)` `` | 一个色块 |
| `` `rgba(0, 0, 0, 0.4)` `` | 一个色块，先变成不透明 |
| `` `x_axis.wrap` `` · `` `#section` `` · `` `10px` `` | 原样不动 |
| `` `red` `` · `` `white` `` · `` `transparent` `` | 原样不动——见下文 |

两条刻意的限制：

- **CSS 颜色名一律不认**——`red`、`white`、`transparent` 在正文里就是普通单词，要区分「颜色名」
  和「单词」得加载一张颜色表（`colord/plugins/names`），而插件没有加载它。请写十六进制、
  `rgb()` 或 `hsl()`。
- **不往站点 CSS 里加任何东西。** 全部效果就是那两条行内样式，所以即使站点从没引入过
  `kit.css`，页面也是对的；边框、圆角与内边距仍是站点自己 `code` 样式的设定。

## 手动标记色块

`data-color-swatch` 写在 `<code>` 元素上，不做扫描也能达到同样效果——而且这是给「本身不是
颜色值」的文字上色的唯一办法：

| 你写 | 你得到 |
| --- | --- |
| `<code data-color-swatch>#E69F00</code>` | 一个色块（文字就是颜色值） |
| `<code data-color-swatch="#E69F00">treat</code>` | 橙色底上的 `treat` 一词 |
| `<code data-color-swatch="rgb(255, 99, 71)">treat</code>` | `colord` 能解析的任意写法——十六进制、`rgb()`、`hsl()` |
| `<code data-color-swatch="nonsense">x</code>` | 完全按原样保留 |

被标记的元素保留自己的 `className` 与 `style`，色块是加在它们之上的。

## 为什么不用 `{.color-swatch}`

Pandoc 风格的属性（`` `#E69F00`\{.color-swatch\} ``、`` `x`\{color="#E69F00"\} ``）**过不了
MDX**，而 Docusaurus 正是用 MDX 编译页面——MDX 会在任何 remark 插件运行**之前**就把 `{...}`
当作 JavaScript 表达式切分：

| 你写 | 会发生什么 |
| --- | --- |
| `` `#E69F00`\{.color-swatch\} `` | 构建失败：`Could not parse expression with acorn` |
| `` `x`\{color="#E69F00"\} `` | 构建通过，浏览器里抛 `ReferenceError: color is not defined` |
| `<code data-color-swatch="#E69F00">x</code>` | 可用——元素上的属性才是 MDX 原生的写法 |

（上面的大括号同样做了转义，原因相同：这一页也是 MDX。）

## 备注

- 围栏代码块永远不动：示例里的颜色是源码，不是样例。
- 半透明颜色会先变成不透明，否则标签自身的背景会透出来，也就无从推断它与文字的对比度。
- 解析不了的颜色保持普通行内代码——扫描永远不会让构建失败。
- 标记属性会被消费掉：它不会出现在渲染后的页面上。
