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
2. **`show: "html"` / `"iframe"` / `"svg"` / `"mermaid"` need to be told which column holds the
   markup.** With more than one result column you **must** name it: `"field": "<column>"`. Without
   it the renderer has nothing to preview. `"tab_name"` names the column that labels each preview
   tab, and without it tabs read `Row 1`, `Row 2`, …
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
| `show` | `table` (default), `text`, `html`, `iframe`, `svg`, `mermaid`. See *Result renderers*. |
| `field` | The column holding the markup, for `html` / `iframe` / `svg` / `mermaid`. Required when the result has more than one column. |
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
- **Every result has a tab strip** (even a plain table), and all of its result-wide chrome sits at the
  strip's right end — see *The strip at the right of the tabs*. Table results bring sorting, resizable
  rows/columns, a right-click menu and header drag.

### Result renderers

| `show` | What it renders | Needs |
| --- | --- | --- |
| `table` | The result grid. | — |
| `text` | A bare scalar, as one line. | A single column/row result. |
| `html` / `iframe` | One tab per row; the markup goes into a sandboxed `iframe` (`srcdoc`), with a trailing `Table` tab that is always last. | `field` (unless the result has exactly one column). `tab_name` to label tabs. |
| `svg` | The markup is spliced **into the page** (one tab per row, trailing `Table` tab). Zoom and pan open in fullscreen, where the tab strip also offers **reset zoom** / **edit source**. | Same as above. |
| `mermaid` | The cell is handed to `<dfk-mermaid>`, which renders in its **embedded** mode: no frame of its own and no floating cluster — **reset zoom** and **edit source** join the result's tab strip instead. One tab per row, trailing `Table` tab. | Same as above. |

Two facts worth knowing before you pick one:

- `html` and `iframe` are the *same* renderer. The frame is sandboxed with `allow-scripts` and
  **without** `allow-same-origin`, so a report's JavaScript runs while the frame keeps an opaque
  origin — that is what makes charts work, and it is also why the parent page cannot read
  `iframe.contentDocument` (it is `null` by design).
- `svg` shares the page, so anything that could execute or navigate — `script`, `foreignObject`,
  `on*` handlers, `javascript:` links — is stripped before insertion.

### The strip at the right of the tabs

Every result — a plain table included — carries the same tab strip, and the right end of it is where
the result-wide controls live, in this order: **the active tab's own buttons**, **download**, then
the **fullscreen** toggle.

| Active tab | Controls |
| --- | --- |
| `Table` | Search, copy table, column-width mode, reset view, unfreeze columns |
| `svg` / `mermaid` | Reset zoom, edit source |
| `html` / `iframe` / `text` | — |

- Those table buttons are the "whole table" half of the grid's right-click menu, placed where they
  cannot cover a cell; the menu itself keeps the per-cell items (copy this cell, wrap this
  row/column, freeze up to this column). Freezing, column widths and the current view therefore
  survive a switch to another tab and back. **Unfreeze columns** is hidden until a column has
  actually been frozen, so it does not sit there as a dead control.
- **Search** opens as an input in the strip rather than floating over the cells it searches, because
  the table spans the full width. It highlights every hit, shows `3/12`, and steps with the arrows;
  it is per result, not shared between blocks.
- **Download** saves whatever the active tab shows, in that tab's format: `.csv` for a table, `.svg`
  for `svg` / `mermaid`, `.html` for `html` / `iframe`, `.txt` for `text`. It is hidden only while
  there is nothing to save yet (a figure that has not rendered).
- **Fullscreen** makes the result fill the viewport; the same button (now "Exit fullscreen") stays
  put, and <kbd>Esc</kbd> works too. It is also the only place a figure zooms or pans.

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

