---
title: Overview
slug: /docs-kit
sidebar_position: 1
description: What duckfn-docs-kit is, what the package includes and how a Docusaurus site wires it up.
---

# Overview

`duckfn-docs-kit` is the shared library behind this documentation site. It
holds the pieces a duckfn-family extension docs site needs but should not
copy — the runnable SQL blocks, the extension preloading, the TOC collapse
control, the home-page web components and the version placeholder — each as a
self-contained entry point that a site wires into its own Docusaurus config.

The package is plain TypeScript over the native DOM (no React and no UI
framework of its own) and is maintained for reuse by other extension docs
sites. It is published on npm as
[`duckfn-docs-kit`](https://www.npmjs.com/package/duckfn-docs-kit); this
repository consumes the same source as a workspace dependency, so everything on
these pages matches the code you are reading.

## What's in the kit

| Feature | Entry point | Page |
| --- | --- | --- |
| Runnable SQL blocks | `duckfn-docs-kit/sql/remark` + the `<dfk-sql>` element | [Runnable SQL blocks](./runnable-sql.md) |
| Extension preloading | `duckfn-docs-kit/sql/extensions` | [Preloaded extensions](./preloaded-extensions.md) |
| TOC collapse control | `duckfn-docs-kit/toc-toggle/plugin` | [TOC toggle](./toc-toggle.md) |
| Home-page components | `duckfn-docs-kit` (the barrel) | [Home components](./home-components.md) |
| Version placeholder | `duckfn-docs-kit/remark` | [Version placeholder](./version-placeholder.md) |

## Wiring it up

Install the package, then add the entries a site needs to
`docusaurus.config.ts`:

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

The global CSS is one import in the site's own stylesheet:

```css
/* src/css/custom.css */
@import 'duckfn-docs-kit/src/kit.css';
```

`kit.css` aggregates the `--duckfn-*` brand tokens and the styles a shadow
boundary cannot host (the TOC toggle). The `dfk-*` components carry their own
styles inside the JS bundle, so they need nothing here.

## What a site does not have to keep

- **No client modules** for the kit: the two plugins inject the browser glue
  (`dfk-*` element registration and the TOC toggle) on every page.
- **No per-block extension boilerplate**: one ordered preload list covers the
  extension the site documents.
- **No duplicated markup**: the landing page's hero, feature grid and
  link cards are web components fed through setters, and every page's TOC
  control comes from the kit's CSS + client glue.

## Requirements

- Docusaurus 3.x and Node ≥ 20.
- Runnable blocks and extension preloading pin `@duckdb/duckdb-wasm` to an
  exact version; the version has to stay ABI-compatible with the extensions the
  site serves. See
  [Preloaded extensions](./preloaded-extensions.md#versions-and-signing).
