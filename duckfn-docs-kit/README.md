# duckfn-docs-kit

Shared building blocks for [duckfn](https://github.com/shijianjs/duckfn)-family
DuckDB extension documentation sites: runnable SQL blocks, mermaid diagrams,
extension preloading, a TOC collapse control, the home-page web components and
the version-placeholder remark plugin. Each one is a self-contained entry point
that a Docusaurus 3 site wires into its own config — the alternative is copying
the same glue into every extension's docs site.

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
| `duckfn-docs-kit` | browser | Custom elements (`<dfk-hero>`, `<dfk-features>`, `<dfk-next-steps>`, `<dfk-sql>`, `<dfk-mermaid>`), `registerDfkElements()` and the value types their setters accept |
| `duckfn-docs-kit/remark` | Node (build) | `remarkVersionPlaceholder`: replaces `{{DUCKFN_VERSION}}` inside `text` / `inlineCode` / `code` nodes |
| `duckfn-docs-kit/sql/remark` | Node (build) | `remarkRunnableSql`: turns fenced `sql {"type":"duckfn",…}` blocks into `<dfk-sql>` elements |
| `duckfn-docs-kit/mermaid/remark` | Node (build) | `remarkMermaid`: turns ```` ```mermaid ```` fences into `<dfk-mermaid>` diagrams |
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
import {remarkMermaid} from 'duckfn-docs-kit/mermaid/remark';
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
            // ```mermaid fences become diagrams; no @docusaurus/theme-mermaid.
            remarkMermaid,
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

A runnable block that reads a file the site ships writes it as
`'{{DFK_BASE_URL}}data/x.tsv'` (`{{DFK_ORIGIN}}` is the page origin on its own):
DuckDB-Wasm resolves nothing relative to the page, and the runtime resolves both
tokens — the baseUrl per locale — just before the SQL runs. See `sql/placeholders`
and the asset-mount section of the guide.

## Requirements

- Node ≥ 20 and Docusaurus 3.x.
- `@duckdb/duckdb-wasm` is pinned to an exact version because the WebAssembly
  extensions a site loads have to stay ABI-compatible with the DuckDB build
  inside the wasm bundle. Keep the pin and the extension build in lockstep.

## Documentation

The user guide lives at <https://shijianjs.github.io/duckfn/docs/docs-kit>. If you —
or an AI agent — are *using* this package, read [`AGENTS.md`](./AGENTS.md): it ships
inside the package and states the contracts (the block metastring, the config
fields, the traps). The conventions this package is *written to* — retained-mode
components, shadow DOM, SSR safety — live in `CONVENTIONS.md` in the repository and
are not published.

## License

MIT
