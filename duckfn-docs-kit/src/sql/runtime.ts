import type * as DuckdbWasm from '@duckdb/duckdb-wasm';

/**
 * The browser-side DuckDB-Wasm runtime: one instance per docs-site frontend
 * runtime (module-level singleton), *not* persisted across page loads.
 *
 * Design points (phase 1):
 *
 * - `@duckdb/duckdb-wasm` is only ever reached through a dynamic `import()`,
 *   so Docusaurus' Node prerender pass and the initial page load never touch
 *   it. The wasm binary and the worker script come from the official jsDelivr
 *   CDN (`getJsDelivrBundles` + `selectBundle`), which sidesteps any webpack
 *   `asyncWebAssembly` / worker configuration on the consuming site.
 *   `selectBundle` falls back to a non-`SharedArrayBuffer` bundle when the
 *   page is not crossOrigin-isolated (GitHub Pages), so it works everywhere.
 * - `init()` is idempotent and retryable: a failed init leaves `state` at
 *   `'error'` and clears the memoised promise, so a later Run click can try
 *   again.
 * - `execute()` hands the whole string to DuckDB. Multi-statement queries
 *   return the result of the **last** statement, which is exactly the
 *   documented behaviour for runnable blocks.
 * - `loadExtension()` is a reserved seam for the future duckfn.wasm /
 *   duckfn-quantstats / duckfn-kuva phases; the kit itself never hard-codes
 *   an extension name.
 */

export type RuntimeState = 'idle' | 'loading' | 'ready' | 'error';

/** A normalised query result: column names plus row objects keyed by them. */
export interface QueryResult {
  columns: string[];
  rows: Record<string, unknown>[];
  /** Set when the statement failed; `columns`/`rows` are then empty. */
  error?: string;
}

let instance: DuckDBRuntime | null = null;

export class DuckDBRuntime {
  /** The per-frontend-runtime singleton; shared by every `<dfk-sql>` block. */
  static getInstance(): DuckDBRuntime {
    instance ??= new DuckDBRuntime();
    return instance;
  }

  #state: RuntimeState = 'idle';
  #message = '';
  #init: Promise<void> | null = null;
  #db: DuckdbWasm.AsyncDuckDB | null = null;
  #conn: DuckdbWasm.AsyncDuckDBConnection | null = null;

  get state(): RuntimeState {
    return this.#state;
  }

  /** The last init failure's message, for the UI to display. */
  get message(): string {
    return this.#message;
  }

  /**
   * Creates the database in the background (first Run click triggers it; a
   * consuming site may also call it early to warm the instance). Concurrent
   * callers share one promise.
   */
  init(): Promise<void> {
    if (this.#state === 'ready') {
      return Promise.resolve();
    }
    if (this.#init) {
      return this.#init;
    }
    this.#state = 'loading';
    this.#init = this.#create().catch((error: unknown) => {
      this.#state = 'error';
      this.#message = errorMessage(error);
      // Drop the memoised promise so the next click retries from scratch.
      this.#init = null;
      throw error;
    });
    return this.#init;
  }

  async #create(): Promise<void> {
    const duckdb = await import('@duckdb/duckdb-wasm');
    const bundle = await duckdb.selectBundle(duckdb.getJsDelivrBundles());
    if (!bundle.mainWorker) {
      throw new Error('The selected DuckDB-Wasm bundle has no worker script');
    }

    // Wrap the worker script in a same-origin Blob URL: the site itself is
    // not COOP/COEP-isolated, and this keeps the CDN script same-origin-safe
    // regardless of CORS headers.
    const workerUrl = URL.createObjectURL(
      new Blob([`importScripts("${bundle.mainWorker}");`], {
        type: 'text/javascript',
      }),
    );
    const worker = new Worker(workerUrl);
    URL.revokeObjectURL(workerUrl);
    const logger = new duckdb.ConsoleLogger();
    this.#db = new duckdb.AsyncDuckDB(logger, worker);
    await this.#db.instantiate(bundle.mainModule, bundle.pthreadWorker);
    this.#conn = await this.#db.connect();
    this.#state = 'ready';
    this.#message = '';
  }

  /** Runs `sql` and resolves with the (last statement's) result. */
  async execute(sql: string): Promise<QueryResult> {
    await this.init();
    const conn = this.#conn;
    if (!conn) {
      return {columns: [], rows: [], error: this.#message || 'DuckDB unavailable'};
    }
    try {
      const table = await conn.query(sql);
      return {
        columns: table.schema.fields.map((field) => field.name),
        rows: table.toArray() as Record<string, unknown>[],
      };
    } catch (error) {
      return {columns: [], rows: [], error: errorMessage(error)};
    }
  }

  /**
   * Reserved for the extension-loading phase (installing a duckfn community
   * extension into the shared instance). Not implemented in phase 1.
   */
  async loadExtension(_name: string): Promise<void> {
    throw new Error('DuckDBRuntime.loadExtension is not implemented yet');
  }
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
