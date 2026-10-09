import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vite';

const src = (p: string) => fileURLToPath(new URL(`./src/${p}`, import.meta.url));

/**
 * Library build: one ESM entry per public runtime module.
 *
 * Entry keys double as output paths, and package.json never lists them: the
 * wildcard `exports` map (`./*` → `./dist/*.js`) resolves every entry,
 * including the ones that live in a feature folder — `toc-toggle/TocToggle`
 * emits `dist/toc-toggle/TocToggle.js` beside the `tsc` declaration at
 * `dist/toc-toggle/TocToggle.d.ts`, and consumers import it as
 * `duckfn-docs-kit/toc-toggle/TocToggle`. Adding or moving a public module
 * means one line in this file and nothing in package.json. Type declarations
 * come from `tsc -p tsconfig.build.json` instead of a Vite plugin, which keeps
 * the toolchain to one extra dependency.
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
        'toc-toggle/client': src('toc-toggle/client.ts'),
        'toc-toggle/plugin': src('toc-toggle/plugin.ts'),
        remark: src('remark.ts'),
        'color-swatch/rehype': src('color-swatch/rehype.ts'),
        'sql/remark': src('sql/remark.ts'),
        // The ANSI parser behind the `terminal` renderer. An entry purely so the
        // Node test suite can pin its behaviour (`test/ansi.test.mjs`): it is the
        // one piece of a result renderer that is pure logic and has no DOM access
        // to lean on, and the sanitising it does (escape sequences and control
        // characters must never reach the DOM) is exactly what a test should hold.
        'sql/ansi': src('sql/ansi.ts'),
        'mermaid/remark': src('mermaid/remark.ts'),
        'sql/client': src('sql/client.ts'),
        'sql/extensions': src('sql/extensions.ts'),
        // The browser-side test of a docs site's own examples: `collect` finds
        // the runnable blocks, `browserRunner` runs them in DuckDB-Wasm driven
        // through a headless browser, `verify` is the entry (and the
        // `duckfn-sql-verify` bin). `harness` is the page the runner loads; it
        // is emitted by the separate `vite.harness.config.ts` build, not here.
        'sql/collect': src('sql/collect.ts'),
        'sql/browserRunner': src('sql/browserRunner.ts'),
        // The same verifier, framed as Playwright Test: the config preset and
        // the `declareDocsTests` generator a site calls from its own spec.
        'sql/playwright': src('sql/playwright.ts'),
        'sql/verify': src('sql/verify.ts'),
      },
      formats: ['es'],
    },
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      // Heavy runtime dependencies stay external so the consuming site's
      // bundler (Docusaurus' webpack) resolves them from node_modules and can
      // code-split the dynamic `import()`s (CodeMirror, VTable, DuckDB-Wasm,
      // sql-formatter).
      // Vite lib mode's *default* externalisation is unreliable in this kit
      // (`iconify-icon` used to end up bundled despite it), so list them
      // explicitly.
      // The `node:` builtins are what `sql/extensions.ts` (Node build code)
      // imports; without this they would hit Vite's *browser* fallback and be
      // replaced by an empty module. Rollup's external check runs before
      // Vite's resolver, so the import survives into `dist/` untouched.
      external: [
        /^node:/,
        // `sql/browserRunner.ts` drives a browser through Playwright's Node API;
        // it stays external so the runner requires it from node_modules at run
        // time (and `playwright-core` never bundles into the browser harness).
        'playwright-core',
        // `sql/playwright.ts` builds on the Playwright Test runner; the site
        // provides it, so it is never bundled either.
        '@playwright/test',
        // The browser entry of DuckDB-Wasm, kept external so the harness — not
        // the library build — owns how the engine is loaded.
        /^@duckdb\/duckdb-wasm/,
        '@visactor/vtable',
        // The table's search, a sibling package of `@visactor/vtable` and
        // versioned in lockstep with it. Also loaded through a dynamic
        // `import()` (only a reader who searches pays for it).
        '@visactor/vtable-search',
        // Mermaid and the pan/zoom controller behind `<dfk-mermaid>`: both are
        // reached only through dynamic `import()`s, and staying external is what
        // lets the site's bundler code-split them into their own chunks.
        'mermaid',
        '@panzoom/panzoom',
        // Names the downloaded file after the section a diagram sits in. Tiny, but
        // external like the rest: the kit never bundles a runtime dependency.
        'filenamify',
        'codemirror',
        '@codemirror/lang-sql',
        '@codemirror/view',
        '@codemirror/commands',
        '@codemirror/state',
        'sql-formatter',
        // The ANSI parser behind the `terminal` renderer. Statically imported by
        // `sql/terminal.ts` (it is small, like `filenamify` and `colord` below) and
        // external like the rest: the kit never bundles a runtime dependency.
        'anser',
        // Colour parsing, contrast and normalisation for `color-swatch/remark`:
        // the plugin is Node build code, and the site's own copy is what
        // resolves it (`colord/plugins/a11y` is the same package, so the regex
        // covers the sub-path too).
        /^colord/,
      ],
    },
  },
});
