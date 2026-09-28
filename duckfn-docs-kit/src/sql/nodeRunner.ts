/**
 * Runs runnable SQL blocks in DuckDB-Wasm from Node, on the same architecture
 * the site itself uses: the official Node **worker** target, one instance per
 * page, one connection per page.
 *
 * Three platform facts shape this file, all of them measured rather than
 * guessed (duckfn's AGENTS.md records the investigation):
 *
 * 1. The official Node *blocking* target (`duckdb-node-blocking.cjs`) deadlocks
 *    when it loads an extension that opens its own connection while registering
 *    — which is exactly what a docs site's extension may do. The worker target
 *    is the only Node target that works.
 * 2. A bare file name in `LOAD` is not usable on wasm: it hangs, without an
 *    error, even with the file registered through `registerFileBuffer`. The
 *    extension has to be fetched over http(s), like the site does it.
 * 3. DuckDB stages that URL under
 *    `~/.duckdb/extensions/<host>[:<port>]/<first path segment>/` — which is
 *    why the served URL keeps a path segment, and why the directory is
 *    pre-created: the loader's own `mkdir` is non-recursive. A colon is illegal
 *    in a Windows path, so the port plan is per platform: Windows takes the
 *    default port 80 (the URL then carries no port at all), POSIX takes any
 *    free port. The directory's previous content is removed so a stale copy can
 *    never be what runs.
 */
import {createServer, type Server} from 'node:http';
import {Worker as WorkerThread} from 'node:worker_threads';
import {createRequire} from 'node:module';
import {mkdirSync, readFileSync, rmSync} from 'node:fs';
import {homedir} from 'node:os';
import {basename, dirname, join} from 'node:path';

/** The slice of the Node target's surface this runner uses (it ships no types). */
interface DuckdbNodeTarget {
  AsyncDuckDB: new (logger: unknown, worker: unknown) => DuckdbDatabase;
  ConsoleLogger: new () => unknown;
}

export interface DuckdbQueryResult {
  numRows: number;
  schema: {fields: {name: string}[]};
}

export interface DuckdbConnection {
  query(sql: string): Promise<DuckdbQueryResult>;
}

interface DuckdbDatabase {
  instantiate(mainModule: string, pthreadWorker: string | null): Promise<void>;
  open(config: {allowUnsignedExtensions?: boolean}): Promise<void>;
  connect(): Promise<DuckdbConnection>;
  terminate(): Promise<void>;
}

const require = createRequire(import.meta.url);

/** Loaded through a variable so a bundler leaves the dynamic import alone. */
const NODE_TARGET = '@duckdb/duckdb-wasm/dist/duckdb-node.cjs';

export type WasmPlatform = 'eh' | 'mvp';

export interface RunnerOptions {
  /**
   * The extension to `LOAD`: a local `.duckdb_extension.wasm` path (served to
   * the worker over a loopback http server) or an absolute `http(s)` URL.
   */
  extension: string;
  /**
   * DuckDB-Wasm platform. Must match how the extension was built — a site
   * serving `duckfn-wasm_eh.duckdb_extension.wasm` runs the `eh` bundle.
   */
  platform?: WasmPlatform;
  /** The engine wasm; defaults to the one shipped beside the worker bundle. */
  engine?: string;
}

export interface RunResult {
  rows: number;
  columns: number;
}

/**
 * The WebWorker surface `AsyncDuckDB` expects, backed by a worker thread.
 *
 * The official `createWorker()` cannot be reused here: it fetches the worker
 * script, wraps it in a blob URL and resolves it as a file path, which is the
 * browser's module story. The contract it relies on internally is small —
 * `worker_threads` entry with `workerData.mod` — and that is what this
 * reproduces.
 */
class WorkerShim {
  #thread: WorkerThread;
  #listeners = new Map<string, ((event: unknown) => void)[]>();
  onmessage: ((event: unknown) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: unknown) => void) | null = null;

  constructor(mod: string) {
    this.#thread = new WorkerThread(require.resolve(NODE_TARGET), {
      workerData: {mod, name: '', type: ''},
    });
    this.#thread.on('message', (data) => this.#emit('message', data));
    this.#thread.on('error', (error) => this.#emit('error', error));
    this.#thread.on('exit', () => this.#emit('close'));
  }

  #emit(type: string, data?: unknown): void {
    const event = {type, data, target: this, currentTarget: this};
    const handler = (this as Record<string, unknown>)[`on${type}`];
    if (typeof handler === 'function') {
      (handler as (event: unknown) => void)(event);
    }
    for (const listener of this.#listeners.get(type) ?? []) {
      listener(event);
    }
  }

  addEventListener(type: string, listener: (event: unknown) => void): void {
    this.#listeners.set(type, [...(this.#listeners.get(type) ?? []), listener]);
  }

  removeEventListener(type: string, listener: (event: unknown) => void): void {
    this.#listeners.set(
      type,
      (this.#listeners.get(type) ?? []).filter((candidate) => candidate !== listener),
    );
  }

  postMessage(data: unknown, transfer?: unknown): void {
    // The worker_threads signature always wants the second argument.
    this.#thread.postMessage(data, (transfer ?? []) as never);
  }

  terminate(): Promise<number> | void {
    return this.#thread.terminate();
  }
}

/**
 * One DuckDB-Wasm instance at a time, driven from Node.
 *
 * `newPage()` is what a docs site does per page load: a fresh instance, a fresh
 * connection, the extension loaded again. Blocks of one page then share state
 * (a table created in one block is visible to the next), while pages stay
 * isolated — which is why the runner is used one page at a time rather than
 * over a single long-lived connection.
 */
