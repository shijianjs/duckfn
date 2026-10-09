---
title: Colour swatches
sidebar_position: 8
description: Inline code that is a colour value is painted with that colour, by a remark plugin.
---

# Colour swatches

`#E69F00` inside inline code looks exactly like `x_axis.wrap`: Infima draws both as a grey tag. The
`remarkColorSwatch` plugin puts the colour behind the text instead, so a palette table or a theme's colour
list reads as colour:

| Palette | N | Colours |
| --- | --- | --- |
| `wong` | 8 | `#E69F00` `#56B4E9` `#009E73` `#F0E442` `#0072B2` `#D55E00` `#CC79A7` `#000000` |
| `tol_bright` | 7 | `#4477AA` `#EE6677` `#228833` `#CCBB44` `#66CCEE` `#AA3377` `#BBBBBB` |
| `category10` | 10 | `#1f77b4` `#ff7f0e` `#2ca02c` `#d62728` `#9467bd` `#8c564b` `#e377c2` `#7f7f7f` `#bcbd22` `#17becf` |

Those cells are ordinary inline code — nothing in the page marks them. The foreground is chosen between black
and white by **contrast** (WCAG ratios, compared), so `#F0E442` gets dark text and `#0072B2` light, in either
colour mode.

## Configuring

```tsx
import {rehypeColorSwatch} from 'duckfn-docs-kit/color-swatch/rehype';

// presets -> classic -> docs
rehypePlugins: [
  rehypeColorSwatch,
],
```

It runs in the **rehype** phase, not the remark one: a swatch is a `style` *attribute* on finished HTML, and
React — which renders the JSX element a marked `<code>` becomes — rejects a string `style` prop
(`The 'style' prop expects a mapping from style properties to values`).

One option:

- `scan` — paint **every** inline code whose whole content is a colour value (default `true`). Turn it off
  where inline code is routinely colour-shaped and mark the ones you want:

  ```tsx
  rehypePlugins: [[rehypeColorSwatch, {scan: false}]],
  ```

## What it paints

Scanning accepts whatever [`colord`](https://www.npmjs.com/package/colord) parses out of the box —
`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()` / `rgba()`, `hsl()` / `hsla()`:

| You write | Result |
| --- | --- |
| `` `#E69F00` `` | a swatch |
| `` `#0072B2` `` · `` `#0af` `` | a swatch |
| `` `rgb(0, 114, 178)` `` · `` `hsl(210, 100%, 35%)` `` | a swatch |
| `` `rgba(0, 0, 0, 0.4)` `` | a swatch, made opaque first |
| `` `x_axis.wrap` `` · `` `#section` `` · `` `10px` `` | untouched |
| `` `red` `` · `` `white` `` · `` `transparent` `` | untouched — see below |

Two deliberate limits:

- **CSS colour names work nowhere** — `red`, `white` and `transparent` appear in prose as ordinary words, and
  telling the name from the word would need a colour table (`colord/plugins/names`) that the plugin does not
  load. Write hex, `rgb()` or `hsl()`.
- **Nothing is added to the site's CSS.** The two inline styles are the whole effect, so a page is right even
  if the site never imported `kit.css`, and the border, radius and padding stay whatever the site's own
  `code` styling says.

## Marking a swatch by hand

`data-color-swatch` on a `<code>` element does the same thing without the scan — and it is the only way to
paint text that is not itself a colour value:

| You write | You get |
| --- | --- |
| `<code data-color-swatch>#E69F00</code>` | a swatch (the text is the colour) |
| `<code data-color-swatch="#E69F00">treat</code>` | the word `treat` on orange |
| `<code data-color-swatch="rgb(255, 99, 71)">treat</code>` | any syntax `colord` parses — hex, `rgb()`, `hsl()` |
| `<code data-color-swatch="nonsense">x</code>` | left exactly as written |

Marked elements keep their own `className` and `style`; the swatch is appended to them.

## Why not `{.color-swatch}`

Pandoc-style attributes (`` `#E69F00`\{.color-swatch\} ``, `` `x`\{color="#E69F00"\} ``) do **not** survive
MDX, which is what Docusaurus compiles a page with — MDX tokenises `{...}` as a JavaScript expression
**before** any remark plugin runs:

| Written | What happens |
| --- | --- |
| `` `#E69F00`\{.color-swatch\} `` | build fails: `Could not parse expression with acorn` |
| `` `x`\{color="#E69F00"\} `` | builds, then throws `ReferenceError: color is not defined` in the browser |
| `<code data-color-swatch="#E69F00">x</code>` | works — an attribute on an element is the MDX-native spelling |

(The braces are escaped above for the same reason: this page is MDX too.)

## Notes

- A fenced code block is never touched: a colour inside an example is source, not a sample.
- A translucent colour is made opaque, because the tag's own background would otherwise show through it and
  its contrast against the text could not be reasoned about.
- A colour that does not parse is left as plain inline code — scanning never fails a build.
- The marker attribute is consumed: it does not reach the rendered page.
