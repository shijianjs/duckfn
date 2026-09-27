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
 * - `loadExtension()` is the only way a duckfn community extension gets into
 *   the shared instance. The kit never hard-codes an extension name — the docs
 *   source names what it needs. Note that on WebAssembly `INSTALL` is a no-op
 *   (there is no persistent storage to install *into*): only `LOAD` does the
 *   work, fetching the `.duckdb_extension.wasm` from the extension repository
 *   and verifying its signature.
 * - `allowUnsignedExtensions` is opt-in and per-instance: it is a database
 *   setting fixed by `open()`, so it has to be known before the first
 *   `connect()`. The first caller of `init()` therefore decides it.
 */

export type RuntimeState = 'idle' | 'loading' | 'ready' | 'error';

/** Options for {@link DuckDBRuntime.init}; the first caller decides them. */
export interface RuntimeOptions {
  /** Let `LOAD` accept an extension whose signature does not verify. */
  allowUnsignedExtensions?: boolean;
}

/** Options for {@link DuckDBRuntime.loadExtension}. */
export interface LoadExtensionOptions {
  /** A repository serving the extension, instead of the DuckDB default. */
  repository?: string;
}

/** A bare SQL identifier: `LOAD` cannot be parameterised, so this is the guard. */
const EXTENSION_NAME = /^[a-z][a-z0-9_]*$/i;

/** An `https:` URL with no character that could escape the SQL string literal. */
const REPOSITORY_URL = /^https:\/\/[^\s'";`]+$/i;

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
  #allowUnsigned = false;
  /** Loaded / in-flight extensions, keyed by repository + name. */
  #loads = new Map<string, Promise<void>>();

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
   *
   * `options` are only read by the caller that actually creates the instance:
   * `allowUnsignedExtensions` is fixed at `open()` time, and later callers
   * cannot retune a database that already exists.
   */
  init(options: RuntimeOptions = {}): Promise<void> {
    if (options.allowUnsignedExtensions) {
      this.#allowUnsigned = true;
    }
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
    // `open()` is where database-level settings land, and it has to run before
    // the first `connect()`. It is called unconditionally so the instance's
    // configuration has exactly one source of truth.
    await this.#db.open({allowUnsignedExtensions: this.#allowUnsigned});
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
   * Loads a duckfn community extension into the shared instance.
   *
   * `LOAD` is the whole mechanism on WebAssembly: it fetches the extension's
   * `.duckdb_extension.wasm` from the repository and verifies the signature
   * before loading it, and `INSTALL` exists only as a no-op (there is no
   * persistent storage on this platform).
   *
   * The name and repository are validated rather than escaped — `LOAD` takes an
   * identifier, not a parameter, so anything that could terminate the statement
   * is rejected outright. Successful loads (and in-flight ones) are memoised per
   * repository + name; a **failure** is not, so a Run click can retry.
   */
  async loadExtension(name: string, options: LoadExtensionOptions = {}): Promise<void> {
    if (!EXTENSION_NAME.test(name)) {
      throw new Error(`Not a valid extension name: ${name}`);
    }
    const {repository} = options;
    if (repository !== undefined && !REPOSITORY_URL.test(repository)) {
      throw new Error(`Not a valid extension repository: ${repository}`);
    }
    const key = `${repository ?? ''}\u0000${name}`;
    const memoised = this.#loads.get(key);
    if (memoised) {
      return memoised;
    }
    const loading = this.#load(name, repository).catch((error: unknown) => {
      this.#loads.delete(key);
      throw error;
    });
    this.#loads.set(key, loading);
    return loading;
  }

  async #load(name: string, repository: string | undefined): Promise<void> {
    await this.init();
    const conn = this.#conn;
    if (!conn) {
      throw new Error(this.#message || 'DuckDB unavailable');
    }
    if (repository) {
      await conn.query(`SET custom_extension_repository = '${repository}'`);
    }
    await conn.query(`LOAD ${name}`);
  }
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
