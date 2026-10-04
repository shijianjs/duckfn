/**
 * The browser fixture shared by the `duckfn-sql-verify` command
 * (`sql/browserRunner`) and the Playwright Test integration
 * (`sql/playwright`).
 *
 * Both need the same thing before a block can run: a short-lived **loopback
 * static server** that serves the harness page, the DuckDB-Wasm engine + worker
 * and the extension under test — all from local files, so verification never
 * touches the network. Keeping the routes and browser detection in one module
 * means the two entry points cannot drift: a block verified through the CLI and
 * a block verified through Playwright Test run against byte-identical fixtures.
 */
import {existsSync, readFileSync} from 'node:fs';
import {createServer, type Server} from 'node:http';
import {createRequire} from 'node:module';
import {basename, dirname, join} from 'node:path';

const require = createRequire(import.meta.url);

/** DuckDB-Wasm platform bundle; must match how the extension was built. */
export type WasmPlatform = 'eh' | 'mvp';

/** Per-platform file names inside `@duckdb/duckdb-wasm/dist`. */
export const PLATFORM_BUNDLES: Record<WasmPlatform, {wasm: string; worker: string}> = {
  eh: {wasm: 'duckdb-eh.wasm', worker: 'duckdb-browser-eh.worker.js'},
  mvp: {wasm: 'duckdb-mvp.wasm', worker: 'duckdb-browser-mvp.worker.js'},
};

export interface EngineBundle {
  enginePath: string;
  workerPath: string;
}

/** Resolves the engine wasm and its worker from `@duckdb/duckdb-wasm/dist`. */
export function resolveEngineBundle(platform: WasmPlatform = 'eh', engine?: string): EngineBundle {
  const bundle = PLATFORM_BUNDLES[platform];
  const distDir = dirname(require.resolve('@duckdb/duckdb-wasm/dist/duckdb-browser.mjs'));
  return {
    enginePath: engine ? resolveLocal(engine) : join(distDir, bundle.wasm),
    workerPath: join(distDir, bundle.worker),
  };
}

/** A path or `file:` URL to an absolute local path; leaves a remote URL alone. */
export function resolveLocal(value: string): string {
  return /^file:\/\//i.test(value) ? new URL(value).pathname.replace(/^\/(\w:)/i, '$1') : value;
}

export interface HarnessRoutes {
  harnessScript: string;
  engine: {file: string; name: string};
  worker: {file: string; name: string};
  extension: {remoteUrl: string} | {localFile: string; baseName: string};
  allowUnsigned: boolean;
}

export interface HarnessRoutesOptions {
  /** The extension to preload: a local `.duckdb_extension.wasm` path or an http(s) URL. */
  extension: string;
  enginePath: string;
  workerPath: string;
  /**
   * Accept an unsigned extension. Defaults to `true`: the loopback fixture is a
   * trusted local harness, exactly as the site's own config opts in.
   */
  allowUnsigned?: boolean;
}

/** Maps the URL space the harness page needs to everything on disk. */
export function harnessRoutes(options: HarnessRoutesOptions): HarnessRoutes {
  // A served extension keeps a `/ext/<name>.duckdb_extension.wasm` shape so its
  // base name still names the entry symbol (`<name>_init_c_api`).
  const remote = /^https?:\/\//i.test(options.extension);
  const localFile = remote ? '' : resolveLocal(options.extension);
  return {
    // Resolved through the package's own `exports` wildcard
    // (`./sql/harness` -> `dist/sql/harness.js`), so it finds the built harness
    // whether the runner runs from the workspace symlink or an installed copy.
    harnessScript: require.resolve('duckfn-docs-kit/sql/harness'),
    engine: {file: options.enginePath, name: basename(options.enginePath)},
    worker: {file: options.workerPath, name: basename(options.workerPath)},
    extension: remote
      ? {remoteUrl: options.extension}
      : {localFile, baseName: basename(localFile)},
    allowUnsigned: options.allowUnsigned ?? true,
  };
}

export interface Harness {
  /** The `harness.html` URL, ready to `page.goto()`. */
  url: string;
  server: Server;
}

/** Starts the static server and reports the harness URL to navigate to. */
export async function startHarness(routes: HarnessRoutes): Promise<Harness> {
  const server = await startStaticServer(routes);
  const address = server.address();
  const port = typeof address === 'object' && address ? address.port : 0;
  return {url: `http://127.0.0.1:${port}/harness.html`, server};
}

/**
 * A loopback static server for the harness: `harness.html` (generated, wiring
 * the engine/extension URLs into the two contracts `harness.ts` reads),
 * `harness.js` (the bundled harness), `/vendor/*` (engine + worker), and
 * `/ext/*` (the served extension file). Nothing else is reachable; this is a
 * short-lived test fixture, not a web server.
 */
export async function startStaticServer(routes: HarnessRoutes): Promise<Server> {
  const harness = readFileSync(routes.harnessScript);
  const engine = readFileSync(routes.engine.file);
  const worker = readFileSync(routes.worker.file);
  const extension =
    'localFile' in routes.extension ? readFileSync(routes.extension.localFile) : null;
  const extBaseName = 'baseName' in routes.extension ? routes.extension.baseName : '';

  const extensionUrl =
    'remoteUrl' in routes.extension ? routes.extension.remoteUrl : `ext/${routes.extension.baseName}`;

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

/**
 * A path to a system Chrome/Edge, or `null` when none of the well-known
 * locations exist. Overridable by `DFK_BROWSER` / an explicit path.
 */
export function findBrowserPath(explicit?: string): string | null {
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
  return null;
}

/** {@link findBrowserPath}, but throws a hint when no browser is installed. */
export function findBrowser(explicit?: string): string {
  const path = findBrowserPath(explicit);
  if (!path) {
    throw new Error(
      'sql/harnessServer: no Chrome/Edge found — pass --browser <path> or set DFK_BROWSER',
    );
  }
  return path;
}