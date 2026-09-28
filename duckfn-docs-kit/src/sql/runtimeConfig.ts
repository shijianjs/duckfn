/**
 * The contract between the build-time plugin (`sql/extensions`) and the
 * browser runtime (`sql/runtime`).
 *
 * The plugin writes one JSON `<script>` tag into every page carrying the
 * site's runtime configuration; the runtime reads it once, when the shared
 * DuckDB instance is first created. Tag id, entry shapes and the validators
 * live together in this dependency-free module so the two sides cannot drift —
 * and so the browser bundle never drags in Node code (or the Node plugin the
 * DOM).
 *
 * This module must not import anything: it is bundled into both the browser
 * and the Node entry points.
 */

/** Id of the JSON config `<script>` the build-time plugin injects per page. */
export const DFK_SQL_RUNTIME_TAG_ID = 'dfk-sql-runtime';

/** A bare SQL identifier: `LOAD` / `INSTALL` cannot be parameterised, so this is the guard. */
export const EXTENSION_NAME_PATTERN = /^[a-z][a-z0-9_]*$/i;

/** A repository URL with no character that could escape the SQL string literal. */
export const REPOSITORY_URL_PATTERN = /^https?:\/\/[^\s'";`<>\\]+$/i;

/** Bare `FROM` keywords `INSTALL` accepts instead of a repository URL. */
export const REPOSITORY_KEYWORDS = new Set(['community', 'core']);

/** An extension preloaded by name, from the official repository or another one. */
export interface NamedPreloadEntry {
  /** The extension name, e.g. `duckfn`. */
  name: string;
  /** `community`, `core` or a repository URL; omitted = the official repository. */
  repository?: string;
}

/** A GitHub release asset the build-time plugin copies to `url` at build time. */
export interface PreloadReleaseSource {
  /** The GitHub repository, `owner/name`. */
  repository: string;
  /** The asset name on that repository's latest release, e.g. `duckfn-wasm_eh.duckdb_extension.wasm`. */
  asset: string;
}

/** An extension preloaded from a file, served by the site itself or remotely. */
export interface UrlPreloadEntry {
  /**
   * A site-relative path (`duckdb-extensions/duckfn.duckdb_extension.wasm`),
   * resolved against the site's baseUrl and served from `static/`; or an
   * absolute `http(s)://` URL.
   *
   * The base name of the last path segment — the text before its first dot —
   * must be the extension name: on WebAssembly that text is what DuckDB turns
   * into the `<name>_init_c_api` entry symbol, which is exactly why a release
   * asset like `duckfn-wasm_eh.duckdb_extension.wasm` has to be renamed to
   * `duckfn.duckdb_extension.wasm` on the way in.
   */
  url: string;
  /** Fetch the file from the repository's latest GitHub release at build time. */
  release?: PreloadReleaseSource;
}

/** One entry of the ordered site-level preload list. */
export type PreloadEntry = string | NamedPreloadEntry | UrlPreloadEntry;

/** The config the build-time plugin injects and the browser runtime reads. */
export interface SiteRuntimeConfig {
  /** Let `LOAD` accept extensions without a valid signature; a site-wide opt-in. */
  allowUnsignedExtensions?: boolean;
  /** Ordered: every entry loads, one after another, before the instance is `ready`. */
  preload: PreloadEntry[];
}

/** `owner/name`. */
const GITHUB_REPOSITORY_PATTERN = /^[\w.-]+\/[\w.-]+$/;

/** A file name / URL safe for the injected JSON, the SQL literal and a static path. */
const SAFE_URL_TEXT_PATTERN = /^[^\s'";`<>\\?#]+$/;

/** Anything that starts like a URL scheme; only `http(s):` may follow it. */
const URL_SCHEME_PATTERN = /^[a-z][a-z0-9+.-]*:/i;

/** True for the only absolute URL form a preload may carry. */
export function isAbsoluteHttpUrl(value: string): boolean {
  return /^https?:\/\//i.test(value);
}

/**
 * The entry-symbol base name of a served extension file: the text before the
 * first dot in its last path segment. DuckDB loads a direct file by that name
 * (`duckfn.duckdb_extension.wasm` -> `duckfn` -> `duckfn_init_c_api`), so a
 * file still carrying a platform suffix cannot be loaded as-is.
 */
export function extensionBaseName(url: string): string {
  const withoutQuery = url.split(/[?#]/, 1)[0];
  const segment = withoutQuery.slice(withoutQuery.lastIndexOf('/') + 1);
  const dot = segment.indexOf('.');
  return dot === -1 ? segment : segment.slice(0, dot);
}

/**
 * Validates and normalises one preload entry, dropping unknown keys and
 * lower-casing repository keywords; throws with a readable message on
 * anything that could not be turned into a safe `LOAD` / `INSTALL` statement
 * or a static file path.
 */
export function normalizePreloadEntry(entry: unknown): PreloadEntry {
  if (typeof entry === 'string') {
    return requireExtensionName(entry, 'A preload entry');
  }
  if (typeof entry !== 'object' || entry === null || Array.isArray(entry)) {
    throw new Error(`A preload entry must be a string, {name, repository?} or {url, release?}: ${describe(entry)}`);
  }
  const record = entry as Record<string, unknown>;
  const hasName = record.name !== undefined;
  const hasUrl = record.url !== undefined;
  if (hasName === hasUrl) {
    throw new Error(
      `A preload entry needs exactly one of \`name\` (load by name) or \`url\` (load a file): ${describe(entry)}`,
    );
  }
  return hasName ? normalizeNamedEntry(record) : normalizeUrlEntry(record);
}

/** Validates the injected config envelope and normalises every preload entry. */
export function parseSiteRuntimeConfig(value: unknown): SiteRuntimeConfig {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`Expected an object: ${describe(value)}`);
  }
  const record = value as Record<string, unknown>;
  requireOnlyKeys(record, ['allowUnsignedExtensions', 'preload'], 'The config');
  const {allowUnsignedExtensions} = record;
  if (allowUnsignedExtensions !== undefined && typeof allowUnsignedExtensions !== 'boolean') {
    throw new Error(`\`allowUnsignedExtensions\` must be a boolean: ${describe(allowUnsignedExtensions)}`);
  }
  const raw = record.preload ?? [];
  if (!Array.isArray(raw)) {
    throw new Error(`\`preload\` must be an array: ${describe(raw)}`);
  }
  const preload = raw.map((entry, index) => {
    try {
      return normalizePreloadEntry(entry);
    } catch (error) {
      throw new Error(`preload[${index}]: ${errorMessage(error)}`);
    }
  });
  return allowUnsignedExtensions === undefined ? {preload} : {allowUnsignedExtensions, preload};
}

function normalizeNamedEntry(record: Record<string, unknown>): NamedPreloadEntry {
  requireOnlyKeys(record, ['name', 'repository'], 'A name preload entry');
  const name = requireExtensionName(record.name, '`name`');
  const {repository} = record;
  if (repository === undefined) {
    return {name};
  }
  if (typeof repository === 'string') {
    const keyword = repository.toLowerCase();
    if (REPOSITORY_KEYWORDS.has(keyword)) {
      return {name, repository: keyword};
    }
    if (REPOSITORY_URL_PATTERN.test(repository)) {
      return {name, repository};
    }
  }
  throw new Error(`\`repository\` must be 'community', 'core' or an http(s) URL without quotes: ${describe(repository)}`);
}

function normalizeUrlEntry(record: Record<string, unknown>): UrlPreloadEntry {
  requireOnlyKeys(record, ['url', 'release'], 'A url preload entry');
  const rawUrl = record.url;
  if (typeof rawUrl !== 'string' || !SAFE_URL_TEXT_PATTERN.test(rawUrl)) {
    throw new Error(`\`url\` must be a file path or http(s) URL without quotes, whitespace or backslashes: ${describe(rawUrl)}`);
  }
  const absolute = isAbsoluteHttpUrl(rawUrl);
  if (!absolute && URL_SCHEME_PATTERN.test(rawUrl)) {
    throw new Error(`\`url\` must be a site-relative path or an http(s) URL: ${describe(rawUrl)}`);
  }
  const url = absolute ? rawUrl : normalizeSitePath(rawUrl);
  if (!EXTENSION_NAME_PATTERN.test(extensionBaseName(url))) {
    throw new Error(
      `The last path segment of \`url\` must start with the extension name before its first dot ` +
        `(that base names the entry symbol, e.g. duckfn.duckdb_extension.wasm): ${url}`,
    );
  }
  const {release} = record;
  if (release === undefined) {
    return {url};
  }
  if (absolute) {
    throw new Error(`\`release\` copies the asset into the site's static directory, so \`url\` has to be site-relative: ${url}`);
  }
  return {url, release: normalizeReleaseSource(release)};
}

/** Strips the leading slash and rejects anything that cannot join a baseUrl safely. */
function normalizeSitePath(value: string): string {
  if (value.startsWith('//')) {
    throw new Error(`Protocol-relative URLs are not supported: ${value}`);
  }
  const path = value.replace(/^\/+/, '');
  const segments = path.split('/');
  if (path === '' || segments.some((segment) => segment === '' || segment === '.' || segment === '..')) {
    throw new Error(`\`url\` must be a plain site-relative file path: ${value}`);
  }
  return path;
}

function normalizeReleaseSource(value: unknown): PreloadReleaseSource {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`\`release\` must be {repository, asset}: ${describe(value)}`);
  }
  const record = value as Record<string, unknown>;
  requireOnlyKeys(record, ['repository', 'asset'], '`release`');
  const {repository, asset} = record;
  if (typeof repository !== 'string' || !GITHUB_REPOSITORY_PATTERN.test(repository)) {
    throw new Error(`\`release.repository\` must look like 'owner/name': ${describe(repository)}`);
  }
  if (typeof asset !== 'string' || !SAFE_URL_TEXT_PATTERN.test(asset) || asset.includes('/')) {
    throw new Error(`\`release.asset\` is the release asset's file name (no slashes): ${describe(asset)}`);
  }
  return {repository, asset};
}

function requireExtensionName(value: unknown, label: string): string {
  if (typeof value !== 'string' || !EXTENSION_NAME_PATTERN.test(value)) {
    throw new Error(`${label} must be a bare SQL identifier ([a-z][a-z0-9_]*): ${describe(value)}`);
  }
  return value;
}

function requireOnlyKeys(record: Record<string, unknown>, allowed: string[], label: string): void {
  for (const key of Object.keys(record)) {
    if (!allowed.includes(key)) {
      throw new Error(`${label} has an unknown key \`${key}\` (expected: ${allowed.join(' / ')})`);
    }
  }
}

function describe(value: unknown): string {
  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
