/**
 * `duckfn-docs-kit/sql/placeholders` — the placeholders a runnable SQL block
 * may use, expanded in the browser right before the SQL reaches DuckDB.
 *
 * Why a placeholder at all: DuckDB-Wasm runs in a Worker whose base URL is a
 * blob:, so it resolves **nothing** relative to the page — `read_csv_auto
 * ('data/x.tsv')` and even the root-relative `'/data/x.tsv'` are looked up in
 * the instance's in-memory filesystem and come back as
 * `IO Error: No files found that match the pattern`. Only an absolute
 * `http(s)` URL reaches the HTTP filesystem. The origin, however, differs per
 * deployment (GitHub Pages, `docusaurus serve`, the verifier's random loopback
 * port), and no build-time substitution can know it — so the block names the
 * one part it cannot know and the runtime fills it in from
 * `window.location.origin`.
 *
 * The same expansion runs for the site (a reader's **Run** click) and for the
 * offline verifier (`sql/harness`), because both go through
 * {@link DuckDBRuntime.execute} — one chokepoint, so a block verified in CI is
 * the block the site runs.
 *
 * Substituted as plain text (no parsing, no escaping): the value is an origin
 * the browser itself reports, and the token is distinctive enough that a
 * collision with real SQL is not a concern.
 */

/**
 * The page origin, without a trailing slash: `'https://example.github.io'`.
 *
 * The `DFK_` prefix is deliberate: the token is expanded by plain text
 * substitution, so a short or generic one (`{{ORIGIN}}`, `{{URL}}`) could
 * collide with content a block legitimately contains. Namespaced, it cannot.
 *
 * Write a block's file URLs as `'{{DFK_ORIGIN}}<baseUrl>/data/x.tsv'` — the
 * `baseUrl` part stays literal, because the site knows it at build time (and
 * the verifier is configured with the same prefix as an asset mount).
 */
export const ORIGIN_PLACEHOLDER = '{{DFK_ORIGIN}}';

/** Replaces every {@link ORIGIN_PLACEHOLDER} in `sql` with `origin`. */
export function expandSqlPlaceholders(sql: string, origin: string): string {
  return sql.split(ORIGIN_PLACEHOLDER).join(origin.replace(/\/+$/, ''));
}

/** {@link expandSqlPlaceholders} against the current page; `''` outside a browser. */
export function expandSqlPlaceholdersHere(sql: string): string {
  const origin = typeof window === 'undefined' ? '' : window.location.origin;
  return expandSqlPlaceholders(sql, origin);
}