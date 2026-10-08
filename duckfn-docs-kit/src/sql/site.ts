/**
 * Node-side resolution of a docs site's SQL-test configuration: which content
 * directories to scan, which extension to preload, and which DuckDB-Wasm
 * platform / browser / timeout to use.
 *
 * It is shared by the `duckfn-sql-verify` command (`sql/verify`) and the
 * Playwright Test integration (`sql/playwright`) so the two entry points read
 * exactly the same defaults and the same `DFK_*` environment overrides — a site
 * that switches on Playwright Test does not have to re-declare its layout.
 */
import {readdirSync, statSync} from 'node:fs';
import {join, resolve} from 'node:path';

import type {WasmPlatform} from './harnessServer';

/** Where a Docusaurus site keeps its pages: English in `docs/`, translations under `i18n/`. */
export const DEFAULT_CONTENT = ['docs', 'i18n'] as const;
export const DEFAULT_TIMEOUT_MS = 30_000;

/**
 * A local directory served at a URL prefix while the blocks run, so SQL can read
 * data files the site also ships (`read_csv_auto('{{DFK_BASE_URL}}data/x.tsv')`).
 *
 * The tests run against a short-lived loopback server, so the `url` must be an
 * absolute path — normally the deployed `baseUrl` prefix, e.g. `/my-site/data`:
 * blocks put the page URL in front of it (DuckDB-Wasm reads nothing relative, see
 * `sql/placeholders`), which makes the identical path correct on the harness's
 * random port and on GitHub Pages' sub-path.
 */
export interface StaticAssetMount {
  /** URL prefix the files are reachable at; must start with `/`. */
  url: string;
  /** Directory to serve, relative to the site root (or absolute). */
  dir: string;
}

export interface DocsSiteConfig {
  /** Absolute site root. */
  siteDir: string;
  /** Content directories relative to the site root. */
  contentDirs: string[];
  /** The extension to `LOAD`: a path or an absolute `http(s)` URL; `undefined` auto-detects. */
  extension?: string;
  /**
   * The site's base URL as an absolute path (`/my-site/`), the value
   * `{{DFK_BASE_URL}}` expands to. `undefined` means the site root; the asset
   * mounts are what a block's URL actually has to match.
   */
  baseUrl?: string;
  platform: WasmPlatform;
  /** Engine wasm override, for pinning a specific DuckDB-Wasm build. */
  engine?: string;
  /** Browser executable; `DFK_BROWSER` is the same override. */
  browser?: string;
  /** Per-block timeout in milliseconds. */
  timeoutMs: number;
  /** Local directories served over HTTP for the blocks, keyed by URL prefix. */
  assets: StaticAssetMount[];
}

export interface DocsSiteOverrides {
  siteDir?: string;
  contentDirs?: readonly string[];
  extension?: string;
  baseUrl?: string;
  platform?: WasmPlatform;
  engine?: string;
  browser?: string;
  timeoutMs?: number;
  assets?: readonly StaticAssetMount[];
}

/**
 * Resolves the configuration: explicit overrides win, then `DFK_*` environment
 * variables, then a detected default (the site layout under the working
 * directory).
 */
export function resolveDocsSiteConfig(overrides: DocsSiteOverrides = {}): DocsSiteConfig {
  const siteDir = resolve(overrides.siteDir ?? process.env.DFK_SITE_DIR ?? process.cwd());
  const content = overrides.contentDirs ?? contentDirsFromEnv() ?? defaultContentDirs(siteDir);
  const assets = overrides.assets ?? assetsFromEnv() ?? [];
  return {
    siteDir,
    contentDirs: [...content],
    extension: overrides.extension ?? process.env.DFK_EXTENSION,
    baseUrl: overrides.baseUrl ?? process.env.DFK_BASE_URL,
    platform:
      overrides.platform ??
      asPlatform(process.env.DFK_PLATFORM) ??
      'eh',
    engine: overrides.engine ?? process.env.DFK_ENGINE,
    browser: overrides.browser ?? process.env.DFK_BROWSER,
    timeoutMs: overrides.timeoutMs ?? numberFromEnv('DFK_TIMEOUT') ?? DEFAULT_TIMEOUT_MS,
    assets: assets.map((mount) => normalizeMount(siteDir, mount)),
  };
}

/**
 * Normalises one mount: the URL prefix is absolute and slash-free at the end
 * (`/a/b/` -> `/a/b`), the directory relative to the site root becomes an
 * absolute one. Throws on a relative URL, which could never be resolved against
 * the page origin.
 */
