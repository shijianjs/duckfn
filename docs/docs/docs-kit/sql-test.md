---
title: Testing the examples
sidebar_position: 4
description: Run every runnable SQL block of a docs site as Playwright tests — the kit ships the collector, the DuckDB-Wasm fixtures and the config preset.
---

# Testing the examples

Runnable SQL blocks are executable documentation, so they rot the way tests do:
an extension function gets renamed, a default changes, and the block a reader
clicks **Run** on is the only place that notices. The kit ships a **Playwright
Test** integration that runs every block in the same browser runtime the site
uses, so a docs project can check them in CI — with the framework's reports, trace
viewer and editor integration:

```ts
// playwright.config.mts
import {defineDuckfnDocsConfig} from 'duckfn-docs-kit/sql/playwright';
export default defineDuckfnDocsConfig();
```

```ts
// tests/docs.spec.mts
import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';
declareDocsTests();
```

Then run `npx playwright test` (this site wires it up as its `npm test`, see
`docs/package.json`). Use `.mts` unless your project is `"type": "module"`: the
kit ships ESM, and a CommonJS project would otherwise make Playwright transpile
the imports to `require()` and fail on the kit's `import.meta`.

It is deliberately **not** part of CI in this repository: the test needs the wasm
build of the extension, and that artefact only comes out of duckfn's own CI (nine
platform artifacts, around 20 minutes). The repository's `AGENTS.md` records that
trade-off.

## What it does

1. **Collect** — walk the content directories and take every fenced block whose
   info string is a runnable config (`sql/collect`). The metastring contract is
   the one `sql/remark` implements for the build, so the list is exactly what
   the site publishes — a block that is not runnable there is not collected
   here.
2. **Run** — `declareDocsTests()` registers one Playwright test per block,
   grouped by content file. Each file gets its own page, and the blocks of a page
   share its DuckDB-Wasm connection (one instance per page, the blocks of a page
   sharing one connection), so a `CREATE` in one block is visible to the next —
   and pages stay isolated. A file runs serially, different files in parallel
   workers. The page itself (`sql/harness`) reuses the very `sql/runtime` the
   site runs.
3. **Report** — Playwright's own reporters (`list`, `html` with traces, `junit`,
   `github`); the VS Code test tree shows each block and lets you run or debug
   one.

## Blocks that fail on purpose

Half the guide ends on a statement that demonstrates an error, and such a block
has to declare it — `test.fail()` checks both directions, so a block that stops
failing is reported just like one that starts failing:

```sql {"type":"duckfn","expect":"error"}
SELECT CAST('abc' AS INTEGER);  -- error: not an integer: "abc"
```

`"expect"` defaults to `"ok"`; `"error"` means *this block must fail*. It is the
only thing the suite reads: the prose around a block, and a `-- error: …` comment
inside it, are there for readers and are not machine-checked.

## Options

Configuration comes from `sql/site`: explicit options first, then `DFK_*`
environment variables, then the detected site layout.

| Variable | Meaning |
| --- | --- |
| `DFK_SITE_DIR` | Docs site root (default: the working directory). |
| `DFK_CONTENT` | Comma-separated content directories relative to the site root. Defaults to `docs/` plus every `i18n/<locale>/docusaurus-plugin-content-docs/current/`. |
| `DFK_EXTENSION` | The extension to preload: a `.duckdb_extension.wasm` path, or an absolute `http(s)` URL. Defaults to the single file under `static/duckdb-extensions/`. |
| `DFK_ASSETS` | Comma-separated `url=dir` pairs: local directories served over HTTP while the blocks run (see the next section). |
| `DFK_PLATFORM` | DuckDB-Wasm bundle, which has to match the extension build (default: `eh`). |
| `DFK_ENGINE` | Engine wasm override, for pinning a specific DuckDB-Wasm build. |
| `DFK_BROWSER` | The Chrome/Edge executable to drive. Defaults to a detected system Chrome/Edge. Playwright launches it directly — no browser download. |
| `DFK_TIMEOUT` | Per-block timeout in milliseconds (default: 30000) — a hang is reported as a failure instead of blocking CI. |

`defineDuckfnDocsConfig()` also accepts these as options (`{siteDir, contentDirs,
…}`) plus a `config` field merged last, so anything the preset sets can be
overridden from your own `playwright.config.mts`.

## Reading the site's own data files (asset mounts)

The harness serves the page, the engine, the worker and the extension from a
short-lived loopback server — nothing else. A block that reads a data file the
site itself ships (a `.tsv` under `static/`, say) would therefore 404. Declare
each directory the blocks read, and the server serves it too:

```ts
// tests/docs.spec.mts
import {fileURLToPath} from 'node:url';
import {declareDocsTests} from 'duckfn-docs-kit/sql/playwright';

declareDocsTests({
  siteDir: fileURLToPath(new URL('..', import.meta.url)),
  baseUrl: '/my-site/',
  assets: [{url: '/my-site/data', dir: 'static/data'}],
});
```

