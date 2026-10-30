---
title: 颜色单元格
sidebar_position: 9
description: 可运行块的 `colors` 选项把表格某一列的值画成颜色胶囊块——每个值一个，列表每项一个。
---

# 颜色单元格

一表颜色就该看起来是颜色。`colors` 指定哪一列（或哪几列）的值是颜色，表格于是把每个值画成
填了该颜色的胶囊块，文字就是值本身。直接给单元格铺背景色会把整行高度都染上，一个格子里也
放不下多种颜色——所以这里是胶囊块，而不是单元格样式。

这个选项属于可运行 SQL 块（见 [可运行 SQL 块](./runnable-sql.md)），除了块本身已经在用的
kit 之外不需要任何额外配置：

```sql {"type":"duckfn","show":"table","colors":"hex"}
SELECT '#ff8000' AS hex, 'orange' AS name
UNION ALL SELECT '#440154', 'viridis, dark end'
UNION ALL SELECT 'tomato', 'a CSS colour name'
UNION ALL SELECT 'not a colour', 'still shown, in grey';
```

只有 `show: "table"` 会画胶囊块——多行或多列结果的默认值。其它渲染器忽略 `colors`。

## 列表：每项一个胶囊块

`LIST` 的值每一项各是一个胶囊块，色板天然就是这个形状。`NULL` 与空字符串会被跳过；
空列表保持空白单元格。

```sql {"type":"duckfn","show":"table","colors":"palette"}
SELECT ['#440154', '#21918c', '#fde725'] AS palette, 'viridis' AS name;
```

## 什么算颜色

值按 **CSS 颜色**读取：十六进制（`#rgb`、`#rrggbb`，以及 4 位和 8 位带 alpha 的形式）、
`rgb()` / `rgba()`、`hsl()` / `hsla()`，以及 CSS 颜色名（`tomato`）。别的都不认——
`oklch()`、`lab()` 不会被解析，先在 SQL 里转成十六进制。

- **认不出的值**会放进中性灰胶囊块里继续显示，坏行不会悄无声息地消失。
- 整个值是 `STRUCT`（例如某种不是字符串的颜色类型）时没有文字可放，该单元格退回普通文字，
  所以要在 SQL 里转成十六进制。
- **半透明的值会合成到行背景上**（浅色或深色主题各自的行背景），文字的黑白按 WCAG 对比度
  选取，浅色块上不会出现白字。
- `colors` 里的列名如果结果里没有，说明是**拼写错误**，而不是空结果：表格会报错并点出这个
  列名。

## 胶囊块是怎么画出来的

胶囊块来自 VTable 的 `customLayout` 与 VisActor 的 `Tag`，不是 `style.bgColor`。只有在结果里
真的出现颜色列时才会加载 `@visactor/vrender-components`，没有颜色列的表格不为这个特性付任何
下载。站点侧没有任何额外配置：选项就属于站点已经在跑的块。