function normalizeMount(siteDir: string, mount: StaticAssetMount): StaticAssetMount {
  if (!mount.url.startsWith('/')) {
    throw new Error(
      `sql/site: asset mount URL must start with "/" (got ${JSON.stringify(mount.url)}) — ` +
        `blocks read root-relative paths, so it is usually the site's baseUrl prefix`,
    );
  }
  const url = mount.url.replace(/\/+$/, '') || '/';
  return {url, dir: resolve(siteDir, mount.dir)};
}

/**
 * The extension to preload: an explicit path or absolute `http(s)` URL, or the
 * single file under `<siteDir>/static/duckdb-extensions/`.
 */
export function resolveExtension(siteDir: string, requested?: string): string {
  if (!requested) {
    return defaultExtension(siteDir);
  }
  return /^https?:\/\//i.test(requested) ? requested : resolve(requested);
}

/**
 * Where a Docusaurus site keeps its pages: the English sources in `docs/`, and
 * each translation under `i18n/<locale>/docusaurus-plugin-content-docs/current/`.
 */
export function defaultContentDirs(siteDir: string): string[] {
  const dirs: string[] = [];
  if (isDirectory(join(siteDir, DEFAULT_CONTENT[0]))) {
    dirs.push(DEFAULT_CONTENT[0]);
  }
  const i18n = join(siteDir, DEFAULT_CONTENT[1]);
  if (isDirectory(i18n)) {
    for (const locale of readdirSync(i18n)) {
      const translated = join(i18n, locale, 'docusaurus-plugin-content-docs', 'current');
      if (isDirectory(translated)) {
        dirs.push(join('i18n', locale, 'docusaurus-plugin-content-docs', 'current'));
      }
    }
  }
  return dirs.length > 0 ? dirs : ['.'];
}

/** The site's preloaded extension, as `dfkExtensions` places it under `static/`. */
export function defaultExtension(siteDir: string): string {
  const dir = join(siteDir, 'static', 'duckdb-extensions');
  const candidates = isDirectory(dir)
    ? readdirSync(dir).filter((name) => name.endsWith('.duckdb_extension.wasm'))
    : [];
  if (candidates.length === 0) {
    throw new Error(
      `sql/site: no extension found in ${dir} — pass --extension <file|url>, or let the site's ` +
        `extension preload plugin fetch it first`,
    );
  }
  if (candidates.length > 1) {
    throw new Error(
      `sql/site: several extensions found in ${dir} (${candidates.join(', ')}) — ` +
        `pass --extension to pick one`,
    );
  }
  return join(dir, candidates[0] as string);
}

export function isDirectory(path: string): boolean {
  return statSync(path, {throwIfNoEntry: false})?.isDirectory() ?? false;
}

/** `DFK_CONTENT` as a comma-separated list, or `null` when unset/empty. */
function contentDirsFromEnv(): string[] | null {
  const raw = process.env.DFK_CONTENT;
  if (!raw) {
    return null;
  }
  const dirs = raw
    .split(',')
    .map((dir) => dir.trim())
    .filter(Boolean);
  return dirs.length > 0 ? dirs : null;
}

/**
 * `DFK_ASSETS` as a comma-separated `url=dir` list, or `null` when unset/empty.
 * The directories stay as written — {@link resolveDocsSiteConfig} resolves them
 * against the site root once it knows it.
 */
function assetsFromEnv(): StaticAssetMount[] | null {
  const raw = process.env.DFK_ASSETS;
  if (!raw) {
    return null;
  }
  const mounts: StaticAssetMount[] = [];
  for (const entry of raw.split(',')) {
    const text = entry.trim();
    if (!text) {
      continue;
    }
    const equals = text.indexOf('=');
    if (equals <= 0 || equals === text.length - 1) {
      throw new Error(`sql/site: DFK_ASSETS entry ${JSON.stringify(text)} is not "url=dir"`);
    }
    mounts.push({url: text.slice(0, equals).trim(), dir: text.slice(equals + 1).trim()});
  }
  return mounts.length > 0 ? mounts : null;
}

function asPlatform(value: string | undefined): WasmPlatform | null {
  return value === 'eh' || value === 'mvp' ? value : null;
}

function numberFromEnv(name: string): number | null {
  const raw = process.env[name];
  if (!raw) {
    return null;
  }
  const value = Number(raw);
  return Number.isFinite(value) ? value : null;
}