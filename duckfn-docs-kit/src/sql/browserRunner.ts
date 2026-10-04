/**
 * Runs runnable SQL blocks in a **real browser**, driven from Node with
 * Playwright — the offline successor to the old Node-worker runner
 * (`nodeRunner.ts`, now removed).
 *
 * Why a browser instead of the Node worker
 * ----------------------------------------
 * The Node worker target is not the environment a reader gets: it cannot read
 * remote `http(s)` data (every remote-data example failed with `IO Error: No
 * files found`), and loading the extension forced a loopback server on a
 * Windows-hostile port (the staging path embeds the port, colons are illegal).
 * A browser is the actual product surface: DuckDB-Wasm reads remote files
 * there, and the extension loads same-origin over plain http with none of the
 * staging/port constraints. That is exactly what the docs site ships.
 *
 * Why Playwright, and why `playwright-core`
 * -----------------------------------------
 * Playwright is the mature, maintained way to drive a browser: it owns the
 * protocol, the connect/navigate/evaluate plumbing, auto-waiting, timeouts and
 * crash handling that a hand-rolled driver would have to reinvent. The kit uses
 * the `playwright-core` package (not the full `playwright`) precisely because
 * `playwright-core` never downloads a browser — it launches one you point it
 * at. The runner starts your **system Chrome/Edge** via `executablePath`, so
 * verification stays fully offline. `--browser <path>` / `DFK_BROWSER` override
 * which executable is used; otherwise a small set of well-known paths is probed.
 *
 * How a page maps to a DuckDB instance
 * -------------------------------------
 * `newPage()` navigates to the harness page (`harness.ts`), which builds a
 * fresh `AsyncDuckDB` from the same-origin engine and preloads the extension.
 * Reloading per content file is the isolation boundary (pages never share an
 * instance), while the blocks of one page share its connection — the same "one
 * instance per page, blocks share a connection" model as the site. Files
 * (`COPY … TO`, `dfn_file_write_*`) land in the instance's in-memory file
 * system, readable within the page and gone on reload — the faithful browser
 * behaviour, not a host-directory emulation.
 *
 * The loopback static server, the routes it serves and the browser detection
 * live in `sql/harnessServer` so this class and the Playwright Test integration
 * (`sql/playwright`) share one fixture; this file owns only the one-page-at-a-
 * time lifecycle.
 */
import {chromium, type Browser, type Page} from 'playwright-core';

import {
  findBrowser,
  harnessRoutes,
  resolveEngineBundle,
  startHarness,
  type Harness,
  type HarnessRoutes,
  type WasmPlatform,
} from './harnessServer';

export type {WasmPlatform} from './harnessServer';

/** The extension to `LOAD`: a local `.duckdb_extension.wasm` path or an http(s) URL. */
export interface RunnerOptions {
  extension: string;
  /** DuckDB-Wasm platform bundle; must match how the extension was built. */
  platform?: WasmPlatform;
  /** Engine wasm override; defaults to the platform bundle inside duckdb-wasm. */
  engine?: string;
  /** Browser executable; defaults to a detected Chrome/Edge (`DFK_BROWSER` wins). */
  browser?: string;
}

export interface RunResult {
  rows: number;
  columns: number;
}

/** The harness's `__dfkRun` return shape (see `harness.ts`). */
interface HarnessResult {
  rows: number;
  columns: number;
  error?: string;
}

/** The harness's `__dfkQuery` return shape (see `harness.ts`). */
export interface QueryResult {
  columns: string[];
  rows: Record<string, unknown>[];
  error?: string;
}

/**
 * One DuckDB-Wasm instance at a time, driven through a headless browser.
 * Mirrors the surface `verify.ts` uses from `WasmSqlRunner`.
 */
export class BrowserSqlRunner {
  #harness: Harness | null = null;
  #browser: Browser | null = null;
  #page: Page | null = null;
  /** Set when the current page failed to initialise; every run then reports it. */
  #pageError: string | null = null;

  readonly #routes: HarnessRoutes;
  readonly #browserExecutable: string;

