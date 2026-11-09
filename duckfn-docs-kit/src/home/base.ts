import {DFK_SQL_RUNTIME_TAG_ID} from '../sql/runtimeConfig';

/**
 * `href` / `src` resolution for the home components.
 *
 * A tag in a Markdown page writes a site-root-relative path — `href="/docs/intro"`
 * — the same way a Docusaurus link does. That path is not run through the
 * theme's `<Link>`, so nothing adds the deploy prefix: on GitHub Pages the site
 * lives at `/duckfn/`, and on a localised page at `/duckfn/zh-Hans/`. Both the
 * href and any `{{DFK_BASE_URL}}` in a block resolve to the same value, which a
 * site already publishes per page for the SQL runtime (`dfk-sql-runtime`, see
 * `sql/extensions`): its `baseUrl` is *this page's* prefix, locale included.
 *
 * Paths that are already absolute, protocol-relative or a bare fragment are
 * left alone, and so is a path that already starts with the prefix — a React
 * page passes `useBaseUrl()` output, which must not be prefixed twice.
 */

let cached: string | undefined;

/** The page's base URL (`/`, `/my-site/`, `/my-site/zh-Hans/`). */
function pageBaseUrl(): string {
  if (cached !== undefined) {
    return cached;
  }
  cached = '/';
  const tag = document.getElementById(DFK_SQL_RUNTIME_TAG_ID);
  if (tag) {
    try {
      const value = (JSON.parse(tag.textContent ?? '') as {baseUrl?: unknown}).baseUrl;
      if (typeof value === 'string' && value.startsWith('/')) {
        cached = value;
      }
    } catch {
      // A malformed config is the SQL runtime's problem to report, not the
      // home page's: the links fall back to the site root.
    }
  }
  return cached;
}

/** Prefixes a site-root-relative path with the page's baseUrl. */
export function resolveHref(href: string): string {
  if (!href.startsWith('/') || href.startsWith('//')) {
    return href;
  }
  const base = pageBaseUrl();
  if (base === '/' || href.startsWith(base)) {
    return href;
  }
  return `${base.replace(/\/+$/, '')}${href}`;
}
