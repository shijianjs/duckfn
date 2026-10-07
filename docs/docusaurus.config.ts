import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import {remarkVersionPlaceholder} from 'duckfn-docs-kit/remark';
import {remarkRunnableSql} from 'duckfn-docs-kit/sql/remark';
import {remarkMermaid} from 'duckfn-docs-kit/mermaid/remark';
import {dfkExtensions} from 'duckfn-docs-kit/sql/extensions';
import {dfkTocToggle} from 'duckfn-docs-kit/toc-toggle/plugin';
import {DUCKFN_VERSION} from './duckfn-version';
import type {UrlPreloadEntry} from 'duckfn-docs-kit/sql/runtimeConfig';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

// GitHub Pages serves a project site from a sub-path (e.g. https://shijianjs.github.io/duckfn),
// so `url` / `baseUrl` are injected by the workflow (see ../.github/workflows/DeployDocs.yml).
// The values below are the local-development fallbacks.
const url = process.env.DOCS_URL ?? 'http://localhost:3000';
const baseUrl = process.env.DOCS_BASE_URL ?? '/';

// The duckfn extension the runnable SQL blocks preload. The file name keeps `duckfn` before its
// first dot — that base is the entry symbol DuckDB looks up, which is why the release asset (which
// carries the wasm platform suffix) is renamed on the way in.
//
// *Where* that file comes from depends on `DOCS_EXTENSION_FROM_RELEASE`: the GitHub Pages
// deployment sets it and fetches the asset from the repository's latest release, which is safe
// because that workflow runs only after the extension pipeline has published the release (see
// ../.github/workflows/DeployDocs.yml). Every other build leaves `release` off, so the plugin
// serves the file that is already under `static/duckdb-extensions/` — the one `just build_wasm_eh`
// writes, i.e. the working tree's own build, never the last release. `just test_wasm` builds it and
// then runs the docs' SQL test, so local runs have no reason to touch the release at all.
const duckfnExtension: UrlPreloadEntry =
  process.env.DOCS_EXTENSION_FROM_RELEASE === '1'
    ? {
        url: 'duckdb-extensions/duckfn.duckdb_extension.wasm',
        release: {repository: 'shijianjs/duckfn', asset: 'duckfn-wasm_eh.duckdb_extension.wasm'},
      }
    : {url: 'duckdb-extensions/duckfn.duckdb_extension.wasm'};