  private constructor(options: {routes: HarnessRoutes; browserExecutable: string}) {
    this.#routes = options.routes;
    this.#browserExecutable = options.browserExecutable;
  }

  static async create(options: RunnerOptions): Promise<BrowserSqlRunner> {
    const {enginePath, workerPath} = resolveEngineBundle(options.platform ?? 'eh', options.engine);
    return new BrowserSqlRunner({
      routes: harnessRoutes({
        extension: options.extension,
        enginePath,
        workerPath,
        // The runner is a trusted local loopback: an unsigned dev extension must
        // load, exactly as the site's own config opts in.
        allowUnsigned: true,
      }),
      browserExecutable: findBrowser(options.browser),
    });
  }

  /** Starts the static server and the browser; idempotent. */
  async #ensureStarted(): Promise<void> {
    if (this.#page) {
      return;
    }
    this.#harness = await startHarness(this.#routes);

    // `playwright-core` launches the executable we hand it and manages the
    // browser process and a throwaway profile for us.
    this.#browser = await chromium.launch({
      executablePath: this.#browserExecutable,
      headless: true,
    });
    this.#page = await this.#browser.newPage();
  }

  /** Drops the current page state and opens a fresh one: new instance, new connection. */
  async newPage(): Promise<void> {
    await this.#ensureStarted();
    const page = this.#page;
    if (!page) {
      throw new Error('sql/browserRunner: browser is not running');
    }
    this.#pageError = null;
    await page.goto(this.#harnessUrl(), {waitUntil: 'load'});
    // `__dfkReady` resolves once the engine is up and the preloads have loaded;
    // Playwright awaits the promise and throws if it rejects.
    try {
      await page.evaluate(() => (window as unknown as {__dfkReady: Promise<void>}).__dfkReady);
    } catch (error) {
      this.#pageError = messageOf(error);
    }
  }

  /** Runs one block; a SQL failure is thrown so the caller can weigh it. */
  async run(sql: string): Promise<RunResult> {
    if (this.#pageError) {
      throw new Error(this.#pageError);
    }
    const page = this.#page;
    if (!page) {
      throw new Error('sql/browserRunner: no page is open — call newPage() first');
    }
    // `sql` crosses as an evaluate argument, never spliced into page code.
    const result = await page.evaluate(
      (statement) => (window as unknown as {__dfkRun: (s: string) => Promise<HarnessResult>}).__dfkRun(statement),
      sql,
    );
    if (result?.error) {
      throw new Error(result.error);
    }
    return {rows: result.rows, columns: result.columns};
  }

  /**
   * Runs one statement and returns its actual rows (not just a count). Used by
   * the vfs browser probe to read each value; `run()` is the verifier's path.
   * Unlike `run()`, a failure is returned as `{error}` rather than thrown.
   */
  async query(sql: string): Promise<QueryResult> {
    if (this.#pageError) {
      return {columns: [], rows: [], error: this.#pageError};
    }
    const page = this.#page;
    if (!page) {
      throw new Error('sql/browserRunner: no page is open — call newPage() first');
    }
    const result = await page.evaluate(
      (statement) =>
        (window as unknown as {__dfkQuery: (s: string) => Promise<QueryResult>}).__dfkQuery(statement),
      sql,
    );
    return result ?? {columns: [], rows: [], error: 'no result'};
  }

  async close(): Promise<void> {
    try {
      await this.#page?.close();
    } catch {
      // The page may already be gone when the browser is shutting down.
    }
    this.#page = null;
    try {
      await this.#browser?.close();
    } catch {
      // Likewise the browser: closing twice or after a crash is not an error.
    }
    this.#browser = null;
    if (this.#harness) {
      const server = this.#harness.server;
      await new Promise<void>((resolve) => server.close(() => resolve()));
      this.#harness = null;
    }
  }

  #harnessUrl(): string {
    if (!this.#harness) {
      throw new Error('sql/browserRunner: harness server is not running');
    }
    return this.#harness.url;
  }
}

function messageOf(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}