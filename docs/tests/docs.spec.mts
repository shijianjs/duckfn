import {fileURLToPath} from 'node:url';

import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';

/**
 * Every runnable SQL block in this site, registered as a Playwright test:
 * one page per content file (blocks on a page share that page's DuckDB
 * connection), one test per block. Blocks that demonstrate a failure declare
 * `{"type":"duckfn","expect":"error"}` and are checked two-way by `test.fail()`.
 *
 * The site root is derived from this file's own location (`..` from
 * `docs/tests/`), not from the process working directory: an IDE may start the
 * Playwright worker from the repository root, where `static/duckdb-extensions/`
 * cannot be found. `DFK_*` environment variables still override everything.
 */
declareDocsTests({siteDir: fileURLToPath(new URL('..', import.meta.url))});