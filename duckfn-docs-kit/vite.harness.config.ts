import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vite';

/**
 * Second build config: the verifier's browser harness.
 *
 * Kept separate from `vite.config.ts` on purpose. The library build lists
 * `@duckdb/duckdb-wasm` as external so the *consuming site's* webpack resolves
 * it — but the harness is a self-contained bundle a headless browser loads
 * directly, with no bundler downstream, so it must pull DuckDB-Wasm in rather
 * than leave a bare specifier. Hence its own config, its own entry, and no
 * duckdb-wasm in `external`.
 *
 * Output is `dist/sql/harness.js`, one file (dynamic imports inlined): the
 * runner serves that single script and drives it with Playwright. `emptyOutDir` stays
 * false — `vite.config.ts` already owns (and wipes) `dist/`; this build only
 * adds its artifact beside it.
 */
export default defineConfig({
  build: {
    lib: {
      entry: fileURLToPath(new URL('./src/sql/harness.ts', import.meta.url)),
      formats: ['es'],
      fileName: () => 'sql/harness.js',
    },
    outDir: 'dist',
    emptyOutDir: false,
    target: 'es2022',
    rollupOptions: {
      output: {
        // One self-contained file: the harness is loaded as a single module
        // script, and the runtime reaches DuckDB-Wasm through a dynamic
        // `import()` that must not become a separate chunk. (Rolldown's
        // replacement for the deprecated `inlineDynamicImports`.)
        codeSplitting: false,
      },
      external: [/^node:/],
    },
  },
});
