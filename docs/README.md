# duckfn documentation site

Static site for <https://shijianjs.github.io/duckfn/>, built with
[Docusaurus](https://docusaurus.io/) and deployed by
[`.github/workflows/DeployDocs.yml`](../.github/workflows/DeployDocs.yml).

## Layout

| Path | Description |
| --- | --- |
| `docs/intro.md` | Introduction. The only page with a `slug`, so `/docs/intro` stays stable. |
| `docs/getting-started/` | Creating a project, installation and quick start. |
| `docs/guide/` | The feature guide: attributes, each registration kind, type mapping, custom types, errors. |
| `docs/examples/duckfn.md` | The example extension shipped with the crate, feature by feature. |
| `docs/internals/architecture.md` | How duckfn works internally. |
| `docs/build-and-release.md`, `docs/contributing.md`, `docs/faq.md` | Project-level pages, at the top level of the sidebar. |
| `i18n/zh-Hans/` | Simplified Chinese translations of all of the above, plus the UI strings. |
| `src/pages/index.tsx` | Home page: hero, feature cards, the Rust/SQL showcase, and the "where to go next" cards. Every string is a `<Translate>` and has an entry in `i18n/zh-Hans/code.json` under `homepage.*`. The hero, feature grid and "next steps" grid are `<dfk-*>` custom elements from `duckfn-docs-kit`, fed through callback refs. |
| `src/css/custom.css` | Palette and theme overrides: the seven `--ifm-color-primary*` steps come from the logo blue. The `--duckfn-*` brand tokens themselves are defined once in `duckfn-docs-kit/src/theme/tokens.css` and pulled in by the `@import` at the top of this file. |
| `static/` | Files copied to the site root (images, `favicon.ico`, `.nojekyll`). |
| `sidebars.ts` | Sidebar definition. Categories come from `_category_.json`; order from `sidebar_position`. |
| `docusaurus.config.ts` | Site configuration, including the locale list and the footer links. |

This package is one workspace of the repository root: the shared building blocks
(`duckfn-docs-kit/` — the `<dfk-*>` home-page elements, the TOC toggle, the brand
tokens, the version-placeholder remark plugin) are a sibling npm workspace,
consumed as `duckfn-docs-kit`. The lockfile and `node_modules` live at the root.

## Preloaded DuckDB extensions

The runnable SQL blocks preload the `duckfn` extension through the
`dfkExtensions` plugin (`duckfn-docs-kit/sql/extensions`), configured in
`docusaurus.config.ts`: on `npm start` / `npm run build` it fetches the latest
release's `duckfn-wasm_eh.duckdb_extension.wasm` into
`static/duckdb-extensions/duckfn.duckdb_extension.wasm` — the file name must keep
`duckfn` before the first dot, because that base is the entry symbol DuckDB looks
up. It also injects the ordered preload list into every page, and the kit's
runtime loads it while DuckDB initialises — which starts in the background as
soon as a page with a runnable block opens, so the first Run click does not wait
for the download.

The plugins also carry the client wiring: `dfkExtensions` registers the `dfk-*`
elements and `dfkTocToggle` (`duckfn-docs-kit/toc-toggle/plugin`) adds the TOC
collapse control, so the site keeps no `src/clientModules/` files of its own.

Downloads are cached under `.cache/duckfn-docs-kit/` and only re-fetched when the
release asset's sha256 changes; with a warm cache the build works offline. Both
`.cache/` and `static/duckdb-extensions/` are gitignored — a file placed there by
hand needs `git add -f`.

The extension is built by CI for DuckDB v1.5.5, and the site pins
`@duckdb/duckdb-wasm` to the exact dev build whose engine matches
(`1.33.1-dev64.0`, engine v1.5.5 — npm's `next` tag at the time of writing;
stable `1.32.0` bundles v1.4.3 and rejects the extension with a C-API layout
mismatch). When either side moves, re-check the runnable SQL page: both example
blocks must run.

## Commands

```shell
npm install          # once, from the repository root (npm workspaces)
npm start -w docs    # dev server at http://localhost:3000
npm start -w docs -- --locale zh-Hans   # dev server, Chinese
npm run build -w docs        # static site into docs/build/
npm run serve -w docs        # preview the build
npm run typecheck -w docs    # tsc
```

`duckfn-docs-kit` is a source dependency, so rebuild it (`npm run build -w duckfn-docs-kit`)
after editing anything under `duckfn-docs-kit/src/`. The components' styles are inlined into
the JS bundle at build time, so rebuild after touching `home.css`; the global CSS that ships
as source (`tokens.css`, `toc-toggle.css`) needs no build step.

`npm run build` is the check that matters: `onBrokenLinks` is set to `throw`, so a link to a page
that does not exist fails the build for both locales.

## Markdown conventions

**Admonitions.** The opening directive goes on a line of its own and takes an optional title in
square brackets — a bare `:::note Title` does not render. The content always starts on the next line:

```md
:::note[Limitations]

- the first point
:::
```

Nesting works by using more colons for each level: `:::::info[Parent]` → `::::danger[Child]` →
`:::tip[Deep Child]`.

Two more things worth knowing: `onBrokenLinks` is `throw`, so every internal link and anchor has to
resolve (in both locales), and code fences should use one of the languages enabled for Prism in
`docusaurus.config.ts` — `bash`, `rust`, `sql` or `toml`.

## Translations

The site ships in English (`en`, default) and Simplified Chinese (`zh-Hans`). Routes are prefixed per
locale: `/docs/...` and `/zh-Hans/docs/...`.

A translated page is a full copy of its English source, placed under
`i18n/zh-Hans/docusaurus-plugin-content-docs/current/` with the same relative path:

- Translate the body and the reader-facing front matter (`title`, `description`).
- Keep `id`, `slug` and `sidebar_position` identical so both languages share routes and order.
- Link to other pages with **relative file paths** (`./types.md`, `../guide/types.md`). A hard-coded
  `/docs/...` link would send a Chinese page to the English one.

UI strings live in `i18n/zh-Hans/*.json`. After changing text in `docusaurus.config.ts`, in
`src/`, or a `_category_.json`, regenerate the stubs and fill in the new entries:

```shell
npx docusaurus write-translations --locale zh-Hans
```

`write-translations` keeps existing messages, so it only adds what is missing. Two things to check
afterwards: the new entries it appends are in English, and the manually translated `homepage.*`
entries in `code.json` are still present — it warns about `homepage.tagline` because that one cannot
be extracted statically, which is expected.

Add another language by listing it in `i18n.locales` in `docusaurus.config.ts` and repeating the
steps above.

## Deployment

Pushing a version tag (`v*.*.*`) builds the site and publishes it to GitHub Pages; the same workflow
can be started by hand from the Actions tab.

`url` and `baseUrl` are not hard-coded — the workflow reads them from `actions/configure-pages` and
passes them to the build as `DOCS_URL` and `DOCS_BASE_URL`, which `docusaurus.config.ts` picks up.
Outside CI they fall back to `http://localhost:3000` and `/`.

One-time setup: in the repository settings, set **Pages → Build and deployment → Source** to
**GitHub Actions**.
