# duckfn documentation site

Static site for <https://shijianjs.github.io/duckfn/>, built with
[Docusaurus](https://docusaurus.io/) and deployed by
[`.github/workflows/DeployDocs.yml`](../.github/workflows/DeployDocs.yml).

## Layout

| Path | Description |
| --- | --- |
| `docs/` | Documentation pages, in **English** (the source language). Folders become sidebar categories. |
| `i18n/zh-Hans/` | Simplified Chinese translations of the pages above and of the UI strings. |
| `src/pages/index.tsx` | Home page. |
| `src/css/custom.css` | Theme overrides. |
| `static/` | Files copied to the site root (images, `favicon.ico`, `.nojekyll`). |
| `sidebars.ts` | Sidebar definition. |
| `docusaurus.config.ts` | Site configuration, including the locale list. |

## Commands

```shell
npm install          # once
npm start            # dev server at http://localhost:3000
npm start -- --locale zh-Hans   # dev server, Chinese
npm run build        # static site into build/
npm run serve        # preview the build
npm run typecheck    # tsc
```

## Translations

The site ships in English (`en`, default) and Simplified Chinese (`zh-Hans`).
Routes are prefixed per locale: `/docs/...` and `/zh-Hans/docs/...`.

A translated page is a full copy of its English source, placed under
`i18n/zh-Hans/docusaurus-plugin-content-docs/current/` with the same relative
path — translate the body *and* the reader-facing front matter (`title`,
`sidebar_label`, `description`), but keep `id`, `slug` and `sidebar_position`
untouched so both languages share the same routes.

UI strings (navbar, footer, home page) live in `i18n/zh-Hans/*.json`. After
changing text in `docusaurus.config.ts` or `src/`, regenerate the stubs:

```shell
npx docusaurus write-translations --locale zh-Hans
```

Add another language by listing it in `i18n.locales` in `docusaurus.config.ts`
and repeating the steps above.

## Deployment

Pushing a version tag (`v*.*.*`) builds the site and publishes it to GitHub
Pages; the same workflow can be started by hand from the Actions tab.

`url` and `baseUrl` are not hard-coded — the workflow reads them from
`actions/configure-pages` and passes them to the build as `DOCS_URL` and
`DOCS_BASE_URL`, which `docusaurus.config.ts` picks up. Outside CI they fall
back to `http://localhost:3000` and `/`.

One-time setup: in the repository settings, set **Pages → Build and deployment →
Source** to **GitHub Actions**.
