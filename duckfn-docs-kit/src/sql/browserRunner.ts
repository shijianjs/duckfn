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
 */
import {createServer, type Server} from 'node:http';
import {createRequire} from 'node:module';
import {existsSync, readFileSync} from 'node:fs';
import {basename, dirname, join} from 'node:path';
import {chromium, type Browser, type Page} from 'playwright-core';

const require = createRequire(import.meta.url);

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

export type WasmPlatform = 'eh' | 'mvp';

export interface RunResult {
  rows: number;
  columns: number;
}

/** Per-platform file names inside `@duckdb/duckdb-wasm/dist`. */
const PLATFORM_BUNDLES: Record<WasmPlatform, {wasm: string; worker: string}> = {
  eh: {wasm: 'duckdb-eh.wasm', worker: 'duckdb-browser-eh.worker.js'},
  mvp: {wasm: 'duckdb-mvp.wasm', worker: 'duckdb-browser-mvp.worker.js'},
};

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
 * Mirrors the surface `verify.ts` used from `WasmSqlRunner`.
 */
export class BrowserSqlRunner {
  #server: Server | null = null;
  #browser: Browser | null = null;
  #page: Page | null = null;
  #harnessUrl = '';
  /** Set when the current page failed to initialise; every run then reports it. */
  #pageError: string | null = null;

  readonly #extensionUrl: string;
  readonly #enginePath: string;
  readonly #workerPath: string;
  readonly #browserExecutable: string;
  readonly #allowUnsigned: boolean;

  private constructor(options: {
    extensionUrl: string;
    enginePath: string;
    workerPath: string;
    browserExecutable: string;
    allowUnsigned: boolean;
  }) {
    this.#extensionUrl = options.extensionUrl;
    this.#enginePath = options.enginePath;
    this.#workerPath = options.workerPath;
    this.#browserExecutable = options.browserExecutable;
    this.#allowUnsigned = options.allowUnsigned;
  }

  static async create(options: RunnerOptions): Promise<BrowserSqlRunner> {
    const platform = options.platform ?? 'eh';
    const bundle = PLATFORM_BUNDLES[platform];
    const distDir = dirname(require.resolve('@duckdb/duckdb-wasm/dist/duckdb-browser.mjs'));
    const enginePath = options.engine ? resolveLocal(options.engine) : join(distDir, bundle.wasm);
    const workerPath = join(distDir, bundle.worker);

    const remote = /^https?:\/\//i.test(options.extension);
    const extensionUrl = remote ? options.extension : resolveLocal(options.extension);

    return new BrowserSqlRunner({
      extensionUrl,
      enginePath,
      workerPath,
      browserExecutable: findBrowser(options.browser),
      // The runner is a trusted local loopback: an unsigned dev extension must
      // load, exactly as the site's own config opts in.
      allowUnsigned: true,
    });
  }

  /** Starts the static server and the browser; idempotent. */
  async #ensureStarted(): Promise<void> {
    if (this.#page) {
      return;
    }
    this.#server = await startStaticServer(this.#routes());
    const address = this.#server.address();
    const port = typeof address === 'object' && address ? address.port : 0;
    this.#harnessUrl = `http://127.0.0.1:${port}/harness.html`;

