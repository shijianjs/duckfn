/**
 * `duckfn-docs-kit/sql/playwright` — the Playwright Test integration for a docs
 * site's runnable SQL blocks.
 *
 * It is the framework-native successor to the hand-rolled `duckfn-sql-verify`
 * command: the same collection (`sql/collect`), the same browser fixture
 * (`sql/harnessServer` + `sql/harness`) and the same "one instance per page,
 * blocks share its connection" model, but dressed as Playwright Test so a
 * block failure arrives with the framework's report (list / html / junit /
 * github), its trace viewer, its VS Code test tree and its `test.fail()`
 * two-way semantics.
 *
 * A site wires it up in two files:
 *
 * ```ts
 * // playwright.config.ts
 * import {defineDuckfnDocsConfig} from 'duckfn-docs-kit/sql/playwright';
 * export default defineDuckfnDocsConfig();
 *
 * // tests/docs.spec.ts
 * import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';
 * declareDocsTests();
 * ```
 *
 * Configuration (site root, content directories, extension, platform, browser,
 * timeout) comes from `sql/site` — explicit options first, then `DFK_*`
 * environment variables, then the detected site layout.
 *
 * Why `test.fail()` matters here
 * ------------------------------
 * A block may fail *on purpose* (`{"type":"duckfn","expect":"error"}`). The
 * block is registered with `test.fail(expectsError(config))`, so:
 *
 * - declared `error` and it fails → the thrown error is an **expected** failure,
 *   reported as a pass;
 * - declared `error` but it starts succeeding → Playwright reports
 *   *"Expected to fail, but passed"* — the check stays two-way;
 * - declared `ok` and it fails → an ordinary test failure.
 *
 * The expectation is the block's own metadata, never a string match on a
 * comment.
 */
import {
  defineConfig,
  test as base,
  type Page,
  type PlaywrightTestConfig,
} from '@playwright/test';

import {collectRunnableSql, expectsError, type RunnableSqlBlock} from './collect';
import {
  findBrowserPath,
  harnessRoutes,
  resolveEngineBundle,
  startHarness,
  type Harness,
} from './harnessServer';
import {
  resolveDocsSiteConfig,
  resolveExtension,
  type DocsSiteOverrides,
} from './site';

/**
 * Options for {@link defineDuckfnDocsConfig}: the shared docs-site options plus
 * the pieces that only the Playwright config cares about.
 */
export interface DuckfnDocsConfigOptions extends DocsSiteOverrides {
  /** Where the spec files live, relative to the config file. Defaults to `tests`. */
  testDir?: string;
  /** Merged last, so a site can override anything the preset sets. */
  config?: PlaywrightTestConfig;
}

/**
 * The kit's Playwright config preset: detects the system Chrome/Edge (so no
 * browser download is needed at run time), points Playwright at the docs tests
 * and gives the engine-loading hooks headroom.
 */
export function defineDuckfnDocsConfig(
  options: DuckfnDocsConfigOptions = {},
): PlaywrightTestConfig {
  const config = resolveDocsSiteConfig(options);
  const browser = findBrowserPath(config.browser);
  const use: NonNullable<PlaywrightTestConfig['use']> = {};
  if (browser) {
    use.launchOptions = {executablePath: browser};
  }
  return defineConfig({
    testDir: options.testDir ?? 'tests',
    fullyParallel: true,
    forbidOnly: Boolean(process.env.CI),
    retries: process.env.CI ? 1 : 0,
    reporter: process.env.CI
      ? [['list'], ['github']]
      : [['list'], ['html', {open: 'never'}]],
    // A cold start loads duckdb-*.wasm and the extension before the first
    // block; the per-block budget (`timeoutMs`, default 30s) is smaller than
    // what the init hook may need.
    timeout: Math.max(config.timeoutMs, 60_000),
    use,
    ...options.config,
  });
}

/** Worker-scoped fixture: the loopback harness server every page navigates to. */
interface DocsWorkerFixtures {
  dfkHarness: Harness;
}

/**
 * The extended `test` a docs spec registers against. It adds one worker-scoped
 * fixture — the harness server — and keeps every Playwright built-in (`page`,
 * `browser`, traces, screenshots).
 */
export const test = base.extend<{}, DocsWorkerFixtures>({
  dfkHarness: [
    async ({}, use) => {
      const config = resolveDocsSiteConfig();
      const extension = resolveExtension(config.siteDir, config.extension);
      const {enginePath, workerPath} = resolveEngineBundle(config.platform, config.engine);
      const harness = await startHarness(
        harnessRoutes({extension, enginePath, workerPath, allowUnsigned: true}),
      );
      try {
        await use(harness);
      } finally {
        await new Promise<void>((resolve) => harness.server.close(() => resolve()));
      }
    },
    {scope: 'worker'},
  ],
});

export interface DocsTestOptions {
  siteDir?: string;
  contentDirs?: readonly string[];
}

/**
 * Registers one Playwright test per runnable block, grouped by content file.
 *
 * Blocks of the same file share one page (and therefore one DuckDB instance and
 * connection), so a block may rely on a table or macro an earlier block on the
 * **same page** created; pages are isolated from each other. A file's tests run
 * serially (`mode: 'serial'`) to preserve that order, while different files run
 * in parallel workers. A page that cannot initialise fails its `beforeAll`
 * hook, instead of turning every block on the page into a mystery failure.
 */
export function declareDocsTests(options: DocsTestOptions = {}): void {
  const config = resolveDocsSiteConfig(options);
  const blocks = collectRunnableSql({
    siteDir: config.siteDir,
    contentDirs: config.contentDirs,
  });

  for (const [file, fileBlocks] of groupByFile(blocks)) {
    test.describe(file, () => {
      test.describe.configure({mode: 'serial'});
      let page: Page | undefined;

      test.beforeAll(async ({browser, dfkHarness}) => {
        page = await browser.newPage();
        try {
          await page.goto(dfkHarness.url, {waitUntil: 'load'});
          // Resolves once the engine is up and the site preloads have loaded.
          await page.evaluate(
            () => (window as unknown as {__dfkReady: Promise<void>}).__dfkReady,
          );
        } catch (error) {
          throw new Error(`page failed to initialise: ${firstLine(error)}`);
        }
      });

      test.afterAll(async () => {
        await page?.close();
        page = undefined;
      });

      for (const block of fileBlocks) {
        test(`line ${block.line}`, async () => {
          const current = page;
          if (!current) {
            throw new Error('the page for this file is not open');
          }
          // Declared failures are expected failures; a declared failure that
          // starts succeeding becomes "Expected to fail, but passed".
          test.fail(expectsError(block.config), 'block declares "expect":"error"');

          const result = await current.evaluate(
            (sql) =>
              (
                window as unknown as {
                  __dfkRun: (statement: string) => Promise<{
                    rows: number;
                    columns: number;
                    error?: string;
                  }>;
                }
              ).__dfkRun(sql),
            block.sql,
          );
          if (result?.error) {
            // Throwing is the failure signal: for an `expect: error` block
            // `test.fail()` turns it into a pass, for any other block it fails
            // the test with the SQL error as the message.
            throw new Error(firstLine(result.error));
          }
        });
      }
    });
  }
}

function groupByFile(blocks: readonly RunnableSqlBlock[]): Map<string, RunnableSqlBlock[]> {
  const byFile = new Map<string, RunnableSqlBlock[]>();
  for (const block of blocks) {
    const list = byFile.get(block.file);
    if (list) {
      list.push(block);
    } else {
      byFile.set(block.file, [block]);
    }
  }
  return byFile;
}

function firstLine(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.split('\n')[0] ?? '';
}