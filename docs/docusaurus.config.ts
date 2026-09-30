import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import {remarkVersionPlaceholder} from 'duckfn-docs-kit/remark';
import {remarkRunnableSql} from 'duckfn-docs-kit/sql/remark';
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

// The Algolia index is crawled from the GitHub Pages deployment, so every record's URL carries the
// Pages sub-path: https://shijianjs.github.io/duckfn/zh-Hans/docs/intro/. A deployment served from
// a domain root (`npm start`, or a mirror on its own domain) has to drop it again — see the
// `replaceSearchResultPathname` comment below.
const algoliaIndexBaseUrl = '/duckfn/';

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

  // ```mermaid fences render as diagrams through the theme registered below —
  // used for the flow diagrams in the architecture and getting-started pages.
  markdown: {
    mermaid: true,
  },

  // GitHub Pages serves `<path>/index.html` at `<path>/`, and 301-redirects `<path>` to `<path>/`.
  // Keeping the slash in Docusaurus' own output means the sitemap, the canonical tags and every
  // internal link advertise the URL that answers 200 instead of a redirect hop — which is also what
  // crawlers (Algolia DocSearch) index. It only changes how URLs are written; the files on disk and
  // the client-side router behave the same, and slash-less links keep working through the redirect.
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

  // The classic preset already registers the search UI (see the note below), but
  // Mermaid is not part of it: this theme is what turns ```mermaid fences into
  // diagrams, and it follows the light/dark color mode on its own.
  themes: ['@docusaurus/theme-mermaid'],

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
          // `<dfk-sql>` runnable examples (also from the kit).
          remarkPlugins: [
            [remarkVersionPlaceholder, {version: DUCKFN_VERSION}],
            remarkRunnableSql,
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

  // No `themes` entry for the search UI: the classic preset already registers
  // `docusaurus-theme-search-algolia`, and it turns itself on as soon as `themeConfig.algolia`
  // below is filled in. Listing it again fails the build with
  // `Plugin "docusaurus-theme-search-algolia" is used 2 times with ID "default"`.
  themeConfig: {
    // Readers can collapse the docs sidebar away; the toggle button appears next to it.
    docs: {
      sidebar: {
        hideable: true,
      },
    },
    // Mermaid (`@docusaurus/theme-mermaid`, registered in `themes`): the `neo` look with the
    // redux palette — `redux-color` in light mode, `redux-dark-color` in dark. The theme is per
    // color mode (the theme component reads `theme[colorMode]` and re-initialises on a switch),
    // while `look` has no light/dark counterpart and goes through `options`, which the theme
    // spreads into `mermaid.initialize`.
    mermaid: {
      theme: {
        light: 'redux-color',
        dark: 'redux-dark-color',
      },
      options: {
        look: 'neo',
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
    algolia: {
      // import docsearch from '@docsearch/js';
      // import '@docsearch/css';
      //
      // docsearch({
      //   container: '#docsearch',
      //   appId: 'J72GU161MT',
      //   indexName: 'duckfn-doc',
      //   apiKey: 'ed529cc7365e034dee6c1359a3ecddda'
      // });
      // The application ID provided by Algolia
      appId: 'J72GU161MT',

      // Public API key: it is safe to commit it
      apiKey: 'ed529cc7365e034dee6c1359a3ecddda',

      indexName: 'duckfn-doc',

      // Optional: see doc section below
      // contextualSearch: true,

      // Optional: Specify domains where the navigation should occur through window.location instead on history.push. Useful when our Algolia config crawls multiple documentation sites and we want to navigate with window.location.href to them.
      // externalUrlRegex: 'external\\.com|domain\\.com',

      // Replace parts of the item URLs from Algolia: the index is crawled from GitHub Pages, so
      // every hit carries `algoliaIndexBaseUrl` (e.g. /duckfn/zh-Hans/docs/intro/), while this
      // deployment may be served from a domain root (`npm start`).
      // Docusaurus strips it here and re-adds *this* build's baseUrl right afterwards, so the same
      // index serves both: GitHub Pages gets /duckfn/zh-Hans/docs/intro/ back, a root-served
      // deployment gets /zh-Hans/docs/intro/. Without it, a root-served deployment links to
      // /zh-Hans/duckfn/zh-Hans/docs/intro/.
      replaceSearchResultPathname: {
        // `from` is the source of a regular expression (Docusaurus builds it with `new RegExp`),
        // anchored so only the leading Pages sub-path is dropped.
        from: `^${algoliaIndexBaseUrl}`,
        to: '/',
      },

      // Optional: Algolia search parameters
      // searchParameters: {},

      // Optional: path for search page that enabled by default (`false` to disable it)
      // searchPagePath: 'search',

      // Optional: whether the insights feature is enabled or not on Docsearch (`false` by default)
      // insights: false,

      // Optional: whether you want to use the new Ask AI feature (undefined by default)
      // askAi: 'YOUR_ALGOLIA_ASK_AI_ASSISTANT_ID',

      //... other Algolia params
    },

  } satisfies Preset.ThemeConfig,
};

export default config;