const config: Config = {
  title: 'duckfn',
  tagline: 'Write DuckDB extensions in plain Rust',
  favicon: 'img/duckfn-logo.svg',

  // Future flags, see https://docusaurus.io/docs/api/docusaurus-config#future
  future: {
    v4: true, // Improve compatibility with the upcoming Docusaurus v4
  },

  url,
  baseUrl,

  onBrokenLinks: 'throw',

  // GitHub Pages serves `<path>/index.html` at `<path>/`, and 301-redirects `<path>` to `<path>/`.
  // Keeping the slash in Docusaurus' own output means the sitemap, the canonical tags and every
  // internal link advertise the URL that answers 200 instead of a redirect hop — which is also the
  // URL the search index records for a page, so a hit lands directly instead of going through a
  // redirect. It only changes how URLs are written; the files on disk and the client-side router
  // behave the same, and slash-less links keep working through the redirect.
  trailingSlash: true,

  // Client-side enhancements live in the kit's plugins (see the `plugins`
  // below), so this site carries no client modules of its own.

  // English is the source language; every page under docs/ can be translated under
  // docs/i18n/zh-Hans/. Add more locales here when needed.
  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'zh-Hans'],
    localeConfigs: {
      en: {
        label: 'English',
        direction: 'ltr',
        htmlLang: 'en',
      },
      'zh-Hans': {
        label: '简体中文',
        direction: 'ltr',
        htmlLang: 'zh-Hans',
      },
    },
  },

  // The classic preset registers the search UI (see the `themes` note below) and
  // says nothing about mermaid: ```mermaid fences are turned into `<dfk-mermaid>`
  // elements by the kit's `remarkMermaid` (in the docs `remarkPlugins` below),
  // which is this site's whole mermaid integration. No `@docusaurus/theme-mermaid`
  // and no swizzled component — the kit's element renders the diagram itself, and
  // is the only place that knows about the dark-mode first-load fix.

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          // Replaces the `{{DUCKFN_VERSION}}` placeholder with the version from
          // docs/duckfn-version.ts, so a release only has to update that one file.
          // The plugin itself ships in duckfn-docs-kit for reuse by other
          // extension docs sites; the version value stays site-specific.
          // `remarkRunnableSql` turns ```sql {"type":"duckfn",…}``` blocks into
          // `<dfk-sql>` runnable examples, and `remarkMermaid` turns ```mermaid
          // fences into `<dfk-mermaid>` diagrams — both from the kit. The diagram
          // element carries the kit's default palette (the `neo` look, redux
          // colours per colour mode); `remarkMermaid({config: {…}})` overrides it
          // for a site that wants its own.
          remarkPlugins: [
            [remarkVersionPlaceholder, {version: DUCKFN_VERSION}],
            remarkRunnableSql,
            remarkMermaid,
          ],
          // Remove this to remove the "edit this page" links.
          editUrl: 'https://github.com/shijianjs/duckfn/tree/main/docs/',
          // Without this, translated pages link back to the English source in docs/docs/;
          // with it they point at the translated file under docs/i18n/<locale>/.
          editLocalizedFiles: true,
        },
        // No blog for now; switch this to an options object to enable one.
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  // The docs-kit wiring: `dfkExtensions` makes every page's preload list
  // resolvable — an entry that names a release is fetched into `static/` at
  // dev/build startup (cached locally, re-downloaded only when the release
  // asset's sha256 changes), one without a release is left as it is — then
  // injects the ordered list into every page (the kit's `src/sql/runtime.ts`
  // loads them while DuckDB initialises) and registers the `dfk-*` elements;
  // `dfkTocToggle` adds the TOC collapse control. Together they replace the
  // client modules this site used to keep under src/clientModules/.
  plugins: [
    dfkExtensions({
      // CI builds the release assets without DuckDB's signing keys — the same
      // reason local development runs `duckdb -unsigned`.
      allowUnsignedExtensions: true,
      preload: [duckfnExtension],
    }),
    dfkTocToggle(),
  ],

  // Search is built into the site: `@easyops-cn/docusaurus-search-local` indexes the pages at
  // build time (its `postBuild`, once per locale) into a lunr index served from this deployment, so
  // there is no Algolia crawler, no account and no API key, and the index is always the one that
  // matches the site it was built from. Listing it under `themes` — not `plugins` — is what makes
  // its `SearchBar` / `SearchPage` win: site themes are loaded after the classic preset's, whose
  // own `docusaurus-theme-search-algolia` stays inert without a `themeConfig.algolia` block, which
  // is why there is no `algolia` key there either.
  themes: [
    [
      '@easyops-cn/docusaurus-search-local',
      {
        // Every published locale has to be listed here: the index is built per locale, and `zh` is
        // what turns on CJK tokenisation (jieba while indexing, the lunr `zh` pipeline in the
        // browser). Without it the Chinese pages would be indexed as unsegmentable runs of
        // characters.
        language: ['en', 'zh'],
        // `filename` puts the index hash in the file name rather than in a `?_=` query string, so
        // a redeployed site cannot serve a stale index out of the browser or the Pages cache.
        hashed: 'filename',
        // The site has no blog (see `blog: false` above); without this the plugin looks for
        // `docs/blog/` and warns that it does not exist, once per locale.
        indexBlog: false,
      },
    ],
  ],

  themeConfig: {
    // Readers can collapse the docs sidebar away; the toggle button appears next to it.
    docs: {
      sidebar: {
        hideable: true,
      },
    },
    // Replace with your project's social card
    image: 'img/docusaurus-social-card.jpg',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      // Same behaviour as docusaurus.io: the sticky navbar slides away once the
      // reader scrolls down past it, and slides back in on the way up. The theme
      // already ships that animation as hashed CSS-module classes on the <nav>,
      // so this needs no CSS in src/css/custom.css and no client module.
      hideOnScroll: true,
      title: 'duckfn',
      logo: {
        alt: 'duckfn logo',
        src: 'img/duckfn-logo.svg',
      },
      items: [
        // One navbar entry per sidebar (see sidebars.ts): the user-facing docs,
        // the docs-kit tooling, and the project's own development guide.
        {
          type: 'docSidebar',
          sidebarId: 'userGuide',
          position: 'left',
          label: 'User guide',
        },
        {
          type: 'docSidebar',
          sidebarId: 'docsKit',
          position: 'left',
          label: 'Docs kit',
        },
        {
          type: 'docSidebar',
          sidebarId: 'development',
          position: 'left',
          label: 'Development',
        },
        {
          type: 'localeDropdown',
          position: 'right',
        },
        {
          // All the project's external links live behind one dropdown, so the navbar keeps a
          // single slot no matter how many of them there are.
          type: 'dropdown',
          label: 'Links',
          position: 'right',
          items: [
            {label: 'GitHub', href: 'https://github.com/shijianjs/duckfn'},
            {label: 'crates.io', href: 'https://crates.io/crates/duckfn'},
            {label: 'docs.rs', href: 'https://docs.rs/duckfn'},
            {label: 'Zread', href: 'https://zread.ai/shijianjs/duckfn'},
            // Upstream ecosystem: DuckDB itself, the extension directory, and the
            // C-API binding crate duckfn is built on.
            {label: 'DuckDB', href: 'https://duckdb.org'},
            {
              label: 'Community extensions',
              href: 'https://duckdb.org/community_extensions/',
            },
            {label: 'quack-rs', href: 'https://github.com/tomtom215/quack-rs'},
          ],
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Docs',
          items: [
            {
              label: 'Introduction',
              to: '/docs/intro',
            },
            {
              label: 'Quick start',
              to: '/docs/getting-started/quick-start',
            },
            {
              label: 'Guide',
              to: '/docs/guide/attributes',
            },
            {
              label: 'Examples',
              to: '/docs/examples/duckfn',
            },
          ],
        },
        {
          title: 'Links',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/shijianjs/duckfn',
            },
            {
              label: 'crates.io',
              href: 'https://crates.io/crates/duckfn',
            },
            {
              label: 'docs.rs',
              href: 'https://docs.rs/duckfn',
            },
            {
              label: 'Zread',
              href: 'https://zread.ai/shijianjs/duckfn',
            },
          ],
        },
        {
          // Upstream of this project: the DuckDB engine, the directory of community
          // extensions, and the C-API binding crate that duckfn sits on.
          title: 'Ecosystem',
          items: [
            {
              label: 'DuckDB',
              href: 'https://duckdb.org',
            },
            {
              label: 'Community extensions',
              href: 'https://duckdb.org/community_extensions/',
            },
            {
              label: 'quack-rs',
              href: 'https://github.com/tomtom215/quack-rs',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} duckfn contributors. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['bash', 'rust', 'sql', 'toml'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
