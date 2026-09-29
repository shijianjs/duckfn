# duckfn-docs-kit — usage guide for agents and site authors

This is the **consumer-facing** companion to [`README.md`](./README.md): the README says what to
import and how to wire it into `docusaurus.config.ts`, this file states the **contracts and the
traps** — the things that are easy to get subtly wrong when you write a docs page or review one.

Read it before writing runnable SQL blocks or configuring preloads.

> Developing the kit itself? Its internal conventions (rendering contract, shadow-DOM rules,
> release flow) are in `CONVENTIONS.md` in the repository. That file is **not** published; this one
> is, so it is what a dependency reader sees.

## The three things that most often go wrong

1. **A runnable block shows only the result of its *last* statement.** Stacking several independent
   examples in one block means the reader sees one result and the others silently vanish. One
   example per block; several statements only for a preamble (`SET`, `CREATE`, …) that the last
   query needs.
2. **`show: "html"` / `"iframe"` / `"svg"` need to be told which column holds the markup.** With more
   than one result column you **must** name it: `"field": "<column>"`. Without it the renderer has
   nothing to preview. `"tab_name"` names the column that labels each preview tab, and without it
   tabs read `Row 1`, `Row 2`, …
3. **Only a block whose info string is JSON with `"type":"duckfn"` becomes runnable.** A bare
   ```` ```sql ```` block (or any other metastring) stays a plain, un-runnable code block — no Run
   button, nothing executed.

## Install and wire it up

See [`README.md`](./README.md) — install, the three `docusaurus.config.ts` plugins, and the one CSS
import. Everything below assumes that is already done.

## Runnable SQL blocks

### The shape

A fenced block whose info string is a JSON config. The block itself becomes a CodeMirror editor
with a Run button; nothing executes until the reader clicks it.

````md
```sql {"type":"duckfn"}
SELECT 40 + 2 AS answer;
```
````

Works in `.md` and `.mdx` alike — the metastring is rewritten during the build, before either format
is compiled.

### Config reference

| Field | Meaning |
| --- | --- |
| `type` | `"duckfn"`. Required — this is what makes the block runnable. |
| `show` | `table` (default), `text`, `html`, `iframe`, `svg`. See *Result renderers*. |
| `field` | The column holding the markup, for `html` / `iframe` / `svg`. Required when the result has more than one column. |
| `tab_name` | The column whose value labels each preview tab. Defaults to `Row N`. |
| `option.width` · `option.height` | CSS lengths for the preview box (`"100%"`, `"640px"`). |
| `option.sandbox` | Sandbox tokens for the iframe, replacing the default `allow-scripts`. Widen deliberately. |
| `extensions` | Extra extension names to `LOAD` before this block runs, on top of the site's preloads. |
| `repository` | Where those extensions come from: `community`, `core`, or a repository URL. |
| `allowUnsignedExtensions` | Accept an unverifiable signature. Site-wide via the preload config, or per block; the first block to initialise the engine settles it. |
| `expect` | `ok` (default) or `error`. `error` declares "this block must fail" — see *Testing*. |

### Behaviour you have to design around

- **The result shown is the last statement's.** A block with `SET …; CREATE …; SELECT …` shows the
  `SELECT`. A block with three independent `SELECT`s shows the third one only.
- **Default `show`:** a single column with a single row renders as `text`; anything else renders as
  a `table`. Set `show` explicitly when the shape matters.
- **One DuckDB-Wasm instance per page, one connection per page.** Blocks on the same page share
  state — a table or macro created in one block is visible to the next — and pages are isolated
  from each other. Do not write a block that depends on another *page*.
- **The site's preloaded extensions are already loaded.** Call into them directly; do not add
  `extensions` for the extension the site documents.
- **Errors are a result, not a broken block.** A failing statement renders its message in the result
  area and keeps whatever the reader typed.
- **Every result has a tab strip** (even a plain table), and the fullscreen toggle lives at its right
  end. Table results bring sorting, resizable rows/columns, a right-click menu and header drag.

### Result renderers

| `show` | What it renders | Needs |
| --- | --- | --- |
| `table` | The result grid. | — |
| `text` | A bare scalar, as one line. | A single column/row result. |
| `html` / `iframe` | One tab per row; the markup goes into a sandboxed `iframe` (`srcdoc`), with a trailing `Table` tab that is always last. | `field` (unless the result has exactly one column). `tab_name` to label tabs. |
| `svg` | The markup is spliced **into the page** (one tab per row, trailing `Table` tab). | Same as above. |

Two facts worth knowing before you pick one:

- `html` and `iframe` are the *same* renderer. The frame is sandboxed with `allow-scripts` and
  **without** `allow-same-origin`, so a report's JavaScript runs while the frame keeps an opaque
  origin — that is what makes charts work, and it is also why the parent page cannot read
  `iframe.contentDocument` (it is `null` by design).
- `svg` shares the page, so anything that could execute or navigate — `script`, `foreignObject`,
  `on*` handlers, `javascript:` links — is stripped before insertion.

### Examples

A table result, and a scalar that degrades to text:

````md
```sql {"type":"duckfn","show":"table"}
SELECT * FROM range(10) WHERE range > 5;
```

```sql {"type":"duckfn"}
SELECT 1;
```
````

An HTML (or iframe) report — note `field` and `tab_name`:

````md
```sql {"type":"duckfn","show":"iframe","field":"html","tab_name":"label","option":{"height":"170px"}}
SELECT * FROM (VALUES
  ('Bars', '<!doctype html><body><h4>Quarterly revenue</h4><svg viewBox="0 0 240 80">…</svg></body>')
) AS t(label, html);
```
````

An inline SVG, and an extension that is not preloaded:

````md
```sql {"type":"duckfn","show":"svg","option":{"height":"140px"}}
SELECT '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><circle cx="50" cy="50" r="40"/></svg>';
```

```sql {"type":"duckfn","show":"table","extensions":["inet"]}
SELECT '127.0.0.1'::INET::VARCHAR AS ip;
```
````

### Mistakes to check for when reviewing a page

- Several independent examples stacked in one block — only the last result is visible. **Split
  them into one block per example.**
- `show: "html"` / `"iframe"` / `"svg"` with a multi-column result and no `field`.
- No `tab_name`, so the preview tabs read `Row 1`, `Row 2`, … instead of something meaningful.
- A bare ```` ```sql ```` block where a runnable one was intended (it renders as a plain listing).
- Declaring `extensions` for the extension the site already preloads.
- A block that depends on a table created on a *different* page.