export class WasmSqlRunner {
  #db: DuckdbDatabase | null = null;
  #conn: DuckdbConnection | null = null;
  #server: Server | null = null;
  #stagingDir: string | null = null;
  readonly #extensionUrl: string;
  readonly #engine: string;
  readonly #workerBundle: string;

  private constructor(extensionUrl: string, engine: string, workerBundle: string) {
    this.#extensionUrl = extensionUrl;
    this.#engine = engine;
    this.#workerBundle = workerBundle;
  }

  static async create(options: RunnerOptions): Promise<WasmSqlRunner> {
    const platform = options.platform ?? 'eh';
    const workerBundle = require.resolve(
      `@duckdb/duckdb-wasm/dist/duckdb-node-${platform}.worker.cjs`,
    );
    const engine = options.engine ?? join(dirname(workerBundle), `duckdb-${platform}.wasm`);

    const remote = /^https?:\/\//i.test(options.extension);
    const served = remote ? null : await serveExtension(options.extension);
    const url = served ? served.url : options.extension;

    const runner = new WasmSqlRunner(url, engine, workerBundle);
    runner.#server = served ? served.server : null;
    runner.#stagingDir = served
      ? prepareStagingDir(served.stagingHost, served.stagingSegment)
      : null;
    return runner;
  }

  /** Drops the current instance and starts a fresh page: new instance, new connection. */
  async newPage(): Promise<void> {
    if (this.#db) {
      await this.#db.terminate();
      this.#db = null;
      this.#conn = null;
    }
    const duckdb = (await import(/* @vite-ignore */ NODE_TARGET)) as unknown as DuckdbNodeTarget;
    const db = new duckdb.AsyncDuckDB(new duckdb.ConsoleLogger(), new WorkerShim(this.#workerBundle));
    await db.instantiate(this.#engine, null);
    await db.open({allowUnsignedExtensions: true});
    const conn = await db.connect();
    await conn.query(`LOAD '${this.#extensionUrl}'`);
    this.#db = db;
    this.#conn = conn;
  }

  /** Runs one block; the caller decides whether a failure is expected. */
  async run(sql: string): Promise<RunResult> {
    const conn = this.#conn;
    if (!conn) {
      throw new Error('sql/verify: no page is open — call newPage() first');
    }
    const table = await conn.query(sql);
    return {rows: table.numRows, columns: table.schema.fields.length};
  }

  /** Where the fetched extension was staged, for diagnostics. */
  get stagingDir(): string | null {
    return this.#stagingDir;
  }

  async close(): Promise<void> {
    if (this.#db) {
      await this.#db.terminate();
      this.#db = null;
      this.#conn = null;
    }
    if (this.#server) {
      await new Promise<void>((resolve) => this.#server?.close(() => resolve()));
      this.#server = null;
    }
  }
}

/**
 * Serves one extension file over loopback http, because wasm `LOAD` only works
 * with a URL (see the file header). The server answers any path with that one
 * file: it is a local, short-lived convenience, not a web server.
 *
 * The port choice is the platform split from the file header: the default port
 * where the platform lets a non-root process take it (so the URL stays
 * port-less, which is what Windows needs), any free port otherwise.
 */
async function serveExtension(file: string): Promise<{
  server: Server;
  url: string;
  /** The directory DuckDB will stage the download under, and its parent. */
  stagingHost: string;
  stagingSegment: string;
}> {
  const body = readFileSync(file);
  const name = basename(file);
  // DuckDB stages a fetched extension under
  // `~/.duckdb/extensions/<host>/<first path segment>/`, so the URL needs one
  // path segment — the site serves its extension from `duckdb-extensions/`, and
  // this mirrors that shape with the extension's own name.
  const segment = extensionName(file);
  const server = createServer((_request, response) => {
    response.writeHead(200, {
      'content-type': 'application/octet-stream',
      'content-length': body.length,
    });
    response.end(body);
  });
  const wanted = process.platform === 'win32' ? 80 : 0;
  await new Promise<void>((resolve, reject) => {
    server.once('error', (error: NodeJS.ErrnoException) => {
      reject(
        new Error(
          `sql/verify: cannot serve the extension on port ${wanted} (${error.code}): a port in ` +
            `the URL would put a colon into DuckDB's staging path, which is not a legal Windows ` +
            `path — free port 80, or load the extension from an http(s) URL with --extension`,
        ),
      );
    });
    server.listen(wanted, 'localhost', resolve);
  });
  const address = server.address();
  const bound = typeof address === 'object' && address ? address.port : wanted;
  const host = bound === 80 ? 'localhost' : `localhost:${bound}`;
  return {
    server,
    url: `http://${host}/${segment}/${name}`,
    stagingHost: host,
    stagingSegment: segment,
  };
}

/**
 * Pre-creates DuckDB's staging directory and empties it.
 *
 * The directory has to exist beforehand (the loader's own `mkdir` is
 * non-recursive), and anything left in it from an earlier run would be reused
 * instead of the extension being fetched again — a stale binary silently
 * testing the wrong build.
 */
function prepareStagingDir(host: string, segment: string): string {
  const dir = join(homedir(), '.duckdb', 'extensions', host, segment);
  rmSync(dir, {recursive: true, force: true});
  mkdirSync(dir, {recursive: true});
  return dir;
}

/** The text before the first dot of the file name — the entry symbol DuckDB looks up. */
function extensionName(file: string): string {
  return basename(file).split('.')[0] ?? '';
}
