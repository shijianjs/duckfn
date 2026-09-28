# duckfn-docs-kit

Shared building blocks for [duckfn](https://github.com/shijianjs/duckfn)-family
DuckDB extension documentation sites: runnable SQL blocks, extension preloading,
a TOC collapse control, the home-page web components and the version-placeholder
remark plugin. Each one is a self-contained entry point that a Docusaurus 3 site
wires into its own config — the alternative is copying the same glue into every
extension's docs site.

Plain TypeScript over the native DOM: no React and no UI framework of its own.
The components are retained-mode classes — they build their DOM once, expose
named domain setters and render into shadow roots, so a site's global CSS and the
kit's never leak into each other.

## Install

```bash
npm install duckfn-docs-kit
```

## Entry points

| Import | Runs in | What it provides |
| --- | --- | --- |
| `duckfn-docs-kit` | browser | Home-page custom elements (`<dfk-hero>`, `<dfk-features>`, `<dfk-next-steps>`, `<dfk-sql>`), `registerDfkElements()` and the value types their setters accept |
| `duckfn-docs-kit/remark` | Node (build) | `remarkVersionPlaceholder`: replaces `{{DUCKFN_VERSION}}` inside `text` / `inlineCode` / `code` nodes |
| `duckfn-docs-kit/sql/remark` | Node (build) | `remarkRunnableSql`: turns fenced `sql run` blocks into `<dfk-sql>` elements |
| `duckfn-docs-kit/sql/extensions` | Node (build) | `dfkExtensions()` Docusaurus plugin: preloads a site's DuckDB extensions before the first block runs |
| `duckfn-docs-kit/toc-toggle/plugin` | Node (build) | `dfkTocToggle()` Docusaurus plugin: adds the TOC collapse control |
| `duckfn-docs-kit/toc-toggle/TocToggle` | browser | The TOC collapse class, for a site that drives it itself |
| `duckfn-docs-kit/src/kit.css` | CSS | Brand tokens plus the styles a shadow boundary cannot host |

The two plugins inject their own browser glue (`dfk-*` element registration and
the TOC toggle) on every page, so a site needs no `clientModules` file of its
own. `duckfn-docs-kit/sql/client` and `duckfn-docs-kit/toc-toggle/client` are
that glue — plugins use them, sites should not import them directly.

## Wiring it up

```ts
// docusaurus.config.ts
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

The global CSS is a single import in the site's own stylesheet:

```css
/* src/css/custom.css */
@import 'duckfn-docs-kit/src/kit.css';
```

`kit.css` aggregates the `--duckfn-*` brand tokens and the styles a shadow
boundary cannot host (the TOC toggle). The `dfk-*` components carry their own
styles inside the JS bundle, so they need nothing here.

## Requirements

- Node ≥ 20 and Docusaurus 3.x.
- `@duckdb/duckdb-wasm` is pinned to an exact version because the WebAssembly
  extensions a site loads have to stay ABI-compatible with the DuckDB build
  inside the wasm bundle. Keep the pin and the extension build in lockstep.

## Documentation

The user guide lives at <https://shijianjs.github.io/duckfn/docs/docs-kit>. The
conventions this package is written to — retained-mode components, shadow DOM,
SSR safety, the SQL rendering contract — are documented in
[`AGENTS.md`](./AGENTS.md), which ships inside the package.

## License

MIT
