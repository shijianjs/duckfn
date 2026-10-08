/**
 * `duckfn-docs-kit/sql/placeholders` — the placeholders a runnable SQL block
 * may use, expanded in the browser just before the SQL reaches DuckDB (and once
 * when the editor is filled in, so the reader sees the resolved URL rather than
 * the token).
 *
 * Why a placeholder at all: DuckDB-Wasm runs in a Worker whose base URL is a
 * blob:, so it resolves **nothing** relative to the page — `read_csv_auto
 * ('data/x.tsv')` and even the root-relative `'/data/x.tsv'` are looked up in
 * the instance's in-memory filesystem and come back as
 * `IO Error: No files found that match the pattern`. Only an absolute
 * `http(s)` URL reaches the HTTP filesystem, and no build-time substitution can
 * know the deployed prefix.
 *
 * Two tokens, because there are two things a block cannot know:
 *
 * - `{{DFK_ORIGIN}}` → `https://example.github.io` — the page's origin, from
 *   `window.location.origin`.
 * - `{{DFK_BASE_URL}}` → `https://example.github.io/my-site/zh-Hans/` — the
 *   origin plus **this page's** baseUrl. Blocks normally want this one: Docusaurus
 *   serves `static/` under each locale's baseUrl, so the same `data/x.tsv` is
 *   `/my-site/data/x.tsv` on the English pages and `/my-site/zh-Hans/data/x.tsv`
 *   on the Chinese ones. Writing it out by hand is exactly the bug this token
 *   removes.
 *
 * The `DFK_` prefix is deliberate: substitution is plain text, so a short or
 * generic token (`{{ORIGIN}}`, `{{URL}}`) could collide with content a block
 * legitimately contains. Namespaced, it cannot.
 *
 * The same expansion runs for the site (a reader's **Run** click) and for the
 * offline verifier (`sql/harness`), because both go through
 * {@link DuckDBRuntime.execute} — one chokepoint, so a block verified in CI is
 * the block the site runs.
 */

/**
 * The page origin, without a trailing slash: `'https://example.github.io'`.
 *
 * Use it for URLs that do *not* live under the site's baseUrl (an external
 * dataset, another host's asset). For the site's own files use
 * {@link BASE_URL_PLACEHOLDER}.
 */
export const ORIGIN_PLACEHOLDER = '{{DFK_ORIGIN}}';

/**
 * The page's base URL as an absolute prefix, always ending in `/`:
 * `'https://example.github.io/my-site/zh-Hans/'`.
 *
 * Write a block's own-file URLs as `'{{DFK_BASE_URL}}data/x.tsv'` — no slash of
 * its own, since the token supplies the trailing one.
 */
export const BASE_URL_PLACEHOLDER = '{{DFK_BASE_URL}}';

/**
 * Replaces every placeholder in `sql`. `baseUrl` may be empty (a site without a
 * baseUrl, or a caller that does not know it), which leaves
 * `{{DFK_BASE_URL}}` as origin + `/` — a working URL, and an obvious one to spot
 * in a failure message.
 */
export function expandSqlPlaceholders(sql: string, origin: string, baseUrl: string): string {
  const bare = origin.replace(/\/+$/, '');
  const base = baseUrl.replace(/^\/+|\/+$/g, '');
  // Always a trailing slash: a block writes `'{{DFK_BASE_URL}}data/x.tsv'` and
  // relies on the token for the separator.
  const absoluteBase = base === '' ? `${bare}/` : `${bare}/${base}/`;
  return sql.split(ORIGIN_PLACEHOLDER).join(bare).split(BASE_URL_PLACEHOLDER).join(absoluteBase);
}