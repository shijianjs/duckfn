import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vite';

const src = (p: string) => fileURLToPath(new URL(`./src/${p}`, import.meta.url));

/**
 * Library build: one ESM entry per runtime subpath in package.json.
 *
 * Entry keys double as output paths. Root-level entries (`index`, `remark`)
 * land at `dist/index.js` etc. and resolve through the wildcard `exports` map
 * (`./*` → `./dist/*.js`); an entry that lives in a feature folder uses a
 * slashed key so its JS lands beside its `.d.ts` — `toc-toggle/TocToggle` emits
 * `dist/toc-toggle/TocToggle.js`, matching the `tsc` declaration at
 * `dist/toc-toggle/TocToggle.d.ts`. That subpath then gets an explicit `exports`
 * entry in package.json (the wildcard would otherwise look for
 * `dist/TocToggle.*`). Type declarations come from
 * `tsc -p tsconfig.build.json` instead of a Vite plugin, which keeps the
 * toolchain to one extra dependency.
 *
 * `index` is the browser barrel (home-page elements + value types); `TocToggle`
 * and `remark` stay separate so a Docusaurus config never pulls browser code
 * into Node and a TOC-only site does not bundle `iconify-icon`.
 *
 * The components' own CSS (`home/home.css`) is *not* exported as a stylesheet:
 * `home/styles.ts` imports it with Vite's `?inline` suffix, so the text is
 * bundled into the JS and each component injects it into its shadow root. The
 * CSS that stays as source is only what a shadow boundary cannot host —
 * `theme/tokens.css` (the `--duckfn-*` variables must be declared on the
 * document's `:root` / `[data-theme]` to inherit into the shadow trees) and
 * `toc-toggle/TocToggle.css` (light-DOM rules that must live inside
 * `@layer docusaurus.theme-classic`). Both sit in their feature folder and
 * resolve through the site's own Docusaurus CSS pipeline via `kit.css`.
 */
export default defineConfig({
  build: {
    lib: {
      entry: {
        index: src('index.ts'),
        'toc-toggle/TocToggle': src('toc-toggle/TocToggle.ts'),
        remark: src('remark.ts'),
      },
      formats: ['es'],
    },
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      // Vite lib mode externalises package dependencies by default;
      // `iconify-icon` (the official web component) is bundled in so a
      // consuming site only needs this package.
      external: [],
    },
  },
});