## Preloading extensions (the `dfkExtensions` plugin)

The site declares an ordered preload list; the kit fetches the release assets at dev/build startup,
injects the list into every page and loads them — in order — while DuckDB initialises. Blocks can
then call into those extensions without declaring anything.

Three source kinds:

| Entry | Meaning |
| --- | --- |
| `'json'` | A name: `LOAD json` from the official repository. |
| `{name: 'h3', repository: 'community'}` | `community`, `core` or a repository URL. On wasm `INSTALL` only records *where* a later `LOAD` fetches from. |
| `{url: 'duckdb-extensions/x.duckdb_extension.wasm', release: {repository, asset}}` | The site serves the file itself; with `release`, the build fetches that asset from the repository's latest release (cached by sha256 in `<siteDir>/.cache/duckfn-docs-kit/`). |

Constraints that bite:

- **The file name is a contract**: the text before the first dot is the entry symbol DuckDB looks
  up, so a release asset named `duckfn-wasm_eh.duckdb_extension.wasm` must be served as
  `duckfn.duckdb_extension.wasm` (the `url` decides the file name).
- **Platforms must match**: a `wasm_eh` extension needs a runtime bundle on the `eh` platform. Pin
  `@duckdb/duckdb-wasm` to the exact version whose bundled DuckDB is ABI-compatible with the
  extension build.
- **Unsigned third-party extensions need `allowUnsignedExtensions: true`** (the WebAssembly
  equivalent of `duckdb -unsigned`). Community extensions are signed and load without it.

## Testing the blocks (`duckfn-sql-verify`)

The kit ships the same runner the browser uses, so a site can execute every block it publishes:

```bash
duckfn-sql-verify --site .            # or: npx duckfn-sql-verify --site .
```

It collects every runnable block (`docs/` plus each `i18n/<locale>/…/current/` by default), runs it
in DuckDB-Wasm with the site's extension loaded, and fails the process when a block does not behave
as it declares. Wire it into `package.json` as `"test": "duckfn-sql-verify --site ."`.

- **A block that demonstrates a failure must say so**: `{"type":"duckfn","expect":"error"}`. The
  check is two-way — a block that declares `error` and starts succeeding is reported too — and a
  `-- error:` comment in the SQL is *not* read; only the metadata counts.
- Useful options: `--extension <path|url>`, `--platform eh|mvp`, `--content <dir>` (repeatable),
  `--timeout <ms>`, `--report <file>`, `--working-dir <dir>`, `--quiet`.
- The default extension is the single file under `static/duckdb-extensions/`.

## TOC collapse control (`dfkTocToggle()`)

```ts
plugins: [dfkTocToggle()],
```

Adds a collapse button to the desktop table of contents and remembers the choice in
`localStorage` (`duckfn:toc-collapsed`). Custom labels come from the plugin's `labels` option,
keyed by a lower-cased `html-lang` prefix (`{en: {hide, show}, 'zh-hans': {…}}`).

## Version placeholder (`remarkVersionPlaceholder`)

```ts
remarkPlugins: [[remarkVersionPlaceholder, {version: DUCKFN_VERSION}]],
```

Replaces `{{DUCKFN_VERSION}}` inside `text`, `inlineCode` and `code` nodes, so a release updates one
file instead of every page. It only touches that exact placeholder — anything else is left alone.

## Home-page components

`<dfk-hero>`, `<dfk-features>`, `<dfk-next-steps>` (and `<dfk-sql>`) are registered by the barrel
import (`duckfn-docs-kit`). The home components are **setter-driven and do not reflect attributes**:
give them named setters (`setTitle`, `setTagline`, `setFeatures`, …), not markup content. They
render into shadow roots, inherit `--duckfn-*` / `--ifm-*` CSS variables from the page, and are
safe to call before the element is connected.

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| A code block has no Run button | Its info string is not JSON with `"type":"duckfn"`. |
| The block runs but nothing appears in the preview | `show: "html"` / `"iframe"` / `"svg"` without `field` on a multi-column result. |
| The preview tabs are labelled `Row 1`, `Row 2`, … | No `tab_name`. |
| Clicking Run shows an error like `Table with name … does not exist` | The block depends on something created on another page, or on a statement that is no longer the last one in its block. |
| `LOAD` fails with a signature error | `allowUnsignedExtensions: true` missing for a third-party asset. |
| A preloaded extension fails to load | The served file name's pre-dot part does not match the extension's entry symbol, or the platform does not match the runtime bundle. |
| Only the last of several examples shows a result | That is the contract — split the block. |
