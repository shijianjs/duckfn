import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import remarkVersionPlaceholder from './plugins/remark-version-placeholder';

// This runs in Node.js - Don't use client-side code here (browser APIs, JSX...)

// GitHub Pages serves a project site from a sub-path (e.g. https://shijianjs.github.io/duckfn),
// so `url` / `baseUrl` are injected by the workflow (see ../.github/workflows/DeployDocs.yml).
// The values below are the local-development fallbacks.
const url = process.env.DOCS_URL ?? 'http://localhost:3000';
const baseUrl = process.env.DOCS_BASE_URL ?? '/';

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
  // internal link advertise the URL that answers 200 instead of a redirect hop — which is also what
  // crawlers (Algolia DocSearch) index. It only changes how URLs are written; the files on disk and
  // the client-side router behave the same, and slash-less links keep working through the redirect.
  trailingSlash: true,

  // Small client-side enhancements the theme has no option for; each file under
  // src/clientModules/ documents what it does. Paths resolve from this directory.
  clientModules: ['./src/clientModules/tocToggle.ts'],

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

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          // Replaces the `{{DUCKFN_VERSION}}` placeholder with the version from
          // docs/duckfn-version.ts, so a release only has to update that one file.
          remarkPlugins: [remarkVersionPlaceholder],
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
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
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
              to: '/docs/examples/rusty-quack',
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

      // Optional: Replace parts of the item URLs from Algolia. Useful when using the same search index for multiple deployments using a different baseUrl. You can use regexp or string in the `from` param. For example: localhost:3000 vs myCompany.com/docs
      // replaceSearchResultPathname: {
      //   from: '/docs/', // or as RegExp: /\/docs\//
      //   to: '/',
      // },

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
