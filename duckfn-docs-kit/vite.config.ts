import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vite';

const src = (p: string) => fileURLToPath(new URL(`./src/${p}`, import.meta.url));

/**
 * Library build: three ESM entries, one per export subpath in package.json.
 *
 * Entry keys double as output paths, so `dist/index.js`, `dist/toc-toggle.js`
 * and `dist/remark/version-placeholder.js` line up with the `exports` map.
 * Type declarations come from `tsc -p tsconfig.build.json` instead of a Vite
 * plugin, which keeps the toolchain to one extra dependency.
 *
 * The @iconify-icons packages are *not* externalised: their data is bundled, so
 * a consuming site only needs this package, not the icon sets.
 *
 * CSS is not processed here at all — `src/css/*.css` ships as source and is
 * exported directly, because the styles must stay in the consuming site's own
 * Docusaurus CSS pipeline (that is what lets `@layer docusaurus.theme-classic`
 * and the Infima variables resolve).
 */
export default defineConfig({
  build: {
    lib: {
      entry: {
        index: src('index.ts'),
        'toc-toggle': src('toc-toggle.ts'),
        'remark/version-placeholder': src('remark/version-placeholder.ts'),
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
