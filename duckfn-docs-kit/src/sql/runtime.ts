import {expandSqlPlaceholdersHere} from './placeholders';
import type * as DuckdbWasm from '@duckdb/duckdb-wasm';
import {
  DFK_SQL_RUNTIME_TAG_ID,
  EXTENSION_NAME_PATTERN,
  REPOSITORY_KEYWORDS,
  REPOSITORY_URL_PATTERN,
  extensionBaseName,
  isAbsoluteHttpUrl,
  normalizePreloadEntry,
  parseSiteRuntimeConfig,
  type PreloadEntry,
  type SiteRuntimeConfig,
} from './runtimeConfig';

/**
 * The browser-side DuckDB-Wasm runtime: one instance per docs-site frontend
 * runtime (module-level singleton), *not* persisted across page loads.
 *
 * Design points:
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
 *   documented behaviour for runnable blocks. It is also the one place where
 *   a block's `{{DFK_ORIGIN}}` placeholders are expanded (see `sql/placeholders`),
 *   so the site and the verifier cannot disagree on them.
 * - Extensions get into the shared instance two ways, both through the same
 *   memoised loader: the **site preload list** (the ordered `preload` array of
 *   the JSON `<script>` tag the build-time plugin injects, loaded right after
 *   `connect()` so a block can rely on the extension without naming it), and
 *   the **per-block `extensions` config** loaded on demand by
 *   {@link DuckDBRuntime.loadExtension}. Note that on WebAssembly `INSTALL` is
 *   a no-op (there is no persistent storage to install *into*): it only
 *   records where a later `LOAD` fetches a name from. A load by name fetches
 *   `<repository>/duckdb-wasm/<revision>/<platform>/<name>.duckdb_extension.wasm`
 *   and verifies the signature; a `{url}` preload fetches exactly that URL
 *   (which is why the URL must be absolute — the worker runs from a blob URL
 *   and cannot resolve relative paths). Either way, the text before the first
 *   dot of the file name is the entry symbol DuckDB looks up, so a release
 *   asset like `duckfn-wasm_eh.duckdb_extension.wasm` has to be renamed to
 *   `duckfn.duckdb_extension.wasm` on the way in (enforced by validation).
 * - Extension names, repositories and URLs are **validated, not escaped** (see
 *   `sql/runtimeConfig`): `LOAD` cannot take them as parameters.
 * - `allowUnsignedExtensions` is opt-in and per-instance: it is a database
 *   setting fixed by `open()`, so it has to be known before the first
 *   `connect()`. The site-wide value (from the injected config) and the first
 *   caller's are merged by whichever `init()` actually creates the instance.
 */

export type RuntimeState = 'idle' | 'loading' | 'ready' | 'error';

/**
 * Options for {@link DuckDBRuntime.init}. They are merged with the site-wide
 * injected config, and only read by the caller that actually creates the
 * instance: `allowUnsignedExtensions` is fixed at `open()` time and later
 * callers cannot retune a database that already exists.
 */
export interface RuntimeOptions {
  /** Let `LOAD` accept an extension whose signature does not verify. */
  allowUnsignedExtensions?: boolean;
  /**
   * Serve the DuckDB-Wasm engine from explicit same-origin URLs instead of the
   * jsDelivr CDN. Used by the offline SQL verifier (`sql/browserRunner`): the
   * harness passes the locally-served `duckdb-*.wasm` / worker script so a CI
   * run never reaches the network. When set, `#create()` skips
   * `getJsDelivrBundles()`/`selectBundle()` and the cross-origin blob-worker
   * wrapper (the worker is same-origin here, so it is constructed directly).
   */
  bundle?: LocalBundle;
}

/**
 * A locally-served DuckDB-Wasm bundle: absolute same-origin URLs for the
 * engine wasm and its worker script. `pthreadWorker` is only needed for the
 * cross-origin-isolated (COI) bundle; the default non-COI `eh`/`mvp` bundles run
 * single-threaded and leave it unset.
 */
export interface LocalBundle {
  mainModule: string;
  mainWorker: string;
  pthreadWorker?: string;
}