- `url` is the **root-relative** prefix the files become reachable at: the
  site's `baseUrl` plus the directory name — the same path the deployed site
  serves. It has to be a prefix the page URL can be put in front of, because
  DuckDB-Wasm reads nothing relative (see the placeholders below):

  ```sql
  SELECT count(*) AS n FROM read_csv_auto('{{DFK_BASE_URL}}data/samples.tsv');
  ```

- `dir` is relative to the site root (`siteDir`), which is why `siteDir` is
  still needed alongside `assets`.
- `baseUrl` is what `{{DFK_BASE_URL}}` expands to and has to agree with those
  prefixes. The harness has no locale, so one prefix serves every locale's blocks.
- Paths are confined to `dir`: a `..` in the URL never reaches outside it, and
  only regular files are served.

The Playwright options, the CLI flags and the environment variables are the same
thing: `--asset /my-site/data=static/data` (repeatable) plus `--base-url /my-site/`
on `duckfn-sql-verify`, or `DFK_ASSETS=/my-site/data=static/data` and
`DFK_BASE_URL=/my-site/`.

## `{{DFK_BASE_URL}}`: why a data URL needs a placeholder

DuckDB-Wasm runs in a Worker whose base URL is a `blob:`, so it resolves
**nothing** relative to the page. Both of these fail, with the same
`IO Error: No files found that match the pattern`:

```sql
SELECT * FROM read_csv_auto('data/samples.tsv');    -- looked up in memory
SELECT * FROM read_csv_auto('/data/samples.tsv');   -- ditto: a path, not a URL
```

Only an absolute `http(s)` URL reaches the HTTP filesystem, and no build-time
substitution can know the deployed prefix (GitHub Pages, `docusaurus serve` and
the harness' random loopback port all differ). So there are two tokens, and
`sql/runtime` expands them in `execute()`, immediately before the SQL reaches
DuckDB:

| Token | Expands to | Use it for |
| --- | --- | --- |
| `{{DFK_ORIGIN}}` | `https://example.github.io` | URLs that do *not* live under the site's baseUrl (an external dataset). |
| `{{DFK_BASE_URL}}` | `https://example.github.io/my-site/` — the origin plus **this page's** baseUrl | The site's own files, i.e. everything under `static/`. |

Prefer `{{DFK_BASE_URL}}` for the site's own files, and **never hard-code the
prefix in a block**: Docusaurus copies `static/` into *every locale's* output, so
`data/samples.tsv` is `/my-site/data/samples.tsv` on the English pages and
`/my-site/zh-Hans/data/samples.tsv` on the Chinese ones — a block that writes the
prefix out works in one locale and 404s in the other. The baseUrl reaches the
browser through the config tag `dfkExtensions` injects (localised per locale), and
the harness through `baseUrl` / `DFK_BASE_URL` / `--base-url`.

The site and the harness share the one `execute()` chokepoint, so a block verified
in CI is byte for byte the block a reader runs. `<dfk-sql>` resolves the same
tokens when it fills the editor, so a reader sees — and can copy — the URL that
will actually be fetched, never a token.

## The command-line fallback

For a framework-free run the kit still ships `duckfn-sql-verify`, which shares
the collector, the harness and the configuration resolution with the Playwright
path:

```bash
duckfn-sql-verify --site . --asset /my-site/data=static/data
```

It needs `playwright-core` (a kit dependency) and exits non-zero when a block
does not behave as it declares. Prefer Playwright Test when you want reports and
IDE support.

## Why it runs the way it does

A block is executed in **DuckDB-Wasm inside a real browser** — the environment a
reader gets — not in Node. This is deliberate: the old Node-worker target could
not read remote `http(s)` data (every remote-data example failed with an
`IO Error`), so blocks that read real files could only ever be checked by hand
in a browser. Running the suite in a browser makes that the normal case.

The browser is driven with **Playwright**, which owns the protocol, navigation,
auto-waiting, timeouts and crash handling rather than a bespoke driver. It
launches your **system Chrome/Edge** directly (`launchOptions.executablePath`),
so no browser is downloaded at run time. Everything else is served locally, so it
runs offline: the engine (`duckdb-*.wasm` and its worker script) from
`node_modules`, the extension from `static/duckdb-extensions/`, over a loopback
http server the harness page fetches them from. Loading the extension over http
works exactly like it does on the site, with none of the port/staging-directory
constraints the Node worker imposed.

```sql
-- A remote read like this is what the Node worker could never do; a browser can.
-- (Not a runnable block here: fetching it needs the network, and the suite runs offline.)
SELECT count(*) AS n FROM read_csv_auto('https://example.com/data/smallest.csv');
```

### File-system examples are the exception

The browser's raw file system is not a faithful POSIX layer: DuckDB-Wasm opens a
file that was never written and hands back zero-filled bytes, so `dfn_file_exists`
reports `true` for things that are absent, and `COPY … TO` / `append` do not land
the bytes you would expect. DuckDB's C API exposes no existence primitive an
extension could use to correct this, so it is a platform limit rather than a
bug — and examples that depend on the file system stay **plain (non-runnable)
code blocks**, with the reason noted on the page. The native build (and the
native test suite `test/sql/functions/duck_vfs.test`) does behave correctly.