A diagram built by a query (the same element a ```` ```mermaid ```` fence produces):

````md
```sql {"type":"duckfn","show":"mermaid"}
SELECT 'flowchart LR' || chr(10)
  || '  A["a SELECT"] --> B["one result cell"]' AS diagram;
```
````

### Mistakes to check for when reviewing a page

- Several independent examples stacked in one block — only the last result is visible. **Split
  them into one block per example.**
- `show: "html"` / `"iframe"` / `"svg"` / `"mermaid"` with a multi-column result and no `field`.
- No `tab_name`, so the preview tabs read `Row 1`, `Row 2`, … instead of something meaningful.
- A bare ```` ```sql ```` block where a runnable one was intended (it renders as a plain listing).
- Declaring `extensions` for the extension the site already preloads.
- A block that depends on a table created on a *different* page.

## Mermaid diagrams (`remarkMermaid`)

````ts
// docusaurus.config.ts, in the docs preset's `remarkPlugins`
remarkPlugins: [remarkMermaid],
````

A ```` ```mermaid ```` fence becomes a `<dfk-mermaid>` element that renders the diagram **in the
browser** (mermaid is a lazy `import()`, so a page with no diagram never downloads it). Do **not**
also install `@docusaurus/theme-mermaid` or list it in `themes`, and do not set `markdown.mermaid`:
the kit's element replaces both, and two renderers on one page would fight.

What the reader gets, in the element's top-right corner on hover: **reset zoom**, **fullscreen**,
**edit the source** in a CodeMirror dialog, and **download SVG**. Zoom and pan are off until the
diagram is expanded — fullscreen is what turns the wheel into a zoom and a drag into a pan.

- **The file is named after the section it sits in.** `1. Expansion.svg`, not
  `mermaid-diagram.svg`, in a cascade that walks from the most specific source to the most general:
  the diagram's own title (mermaid frontmatter, `---\ntitle: …\n---`) → the nearest heading above
  it → the document title → `mermaid-diagram.svg`. So a diagram is best named by the heading it
  lives under; give it a frontmatter `title:` when the heading is not the name you want. The name
  goes through `filenamify`, so nothing a filesystem chokes on (`:`, `?`, `*`, `|`, …) reaches the
  file.
- **The diagram on the page is a picture, not a viewport.** The cursor is the browser's (an I-beam
  over a label), the wheel scrolls the page, and dragging selects text — labels are still text, and
  copying one works. Expanding the diagram is what turns the pointer into a `grab` hand and gives a
  drag to panning; **Reset zoom** returns it to fit without leaving fullscreen, and zooming is off
  again the moment the diagram is back inline. There is no select/drag mode to remember.
- **A `show: "mermaid"` result reuses the same element**, in an *embedded* mode: the result panel
  already draws the frame and the strip, so the element renders neither a frame nor a floating
  cluster, and **reset zoom** / **edit source** move into the tab strip. See *The strip at the right
  of the tabs*.

- **The palette is a site choice, not a page one.** The kit's default is the `neo` look with
  `redux-color` / `redux-dark-color`; a site overrides it in its own config, which also keeps the
  kit fork-free for downstream docs sites:

  ````ts
  remarkMermaid({config: {theme: {light: 'neutral', dark: 'dark'}, options: {look: 'classic'}}})
  ````

  `theme` is per colour mode (the element re-renders on a theme switch); `look` has no light/dark
  counterpart and lives in `options`. Mermaid silently ignores a value it does not recognise, so
  check a diagram in a browser — a build proves nothing here.
- **Diagrams render per page, in the page's own colour mode** — read from `<html data-theme>` (the
  attribute written before first paint), not from a framework hook. That is what keeps a dark-mode
  first load from painting a light diagram and then a dark one.
- **Labels**: keep them quoted (`A["text"]`), use `<br/>` for a line break, and avoid a bare `#` or
  an unescaped `&`. A syntax error shows up in the page, not in the build.
- A ```` ```mermaid ```` fence inside a longer fence (documenting it, as here) is *not* turned into
  a diagram — it is text inside the outer code block.

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

The kit runs every block in a **real browser**, on the same runtime the page uses, so what CI
checks is what a reader gets:

```bash
duckfn-sql-verify --site .            # or: npx duckfn-sql-verify --site .
```

It collects every runnable block (`docs/` plus each `i18n/<locale>/…/current/` by default), runs it
in DuckDB-Wasm **in a headless browser** (driven with `playwright-core`, launching your system
Chrome/Edge via `executablePath` — so no browser download) with the site's extension loaded, and
fails the process when a block does not behave as it declares. It is fully offline: the engine and
the extension are served from local files. Wire it into `package.json` as
`"test": "duckfn-sql-verify --site ."`.

- **A block that demonstrates a failure must say so**: `{"type":"duckfn","expect":"error"}`. The
  check is two-way — a block that declares `error` and starts succeeding is reported too — and a
  `-- error:` comment in the SQL is *not* read; only the metadata counts.
- Useful options: `--extension <path|url>`, `--platform eh|mvp`, `--content <dir>` (repeatable),
  `--browser <path>` (the Chrome/Edge executable; otherwise a detected one or `DFK_BROWSER`),
  `--timeout <ms>`, `--report <file>`, `--quiet`. It needs `playwright-core`, which the kit lists
  as a dependency; unlike `playwright` it never downloads a browser.
- The default extension is the single file under `static/duckdb-extensions/`.
- **File-system examples stay plain code blocks.** Under DuckDB-Wasm the raw file system is not
  POSIX-faithful: `dfn_file_exists` reads a missing file as "opened" (always `true`), writes/append
  byte order is wrong, and there is no existence primitive in DuckDB's C API to correct it. So keep
  any `COPY … TO` / `output_dir` / file-write demo as a non-runnable block and note the reason.

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

`<dfk-hero>`, `<dfk-features>`, `<dfk-next-steps>` (along with `<dfk-sql>` and `<dfk-mermaid>`,
which the plugins generate) are registered by the barrel import (`duckfn-docs-kit`). The home
components are **setter-driven and do not reflect attributes**:
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
| A ```` ```mermaid ```` fence renders as a plain code block | `remarkMermaid` is not in the docs preset's `remarkPlugins`; and if `@docusaurus/theme-mermaid` is still installed, remove it and `markdown.mermaid`. |
| A diagram is blank, or shows a message instead | The mermaid source does not parse — the message carries mermaid's own error text. |
