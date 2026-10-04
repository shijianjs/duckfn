import {defineDuckfnDocsConfig} from 'duckfn-docs-kit/sql/playwright';

/**
 * Playwright config for the docs site's own SQL tests.
 *
 * The kit's preset points Playwright at `tests/`, uses the system Chrome/Edge
 * (so no browser download is needed), wires the reporter, and leaves every
 * `DFK_*` override in place (`DFK_BROWSER`, `DFK_PLATFORM`, `DFK_EXTENSION`, …).
 *
 * Run it with `npm test` from `docs/`, or `npx playwright test` directly; the
 * HTML report lands in `playwright-report/` when it is not opened on failures.
 */
export default defineDuckfnDocsConfig();