/** Options for {@link DuckDBRuntime.loadExtension}. */
export interface LoadExtensionOptions {
  /** `community`, `core` or a repository URL, instead of the official default. */
  repository?: string;
}

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
  /** A locally-served engine bundle (offline verifier); `null` means CDN. */
  #bundle: LocalBundle | null = null;
  /** The injected site config; read (and validated) once on first init. */
  #site: SiteRuntimeConfig | null = null;
  /** Loaded / in-flight extensions, keyed by repository + name, or by URL. */
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
   * callers share one promise, and it only resolves once the site's preloads
   * are loaded too.
   *
   * Options and the injected site config are only read by the caller that
   * actually creates the instance: `allowUnsignedExtensions` is fixed at
   * `open()` time, and later callers cannot retune a database that already
   * exists. A malformed injected config fails here, before any download.
   */
  init(options: RuntimeOptions = {}): Promise<void> {
    if (options.allowUnsignedExtensions) {
      this.#allowUnsigned = true;
    }
    if (options.bundle) {
      this.#bundle = options.bundle;
    }
    if (this.#state === 'ready') {
      return Promise.resolve();
    }
    if (this.#init) {
      return this.#init;
    }
    const site = this.#siteConfig();
    if (site.allowUnsignedExtensions) {
      this.#allowUnsigned = true;
    }
    this.#state = 'loading';
    this.#init = this.#create(site.preload).catch((error: unknown) => {
      this.#state = 'error';
      this.#message = errorMessage(error);
      // Drop the memoised promise so the next click retries from scratch.
      this.#init = null;
      throw error;
    });
    return this.#init;
  }

  /** The injected config, validated once; a missing tag means "no preloads". */
  #siteConfig(): SiteRuntimeConfig {
    this.#site ??= readSiteRuntimeConfig();
    return this.#site;
  }

  async #create(preload: readonly PreloadEntry[]): Promise<void> {
    const duckdb = await import('@duckdb/duckdb-wasm');

    // Offline verifier path: the engine and its worker are served same-origin
    // by the runner, so the worker is constructed directly (no cross-origin
    // blob wrapper) and no CDN bundle is selected.
    if (this.#bundle) {
      const worker = new Worker(this.#bundle.mainWorker);
      this.#db = new duckdb.AsyncDuckDB(new duckdb.ConsoleLogger(), worker);
      await this.#db.instantiate(this.#bundle.mainModule, this.#bundle.pthreadWorker ?? null);
      await this.#db.open({allowUnsignedExtensions: this.#allowUnsigned});
      this.#conn = await this.#db.connect();
      for (const entry of preload) {
        await this.#loadEntry(entry);
      }
      this.#state = 'ready';
      this.#message = '';
      return;
    }

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
    // Site preloads run before `ready`: every block may rely on them, and the
    // first Run click pays for all of them at once. Sequential on purpose —
    // the list is ordered (one extension may build on another) and parallel
    // loads would race the shared connection.
    for (const entry of preload) {
      await this.#loadEntry(entry);
    }
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
      const table = await conn.query(expandSqlPlaceholdersHere(sql));
      return {
        columns: table.schema.fields.map((field) => field.name),
        rows: table.toArray() as Record<string, unknown>[],
      };
    } catch (error) {
      return {columns: [], rows: [], error: errorMessage(error)};
    }
  }

  /**
   * Loads one extension on demand — what a runnable block's `extensions`
   * config ends up doing.
   *
   * `LOAD` is the whole mechanism on WebAssembly: it fetches the extension's
   * `.duckdb_extension.wasm` and verifies the signature before loading it.
   * `INSTALL … FROM` only records *where* a later `LOAD` should fetch from
   * (there is no persistent storage to install into), which is also why it is
   * used for a non-default `repository` instead of the global
   * `SET custom_extension_repository` — the recorded source stays attached to
   * this one extension.
   *
   * The name and repository are validated rather than escaped — `LOAD` takes
   * an identifier, not a parameter, so anything that could terminate the
   * statement is rejected outright. Successful loads (and in-flight ones) are
   * memoised per repository + name; a **failure** is not, so a Run click can
   * retry.
   */
  async loadExtension(name: string, options: LoadExtensionOptions = {}): Promise<void> {
    await this.init();
    return this.#loadEntry({name, repository: options.repository});
  }

  /**
   * The shared loader behind site preloads and {@link loadExtension}:
   * validates and normalises the entry, then performs it at most once. It
   * never calls `init()` itself — preloads run from inside `#create()`, and
   * awaiting `init()` there would deadlock on its own promise.
   */
  #loadEntry(entry: unknown): Promise<void> {
    const normalized = normalizePreloadEntry(entry);
    if (typeof normalized === 'string') {
      return this.#memo(`\u0000${normalized}`, () => this.#loadByName(normalized));
    }
    if ('name' in normalized) {
      const {name, repository} = normalized;
      return this.#memo(`${repository ?? ''}\u0000${name}`, () =>
        this.#loadByName(name, repository),
      );
    }
    const {url} = normalized;
    return this.#memo(`url\u0000${url}`, () => this.#loadFromUrl(url));
  }

  /** Memoises a load per key; a failure drops the entry so a retry can run. */
  #memo(key: string, load: () => Promise<void>): Promise<void> {
    const memoised = this.#loads.get(key);
    if (memoised) {
      return memoised;
    }
    const loading = load().catch((error: unknown) => {
      this.#loads.delete(key);
      throw error;
    });
    this.#loads.set(key, loading);
    return loading;
  }

  async #loadByName(name: string, repository?: string): Promise<void> {
    if (!EXTENSION_NAME_PATTERN.test(name)) {
      throw new Error(`Not a valid extension name: ${name}`);
    }
    const conn = this.#requireConnection();
    if (repository !== undefined) {
      await conn.query(`INSTALL ${name} FROM ${repositoryClause(repository)}`);
    }
    await conn.query(`LOAD ${name}`);
  }

  /** Loads an extension file from the absolute URL a `{url}` entry names. */
  async #loadFromUrl(url: string): Promise<void> {
    const conn = this.#requireConnection();
    await conn.query(`LOAD '${resolveExtensionUrl(url)}'`);
  }

  #requireConnection(): DuckdbWasm.AsyncDuckDBConnection {
    const conn = this.#conn;
    if (!conn) {
      throw new Error(this.#message || 'DuckDB unavailable');
    }
    return conn;
  }
}

