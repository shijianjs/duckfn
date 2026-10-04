import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';

/**
 * Every runnable SQL block in this site, registered as a Playwright test:
 * one page per content file (blocks on a page share that page's DuckDB
 * connection), one test per block. Blocks that demonstrate a failure declare
 * `{"type":"duckfn","expect":"error"}` and are checked two-way by `test.fail()`.
 *
 * Configuration comes from `sql/site`: `DFK_*` environment variables, or the
 * detected site layout (this file's directory is the site root because
 * `playwright test` runs from `docs/`).
 */
declareDocsTests();