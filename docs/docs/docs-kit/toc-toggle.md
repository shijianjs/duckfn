---
title: TOC toggle
sidebar_position: 4
description: A collapse control for the desktop table of contents, injected by a one-line plugin.
---

# TOC toggle

Docusaurus has no built-in control for the right-hand table of contents —
`themeConfig.tableOfContents` only takes heading levels — so the kit adds one:
a collapse/expand button at the top of the desktop TOC. **This page is the
demo**: on a desktop-width window the button above "On this page" is it.

## Configuring

```tsx
import {dfkTocToggle} from 'duckfn-docs-kit/toc-toggle/plugin';

plugins: [dfkTocToggle()],
```

The styles travel with the kit's global CSS:

```css
@import 'duckfn-docs-kit/src/kit.css';
```

The plugin injects the client glue — initialise once, refresh after every route
change — so the site keeps no client module of its own.

## Behaviour

- Collapsing switches the article to full width; the choice is remembered in
  `localStorage` under the key `duckfn:toc-collapsed`.
- Desktop only, following Docusaurus' own TOC breakpoint. When the viewport
  shrinks the button and the layout class are dropped, and it re-checks on the
  way back.
- Labels follow the page language; `en` and `zh-Hans` ship built in.
- Pages without headings (no TOC) simply show nothing.