/**
 * The `FROM` clause of `INSTALL`: bare keywords stay bare, URLs are quoted so
 * the recorded repository is exactly the given one; the URL pattern forbids
 * quote characters from reaching the SQL literal.
 */
function repositoryClause(repository: string): string {
  const keyword = repository.toLowerCase();
  if (REPOSITORY_KEYWORDS.has(keyword)) {
    return keyword;
  }
  if (!REPOSITORY_URL_PATTERN.test(repository)) {
    throw new Error(`Not a valid extension repository: ${repository}`);
  }
  return `'${repository}'`;
}

/**
 * Turns a preload `url` into the absolute URL the worker will fetch: the
 * worker runs from a blob URL and cannot resolve relative paths, so
 * site-relative entries (already prefixed with the site's baseUrl by the
 * build-time plugin) are resolved against the page origin here, on the main
 * thread.
 */
function resolveExtensionUrl(url: string): string {
  const absolute = isAbsoluteHttpUrl(url) ? url : new URL(url, window.location.origin).href;
  if (!EXTENSION_NAME_PATTERN.test(extensionBaseName(absolute))) {
    throw new Error(
      `An extension file must be named <extension>.duckdb_extension.wasm — the base name ` +
        `before the first dot is the entry symbol: ${absolute}`,
    );
  }
  return absolute;
}

/** Reads the JSON config the build-time plugin injects; missing means defaults. */
function readSiteRuntimeConfig(): SiteRuntimeConfig {
  if (typeof document === 'undefined') {
    return {preload: []};
  }
  const text = document.getElementById(DFK_SQL_RUNTIME_TAG_ID)?.textContent?.trim();
  if (!text) {
    return {preload: []};
  }
  try {
    return parseSiteRuntimeConfig(JSON.parse(text));
  } catch (error: unknown) {
    throw new Error(`Invalid <script id="${DFK_SQL_RUNTIME_TAG_ID}"> config: ${errorMessage(error)}`);
  }
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
