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
| `docs/examples/rusty-quack.md` | The repository's example extension, feature by feature. |
| `docs/internals/architecture.md` | How duckfn works internally. |
| `docs/build-and-release.md`, `docs/contributing.md`, `docs/faq.md` | Project-level pages, at the top level of the sidebar. |
| `i18n/zh-Hans/` | Simplified Chinese translations of all of the above, plus the UI strings. |
| `src/pages/index.tsx` | Home page: hero, feature cards, the Rust/SQL showcase, and the "where to go next" cards. Every string is a `<Translate>` and has an entry in `i18n/zh-Hans/code.json` under `homepage.*`. |
| `src/components/icons.tsx` | The home page's inline SVG glyphs (Lucide and Simple Icons paths, quoted at the top of the file) — an icon package would be the only new runtime dependency on the landing page. |
| `src/css/custom.css` | Brand palette and theme overrides. The `--duckfn-*` tokens here are the single definition of the brand blue and accent yellow, so the home page never hard-codes a colour. |
| `static/` | Files copied to the site root (images, `favicon.ico`, `.nojekyll`). |
| `sidebars.ts` | Sidebar definition. Categories come from `_category_.json`; order from `sidebar_position`. |
| `docusaurus.config.ts` | Site configuration, including the locale list and the footer links. |

## Commands

```shell
npm install          # once
npm start            # dev server at http://localhost:3000
npm start -- --locale zh-Hans   # dev server, Chinese
npm run build        # static site into build/
npm run serve        # preview the build
npm run typecheck    # tsc
```

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
