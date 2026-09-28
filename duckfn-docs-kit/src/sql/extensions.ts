import {createHash} from 'node:crypto';
import {mkdir, readFile, rename, stat, unlink, writeFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import path from 'node:path';
import {
  DFK_SQL_RUNTIME_TAG_ID,
  isAbsoluteHttpUrl,
  normalizePreloadEntry,
  type PreloadEntry,
  type PreloadReleaseSource,
  type SiteRuntimeConfig,
} from './runtimeConfig';

/**
 * `duckfn-docs-kit/sql/extensions` — the Docusaurus plugin behind the
 * runnable-SQL examples' extensions.
 *
 * It does two things, both driven by one ordered `preload` list configured in
 * `docusaurus.config.ts`:
 *
 * 1. At startup (dev server and build alike) it fetches every `release`
 *    source from that GitHub repository's **latest** release into the site's
 *    `static/` directory, where the file is then served same-origin.
 *    Downloads are cached locally and only re-fetched when the release
 *    asset's sha256 differs (GitHub's own `digest` field is the comparison).
 * 2. It injects the resolved preload list as one JSON `<script>` tag into
 *    every page; the browser runtime (`sql/runtime`) reads it once and loads
 *    the extensions in order while the DuckDB instance initialises.
 *
 * It also injects the kit's client bootstrap (`sql/client.ts`) on every page
 * through `getClientModules()`: docs pages never import the kit's React tree,
 * so the `dfk-*` element registration has to come from a client module — and
 * it comes from here, not from a file the site keeps by hand.
 *
 * The site-relative `url` of an entry doubles as the fetch destination *and*
 * the runtime path, so the two can never drift: with baseUrl `/duckfn/`,
 * `duckdb-extensions/duckfn.duckdb_extension.wasm` lands in
 * `<siteDir>/static/duckdb-extensions/duckfn.duckdb_extension.wasm` and is
 * preloaded from `/duckfn/duckdb-extensions/duckfn.duckdb_extension.wasm`.
 * (Docusaurus serves `static/` under each locale's baseUrl, so the localized
 * value from the plugin context is the right prefix for every build.)
 *
 * This is Node-side build code: it must not import any browser module, and
 * the browser side must not import this file (the shared contract lives in
 * `./runtimeConfig`).
 */

/**
 * The slice of Docusaurus' plugin API this module touches, typed structurally
 * instead of importing `@docusaurus/types`: the kit stays free of Docusaurus
 * dependencies, and the consuming site's own typecheck proves compatibility
 * when the returned module lands in its `plugins` list.
 */
export interface DfkExtensionsContext {
  siteDir: string;
  siteConfig: {baseUrl: string};
}

interface DfkHtmlTag {
  tagName: string;
  attributes: Record<string, string>;
  innerHTML: string;
}

export interface DfkExtensionsPlugin {
  name: string;
  getClientModules(): string[];
  loadContent(): Promise<void>;
  injectHtmlTags(): {headTags: DfkHtmlTag[]};
}

/** The plugin module Docusaurus calls with its `LoadContext` and options. */
export type DfkExtensionsPluginModule = (context: DfkExtensionsContext) => DfkExtensionsPlugin;

/** Options for {@link dfkExtensions}. */
export interface DfkExtensionsOptions {
  /**
   * The ordered list of extensions every page preloads while the shared
   * DuckDB instance initialises — official names, `{name, repository}`
   * entries, or `{url}` files (optionally fetched from a GitHub release).
   * See `PreloadEntry` in `./runtimeConfig` for the exact shapes.
   */
  preload?: PreloadEntry[];
  /**
   * Let `LOAD` accept extensions whose signature does not verify. Needed
   * whenever a preloaded file is not signed with DuckDB's keys — the GitHub
   * release assets of a third-party extension are not — and merged with the
   * per-block setting of whichever block initialises the runtime first.
   */
  allowUnsignedExtensions?: boolean;
  /**
   * Where release assets are cached between builds. Relative paths resolve
   * against the Docusaurus site directory; defaults to
   * `<siteDir>/.cache/duckfn-docs-kit`.
   */
  cacheDir?: string;
  /** A GitHub token for the release API (rate limits, private repositories). Defaults to `process.env.GITHUB_TOKEN`. */
  token?: string;
}

const LOG_PREFIX = '[dfk-extensions] ';

/**
 * Builds the Docusaurus plugin. Usage in `docusaurus.config.ts`:
 *
 * ```ts
 * import {dfkExtensions} from 'duckfn-docs-kit/sql/extensions';
 *
 * plugins: [
 *   dfkExtensions({
 *     allowUnsignedExtensions: true,
 *     preload: [
 *       {name: 'inet'},
 *       {
 *         url: 'duckdb-extensions/duckfn.duckdb_extension.wasm',
 *         release: {repository: 'shijianjs/duckfn', asset: 'duckfn-wasm_eh.duckdb_extension.wasm'},
 *       },
 *     ],
 *   }),
 * ],
 * ```
 */
export function dfkExtensions(options: DfkExtensionsOptions = {}): DfkExtensionsPluginModule {
  const {
    allowUnsignedExtensions = false,
    preload = [],
    cacheDir,
    token = process.env.GITHUB_TOKEN,
  } = options;

  // Validated once, however the lifecycles are entered, so a malformed entry
  // fails the build before any download is attempted.
  let prepared: SiteRuntimeConfig | null = null;
  const prepare = (): SiteRuntimeConfig => {
    prepared ??= {
      ...(allowUnsignedExtensions ? {allowUnsignedExtensions: true} : {}),
      preload: preload.map((entry, index) => {
        try {
          return normalizePreloadEntry(entry);
        } catch (error) {
          throw new Error(`duckfn-docs-kit preload[${index}]: ${messageOf(error)}`);
        }
      }),
    };
    return prepared;
  };

  return (context) => ({
    name: 'dfk-extensions',

    /**
     * The `dfk-*` element registration (see `sql/client.ts`), injected on
     * every page so docs pages — whose React tree never imports the kit —
     * still upgrade the runnable-SQL elements.
     */
    getClientModules() {
      // Resolved from the consuming site, so the config bundler cannot break
      // the lookup; the exports map (`./*` -> dist) keeps the subpath valid
      // even if the entry moves.
      const requireFromSite = createRequire(path.join(context.siteDir, 'package.json'));
      return [requireFromSite.resolve('duckfn-docs-kit/sql/client')];
    },

    async loadContent() {
      const config = prepare();
      const {siteDir} = context;
      const cacheRoot = path.resolve(siteDir, cacheDir ?? '.cache/duckfn-docs-kit');
      for (const entry of config.preload) {
        if (typeof entry === 'string' || !('url' in entry)) {
          continue;
        }
        const dest = path.join(siteDir, 'static', entry.url);
        const label = `static/${entry.url}`;
        if (entry.release) {
          await fetchReleaseAsset(entry.release, {dest, label, cacheRoot, token});
        } else if (!isAbsoluteHttpUrl(entry.url) && !(await fileExists(dest))) {
          throw new Error(
            `${LOG_PREFIX}preload url "${entry.url}" has no file: place the file under ` +
              `static/${entry.url}, or point the entry at a GitHub release source`,
          );
        }
      }
    },

    injectHtmlTags() {
      const config = prepare();
      if (!config.allowUnsignedExtensions && config.preload.length === 0) {
        return {headTags: []};
      }
      const resolved: SiteRuntimeConfig = {
        ...config,
        preload: config.preload.map((entry) => resolveForSite(entry, context.siteConfig.baseUrl)),
      };
      return {
        headTags: [
          {
            tagName: 'script',
            attributes: {id: DFK_SQL_RUNTIME_TAG_ID, type: 'application/json'},
            // `<` is escaped so no config string can ever close the script tag.
            innerHTML: JSON.stringify(resolved).replaceAll('<', '\\u003c'),
          },
        ],
      };
    },
  });
}

/** Fetches one release asset into `dest`, going through the local cache. */
async function fetchReleaseAsset(
  source: PreloadReleaseSource,
  options: {dest: string; label: string; cacheRoot: string; token?: string},
): Promise<void> {
  const {dest, label, cacheRoot, token} = options;
  const log = (text: string): void => console.log(`${LOG_PREFIX}${source.repository}#${source.asset}: ${text}`);
  const resolved = await resolveReleaseAsset(source, {cacheRoot, token, log});
  await writeIfChanged(dest, resolved.bytes, resolved.sha256, log, label);
}

/**
 * Resolves the asset's bytes: cache hit when the release digest matches,
 * otherwise download + verify + re-cache. A network failure falls back to a
 * cached copy with a warning — offline development keeps working — but only
 * when a cache exists, so CI (which starts empty) fails loudly.
 */
async function resolveReleaseAsset(
  source: PreloadReleaseSource,
  options: {cacheRoot: string; token?: string; log: (text: string) => void},
): Promise<CachedAsset> {
  const {cacheRoot, token, log} = options;
  const cachePaths = cacheFilePaths(cacheRoot, source);
  const cached = await readCachedAsset(cachePaths);
  const useCached = (error: unknown): CachedAsset => {
    if (!cached) {
      throw error;
    }
    console.warn(
      `${LOG_PREFIX}${source.repository}#${source.asset}: ${messageOf(error)} — ` +
        `using the cached copy (${shortHash(cached.sha256)})`,
    );
    return cached;
  };

  let release: GithubRelease;
  try {
    release = await fetchLatestRelease(source.repository, token);
  } catch (error) {
    return useCached(error);
  }
  const asset = pickAsset(release, source);
  const expected = digestSha256(asset.digest);
  if (cached && expected !== null && cached.sha256 === expected) {
    log(`up to date (${shortHash(expected)})`);
    return cached;
  }

  log(`downloading${release.tag_name ? ` ${release.tag_name}` : ''}…`);
  let bytes: Buffer;
  try {
    bytes = await downloadAsset(asset, token);
  } catch (error) {
    return useCached(error);
  }
  const sha256 = sha256Hex(bytes);
  if (expected !== null && sha256 !== expected) {
    // Louder than the cache fallback on purpose: the bytes on offer do not
    // match what GitHub says the release asset is.
    throw new Error(
      `${LOG_PREFIX}${source.repository}#${source.asset}: sha256 mismatch, ` +
        `expected ${expected}, downloaded ${sha256}`,
    );
  }
  const tag = release.tag_name ?? '';
  await writeCachedAsset(cachePaths, bytes, sha256, tag);
  log(`cached ${shortHash(sha256)}`);
  return {bytes, sha256, tag};
}

/** One GitHub release asset, reduced to the fields this plugin reads. */
interface GithubAsset {
  name?: string;
  browser_download_url?: string;
  digest?: string | null;
}

interface GithubRelease {
  tag_name?: string;
  assets?: GithubAsset[];
}

const GITHUB_API = 'https://api.github.com';

async function fetchLatestRelease(repository: string, token: string | undefined): Promise<GithubRelease> {
  const headers = githubHeaders(token, 'application/vnd.github+json');
  const response = await fetch(`${GITHUB_API}/repos/${repository}/releases/latest`, {headers});
  if (!response.ok) {
    throw new Error(`GitHub API ${response.status} for ${repository}: ${(await response.text()).slice(0, 200)}`);
  }
  return (await response.json()) as GithubRelease;
}

function pickAsset(release: GithubRelease, source: PreloadReleaseSource): GithubAsset {
  const assets = release.assets ?? [];
  const asset = assets.find((candidate) => candidate.name === source.asset);
  if (!asset || typeof asset.browser_download_url !== 'string') {
    const available = assets.map((candidate) => candidate.name).filter(Boolean).join(', ') || 'none';
    throw new Error(
      `${LOG_PREFIX}release ${release.tag_name ?? '?'} of ${source.repository} has no asset ` +
        `"${source.asset}" (available: ${available})`,
    );
  }
  return asset;
}

async function downloadAsset(asset: GithubAsset, token: string | undefined): Promise<Buffer> {
  const response = await fetch(asset.browser_download_url!, {
    headers: githubHeaders(token, 'application/octet-stream'),
    redirect: 'follow',
  });
  if (!response.ok) {
    throw new Error(`asset download failed with ${response.status}: ${asset.browser_download_url}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

/** Headers shared by the API and asset requests (the API rejects requests without a User-Agent). */
function githubHeaders(token: string | undefined, accept: string): Record<string, string> {
  const headers: Record<string, string> = {
    Accept: accept,
    'User-Agent': 'duckfn-docs-kit',
    'X-GitHub-Api-Version': '2022-11-28',
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  return headers;
}

/** GitHub's `digest` field (`sha256:<hex>`) as a bare lowercase hex digest; null when absent or unparseable. */
function digestSha256(digest: string | null | undefined): string | null {
  if (typeof digest !== 'string') {
    return null;
  }
  const [algorithm, hex] = digest.split(':');
  if (algorithm !== 'sha256' || hex === undefined || !/^[0-9a-f]{64}$/i.test(hex)) {
    return null;
  }
  return hex.toLowerCase();
}

function sha256Hex(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}

interface CachePaths {
  file: string;
  meta: string;
}

interface CachedAsset {
  bytes: Buffer;
  sha256: string;
  tag: string;
}

interface CacheMeta {
  sha256?: string;
  tag?: string;
  fetchedAt?: string;
}

function cacheFilePaths(cacheRoot: string, source: PreloadReleaseSource): CachePaths {
  const dir = path.join(cacheRoot, source.repository.replace('/', '__'));
  return {file: path.join(dir, source.asset), meta: path.join(dir, `${source.asset}.json`)};
}

async function readCachedAsset(paths: CachePaths): Promise<CachedAsset | null> {
  try {
    const [bytes, metaText] = await Promise.all([readFile(paths.file), readFile(paths.meta, 'utf8')]);
    const meta = JSON.parse(metaText) as CacheMeta;
    if (typeof meta.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(meta.sha256)) {
      return null;
    }
    return {bytes, sha256: meta.sha256, tag: meta.tag ?? ''};
  } catch {
    // No (readable) cache — the normal state on the first run.
    return null;
  }
}

async function writeCachedAsset(paths: CachePaths, bytes: Buffer, sha256: string, tag: string): Promise<void> {
  await mkdir(path.dirname(paths.file), {recursive: true});
  await writeFileAtomically(paths.file, bytes);
  const meta: CacheMeta = {sha256, tag, fetchedAt: new Date().toISOString()};
  await writeFileAtomically(paths.meta, Buffer.from(`${JSON.stringify(meta, null, 2)}\n`));
}

/** Writes `bytes` to `file` unless it already holds exactly those bytes. */
async function writeIfChanged(
  file: string,
  bytes: Buffer,
  sha256: string,
  log: (text: string) => void,
  label: string,
): Promise<void> {
  const current = await readFile(file).catch(() => null);
  if (current && sha256Hex(current) === sha256) {
    log(`${label} up to date`);
    return;
  }
  await mkdir(path.dirname(file), {recursive: true});
  await writeFileAtomically(file, bytes);
  log(`wrote ${label} (${bytes.length} bytes)`);
}

/** Write via a temporary file + rename so a reader never sees a partial file. */
async function writeFileAtomically(file: string, bytes: Buffer): Promise<void> {
  const tmp = `${file}.tmp-${process.pid.toString(36)}-${Date.now().toString(36)}`;
  await writeFile(tmp, bytes);
  for (let attempt = 1; ; attempt += 1) {
    try {
      await rename(tmp, file);
      return;
    } catch (error) {
      if (attempt >= 3) {
        await unlink(tmp).catch(() => undefined);
        throw error;
      }
      // Windows can hold a transient lock on the destination (antivirus,
      // indexers); wait a moment and try again.
      await new Promise((resolve) => setTimeout(resolve, 50 * attempt));
    }
  }
}

/**
 * Prefixes site-relative paths with the deploy's baseUrl so the runtime can
 * resolve them against the page origin; absolute URLs pass through, and the
 * build-only `release` source is stripped from what the browser sees.
 */
function resolveForSite(entry: PreloadEntry, baseUrl: string): PreloadEntry {
  if (typeof entry === 'string' || !('url' in entry)) {
    return entry;
  }
  return isAbsoluteHttpUrl(entry.url)
    ? {url: entry.url}
    : {url: joinBaseUrl(baseUrl, entry.url)};
}

/** `/duckfn/` + `duckdb-extensions/x.wasm` -> `/duckfn/duckdb-extensions/x.wasm`. */
function joinBaseUrl(baseUrl: string, url: string): string {
  const base = baseUrl.replace(/^\/+|\/+$/g, '');
  return base === '' ? `/${url}` : `/${base}/${url}`;
}

async function fileExists(file: string): Promise<boolean> {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}

function shortHash(sha256: string): string {
  return sha256.slice(0, 12);
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