    // `playwright-core` launches the executable we hand it and manages the
    // browser process and a throwaway profile for us.
    this.#browser = await chromium.launch({
      executablePath: this.#browserExecutable,
      headless: true,
    });
    this.#page = await this.#browser.newPage();
  }

  /** Maps the URL space the harness page needs to everything on disk. */
  #routes(): Routes {
    // A served extension keeps a `/ext/<name>.duckdb_extension.wasm` shape so
    // its base name still names the entry symbol (`<name>_init_c_api`).
    const ext = /^https?:\/\//i.test(this.#extensionUrl)
      ? {remoteUrl: this.#extensionUrl}
      : {localFile: this.#extensionUrl, baseName: basename(this.#extensionUrl)};
    return {
      // Resolved through the package's own `exports` wildcard
      // (`./sql/harness` -> `dist/sql/harness.js`), so it finds the built harness
      // whether the runner runs from the workspace symlink or an installed copy.
      harnessScript: require.resolve('duckfn-docs-kit/sql/harness'),
      engine: {file: this.#enginePath, name: basename(this.#enginePath)},
      worker: {file: this.#workerPath, name: basename(this.#workerPath)},
      extension: ext,
      allowUnsigned: this.#allowUnsigned,
    };
  }

  /** Drops the current page state and opens a fresh one: new instance, new connection. */
  async newPage(): Promise<void> {
    await this.#ensureStarted();
    const page = this.#page;
    if (!page) {
      throw new Error('sql/browserRunner: browser is not running');
    }
    this.#pageError = null;
    await page.goto(this.#harnessUrl, {waitUntil: 'load'});
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
    if (this.#server) {
      await new Promise<void>((resolve) => this.#server?.close(() => resolve()));
      this.#server = null;
    }
  }
}

/** A path or `file:` URL to an absolute local path; leaves a remote URL alone. */
function resolveLocal(value: string): string {
  return /^file:\/\//i.test(value) ? new URL(value).pathname.replace(/^\/(\w:)/i, '$1') : value;
}

// ---------------------------------------------------------------------------
// Static server
// ---------------------------------------------------------------------------

interface Routes {
  harnessScript: string;
  engine: {file: string; name: string};
  worker: {file: string; name: string};
  extension: {remoteUrl: string} | {localFile: string; baseName: string};
  allowUnsigned: boolean;
}

/**
 * A loopback static server for the harness: `harness.html` (generated, wiring
 * the engine/extension URLs into the two contracts `harness.ts` reads),
 * `harness.js` (the bundled harness), `/vendor/*` (engine + worker), and
 * `/ext/*` (the served extension file). Nothing else is reachable; this is a
 * short-lived test fixture, not a web server.
 */
async function startStaticServer(routes: Routes): Promise<Server> {
  const harness = readFileSync(routes.harnessScript);
  const engine = readFileSync(routes.engine.file);
  const worker = readFileSync(routes.worker.file);
  const extension =
    'localFile' in routes.extension ? readFileSync(routes.extension.localFile) : null;
  const extBaseName = 'baseName' in routes.extension ? routes.extension.baseName : '';

  const extensionUrl =
    'remoteUrl' in routes.extension
      ? routes.extension.remoteUrl
      : `ext/${routes.extension.baseName}`;

  const html = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>duckfn sql harness</title>
    <script>
      window.DFK_HARNESS_BUNDLE = {
        mainModule: '/vendor/${routes.engine.name}',
        mainWorker: '/vendor/${routes.worker.name}',
      };
    </script>
    <script id="dfk-sql-runtime" type="application/json">
      {"allowUnsignedExtensions":${routes.allowUnsigned},"preload":[{"url":"${extensionUrl}"}]}
    </script>
  </head>
  <body>
    <script type="module" src="/harness.js"></script>
  </body>
</html>`;

  const server = createServer((request, response) => {
    const path = new URL(request.url ?? '/', 'http://localhost').pathname;
    if (path === '/harness.html') {
      send(response, 'text/html; charset=utf-8', Buffer.from(html, 'utf8'));
      return;
    }
    if (path === '/harness.js') {
      send(response, 'text/javascript; charset=utf-8', harness);
      return;
    }
    if (path === `/vendor/${routes.engine.name}`) {
      send(response, 'application/wasm', engine);
      return;
    }
    if (path === `/vendor/${routes.worker.name}`) {
      send(response, 'text/javascript; charset=utf-8', worker);
      return;
    }
    if (extension && path === `/ext/${extBaseName}`) {
      send(response, 'application/octet-stream', extension);
      return;
    }
    response.writeHead(404, {'content-type': 'text/plain'});
    response.end('not found');
  });

  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    // Port 0 lets the OS pick a free one; a browser has no reason for a fixed
    // port (unlike the Node worker's staging path, which embedded it).
    server.listen(0, '127.0.0.1', resolve);
  });
  return server;
}

function send(response: import('node:http').ServerResponse, type: string, body: Buffer): void {
  response.writeHead(200, {'content-type': type, 'content-length': body.length});
  response.end(body);
}

// ---------------------------------------------------------------------------
// Browser detection
// ---------------------------------------------------------------------------

/** A short list of well-known Chrome/Edge locations, overridable by DFK_BROWSER. */
function findBrowser(explicit?: string): string {
  const candidate = explicit ?? process.env.DFK_BROWSER;
  if (candidate) {
    return candidate;
  }
  const paths: string[] = [];
  if (process.platform === 'win32') {
    const pf = process.env['PROGRAMFILES'] ?? 'C:\\Program Files';
    const pf86 = process.env['PROGRAMFILES(X86)'] ?? 'C:\\Program Files (x86)';
    const local = process.env['LOCALAPPDATA'] ?? '';
    paths.push(
      join(pf, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      join(pf86, 'Google', 'Chrome', 'Application', 'chrome.exe'),
      join(pf86, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
      join(pf, 'Microsoft', 'Edge', 'Application', 'msedge.exe'),
      local ? join(local, 'Google', 'Chrome', 'Application', 'chrome.exe') : '',
    );
  } else if (process.platform === 'darwin') {
    paths.push(
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    );
  } else {
    paths.push(
      '/usr/bin/google-chrome',
      '/usr/bin/google-chrome-stable',
      '/usr/bin/microsoft-edge',
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser',
    );
  }
  for (const path of paths) {
    if (path && existsSync(path)) {
      return path;
    }
  }
  throw new Error(
    'sql/browserRunner: no Chrome/Edge found — pass --browser <path> or set DFK_BROWSER',
  );
}

function messageOf(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
