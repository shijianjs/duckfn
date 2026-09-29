/**
 * `duckfn-docs-kit/sql/harness` — the browser page the offline SQL verifier
 * (`sql/browserRunner`) drives with Playwright.
 *
 * It is deliberately thin: it reuses the site's own runtime (`sql/runtime`) so
 * a verified block runs through exactly the code path a reader's **Run** click
 * does — same `AsyncDuckDB` wiring, same ordered extension preload, same
 * "multi-statement returns the last result" `execute()`. The only difference is
 * where the engine comes from: instead of the jsDelivr CDN, the runner serves
 * `duckdb-*.wasm` and its worker script same-origin and hands those URLs in
 * through {@link window.DFK_HARNESS_BUNDLE}, which `runtime.init` accepts as a
 * {@link LocalBundle}.
 *
 * The two contracts the page reads are injected by the served `harness.html`:
 *
 * - `window.DFK_HARNESS_BUNDLE` — `{mainModule, mainWorker, pthreadWorker?}`,
 *   the locally-served engine bundle (an absolute same-origin URL each).
 * - the `<script id="dfk-sql-runtime">` JSON tag — the preload list and
 *   `allowUnsignedExtensions`, in the very shape the site plugin injects
 *   (see `sql/runtimeConfig`), so nothing here re-implements extension loading.
 *
 * The runner communicates through `window.__dfkReady`, `window.__dfkRun` and
 * `window.__dfkQuery`, each reached with a Playwright `page.evaluate` (which
 * awaits the returned promise and marshals the value back to Node). Page reload
 * per content file is what gives "one instance per page": a fresh document
 * resets the module-level singleton, so pages never share a DuckDB instance
 * while the blocks of one page keep sharing its connection (via
 * `DuckDBRuntime.execute`).
 */
import {DuckDBRuntime, type LocalBundle} from './runtime';

declare global {
  interface Window {
    /** The locally-served engine bundle, injected by the runner's harness.html. */
    DFK_HARNESS_BUNDLE?: LocalBundle;
    /** Resolves once the instance is up and the site preloads have loaded. */
    __dfkReady?: Promise<void>;
    /** Runs one block and reports rows/columns, or a captured error message. */
    __dfkRun?: (sql: string) => Promise<HarnessRunResult>;
    /**
     * Runs one statement and returns its actual result — the rows, not just a
     * count. Used by the vfs browser probe (`browserRunner.query`) to read the
     * value each statement produced; the docs verifier never needs it.
     */
    __dfkQuery?: (sql: string) => Promise<HarnessQueryResult>;
  }
}

/** What {@link window.__dfkRun} resolves with; mirrors the runner's `RunResult`. */
export interface HarnessRunResult {
  rows: number;
  columns: number;
  /** Set when the block failed; `rows`/`columns` are then 0. */
  error?: string;
}

/** What {@link window.__dfkQuery} resolves with: the marshalled result rows. */
export interface HarnessQueryResult {
  columns: string[];
  rows: Record<string, unknown>[];
  error?: string;
}

async function ready(): Promise<void> {
  const bundle = window.DFK_HARNESS_BUNDLE;
  if (!bundle) {
    throw new Error('sql/harness: window.DFK_HARNESS_BUNDLE is not set');
  }
  const runtime = DuckDBRuntime.getInstance();
  const allowUnsigned = readAllowUnsigned();
  await runtime.init({bundle, allowUnsignedExtensions: allowUnsigned});
}

/**
 * `allowUnsignedExtensions` is fixed at `open()` time, so the harness has to
 * hand it to `init()` directly rather than let the runtime read it lazily from
 * the config tag. Reading the same tag here (idempotently) keeps the injected
 * config the single source of truth — the runtime still reads the preload list
 * from it inside `#create()`.
 */
function readAllowUnsigned(): boolean {
  const text = document
    .getElementById('dfk-sql-runtime')
    ?.textContent?.trim();
  if (!text) {
    return false;
  }
  try {
    const parsed = JSON.parse(text) as {allowUnsignedExtensions?: boolean};
    return parsed.allowUnsignedExtensions === true;
  } catch {
    return false;
  }
}

async function run(sql: string): Promise<HarnessRunResult> {
  const result = await DuckDBRuntime.getInstance().execute(sql);
  if (result.error) {
    return {rows: 0, columns: 0, error: result.error};
  }
  return {rows: result.rows.length, columns: result.columns.length};
}

async function query(sql: string): Promise<HarnessQueryResult> {
  const result = await DuckDBRuntime.getInstance().execute(sql);
  if (result.error) {
    return {columns: [], rows: [], error: result.error};
  }
  // Rebuild each row as a plain, own-enumerable object. `execute()` hands back
  // Apache-Arrow row objects whose values sit behind getters, so Playwright's
  // `page.evaluate` serializer would marshal them to `{}`; reading the getters
  // here, in-page, and copying to a literal survives the hop. BigInt values
  // (DuckDB integers) become strings for the same reason.
  const rows = result.rows.map((row) => {
    const plain: Record<string, unknown> = {};
    for (const column of result.columns) {
      const value = row[column];
      plain[column] =
        typeof value === 'bigint'
          ? value.toString()
          : value === null || value === undefined
            ? null
            : typeof value === 'object'
              ? JSON.stringify(value)
              : value;
    }
    return plain;
  });
  return {columns: result.columns, rows};
}

window.__dfkReady = ready();
window.__dfkRun = run;
window.__dfkQuery = query;
