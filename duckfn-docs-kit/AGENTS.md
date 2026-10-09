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
with a Run button; nothing executes until the reader clicks it. A second button, **Run all**, runs
every runnable block on the page in document order, one at a time — it is there for pages that stack
a dozen figure-drawing blocks.

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
| `show` | `table` (default), `text`, `html`, `iframe`, `svg`, `mermaid`, `terminal`. See *Result renderers*. |
| `field` | The column holding the markup, for `html` / `iframe` / `svg` / `mermaid` / `terminal`. Required when the result has more than one column. |
| `tab_name` | The column whose value labels each preview tab. Defaults to `Row N`. |
| `option.width` · `option.height` | CSS lengths for the preview box (`"100%"`, `"640px"`). |
| `option.code_max_height` | A ceiling for the **code editor** (`"16rem"`, `"40vh"`): past it the block stops growing and the code area scrolls inside itself. What keeps the result in reach when an example runs to a hundred lines. |
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
| `terminal` | The cell the way a terminal would draw it: SGR colours and decorations, box-drawing and braille characters, monospace, dark, **never wrapped**. For what a CLI *would have printed*, which `text` can only show flat. One tab per row, trailing `Table` tab, and no per-tab buttons (there is nothing to zoom). | Same as above. |

Two facts worth knowing before you pick one:

- `html` and `iframe` are the *same* renderer. The frame is sandboxed with `allow-scripts` and
  **without** `allow-same-origin`, so a report's JavaScript runs while the frame keeps an opaque
  origin — that is what makes charts work, and it is also why the parent page cannot read
  `iframe.contentDocument` (it is `null` by design).
- `svg` shares the page, so anything that could execute or navigate — `script`, `foreignObject`,
  `on*` handlers, `javascript:` links — is stripped before insertion. A figure that is wider than
  the result panel is **scaled down to fit the panel's width**, with the panel's height following
  its aspect ratio (and a figure that already fits keeps its natural size — nothing is upscaled).
  Markup whose root declares only a `width`/`height` and no `viewBox` — what most plotting
  libraries emit — is given `viewBox="0 0 <width> <height>"` on the way in, because without a
  `viewBox` an SVG does not scale at all: it would keep its own size and be cut off at the panel's
  edge. Only a pixel size can be turned into a coordinate system; `width="100%"` is left alone.
- `terminal` is parsed with [`anser`](https://www.npmjs.com/package/anser), which returns **styled
  runs** rather than an HTML string — that is what lets the runs become text nodes and `<span>`s
  instead of markup pasted in. Everything escape-shaped is stripped on the way: terminal string
  sequences (OSC titles, DCS payloads) before parsing, any leftover CSI and every remaining control
  character after it, so a label carrying an escape byte cannot put visible junk — or markup — on the
  page. `\n` and `\t` survive (CSS gives the tab its stop). The colours that come through are the
  basic and bright palettes, 256-colour and 24-bit, plus bold / dim / italic / underline /
  strikethrough / hidden; **reverse video arrives already swapped**. The surface is dark in both site
  themes (a frame is a picture of a terminal, not a part of the page) and `--duckfn-terminal-bg` /
  `--duckfn-terminal-fg` are the two values to override. **Download** saves the raw text with its
  escapes, ready to be piped back into a terminal.

### The strip at the right of the tabs

Every result — a plain table included — carries the same tab strip, and the right end of it is where
the result-wide controls live, in this order: **the active tab's own buttons**, **download**, then
the **fullscreen** toggle.

| Active tab | Controls |
| --- | --- |
| `Table` | Search, copy table, column-width mode, reset view, unfreeze columns |
| `svg` / `mermaid` | Reset zoom (fullscreen only), edit source |
| `html` / `iframe` / `text` / `terminal` | — |

- Those table buttons are the "whole table" half of the grid's right-click menu, placed where they
  cannot cover a cell; the menu itself keeps the per-cell items (copy this cell, wrap this
  row/column, freeze up to this column). Freezing, column widths and the current view therefore
  survive a switch to another tab and back. **Unfreeze columns** is hidden until a column has
  actually been frozen, so it does not sit there as a dead control.
- **Reset zoom** is hidden the same way while the result is inline: a figure only zooms in
  fullscreen, so inline there is nothing to reset and the button would be dead. Expand the result and
  it appears next to **Edit source**.
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
- `show: "terminal"` on a cell that carries no escape sequences: it renders, but as flat monospace
  text. Check that the query really returns the sequences — in DuckDB that is `chr(27)`, as the
  example in the docs' terminal section does.
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

What the reader gets, in the element's top-right corner on hover: **reset zoom** (once the diagram
can actually be zoomed, i.e. from the moment it is expanded), **fullscreen**, **edit the source** in
a CodeMirror dialog, and **download SVG**. Expanding is what turns the diagram into a viewport: the
pointer becomes a grab hand, a drag pans without zooming in first, and the wheel zooms both ways —
including below fit, so a figure that fills the screen can still be shrunk.

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
  drag to panning, at which point selecting text inside the diagram is deliberately off: a drag that
  panned anyway would silently steal the selection, and a mode that tried to keep both made the
  gesture ambiguous. **Reset zoom** returns it to fit without leaving fullscreen, and zooming is off
  again the moment the diagram is back inline — where the labels are selectable text once more.
  There is no select/drag mode to remember.
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

## Testing the blocks (Playwright Test)

The kit runs every block in a **real browser**, on the same runtime the page uses, so what CI
checks is what a reader gets — and it runs them as **Playwright tests**, so a failure arrives with
the framework's report, trace viewer and editor integration. Wire it up in two files:

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

then run `npx playwright test` (or `"test": "playwright test"` in `package.json`). Use the `.mts`
extension unless your project is `"type": "module"`: the kit ships ESM, and a CommonJS project would
otherwise make Playwright transpile the imports to `require()` and choke on the kit's `import.meta`.

- **One test per block**, one page per content file: blocks on a page share its DuckDB connection, so
  a block may rely on a table or macro an earlier block on the **same page** created — pages stay
  isolated. A file runs serially, different files in parallel workers.
- **A block that demonstrates a failure must say so**: `{"type":"duckfn","expect":"error"}`. The
  check is two-way (`test.fail()` reports a block that declares `error` and starts succeeding too),
  and a `-- error:` comment in the SQL is *not* read; only the metadata counts.
- **Configuration** comes from the `DFK_*` environment variables — `DFK_SITE_DIR`, `DFK_CONTENT`,
  `DFK_EXTENSION`, `DFK_ASSETS`, `DFK_PLATFORM`, `DFK_ENGINE`, `DFK_BROWSER`, `DFK_TIMEOUT`; unset,
  the site layout is detected. The preset launches your **system Chrome/Edge** (detected, or
  `DFK_BROWSER`), so no browser is downloaded at run time. `@playwright/test` is an optional peer
  dependency; if you do not want Playwright's install-time download either, install it with
  `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1`.
- **A block that reads the site's own data files needs an asset mount, and an absolute URL.** The test
  server is otherwise closed (`harness.html` / `harness.js` / `/vendor/*` / `/ext/*` only), so a
  `read_csv_auto('{{DFK_BASE_URL}}data/x.tsv')` would 404. Declare the directories the blocks read and
  the base URL they sit under:

  ```ts
  // tests/docs.spec.mts
  declareDocsTests({
    siteDir: fileURLToPath(new URL('..', import.meta.url)),
    baseUrl: '/my-site/',
    assets: [{url: '/my-site/data', dir: 'static/data'}],
  });
  ```

  `url` is the **root-relative** prefix the files are reachable at — the site's `baseUrl` plus the
  directory name, exactly the path the deployed site serves. `dir` is relative to the site root. `baseUrl`
  is what `{{DFK_BASE_URL}}` expands to and must agree with those prefixes; it exists because the harness
  has no locale, so one prefix has to serve every locale's blocks. The CLI takes the same thing as
  `--asset /my-site/data=static/data` (repeatable) plus `--base-url /my-site/`, and `DFK_ASSETS` /
  `DFK_BASE_URL` as the environment equivalents. `siteDir` is still needed alongside `assets` unless it is
  already detected or set in the environment, because `dir` is resolved against it.
- **Two placeholders, resolved at run time** (`sql/placeholders`): `{{DFK_ORIGIN}}` → the page origin,
  `{{DFK_BASE_URL}}` → the origin plus **this page's** baseUrl. DuckDB-Wasm runs in a Worker based at a
  `blob:` URL and resolves nothing relative to the page: `'data/x.tsv'` *and* `'/data/x.tsv'` are both
  looked up in the instance's in-memory filesystem and fail with `IO Error: No files found that match the
  pattern`. Only an absolute `http(s)` URL reaches HTTPFS, and no build-time substitution knows the origin
  — so `DuckDBRuntime.execute()` expands both tokens right before the SQL reaches DuckDB, one chokepoint
  shared by the site and the harness.
- **Prefer `{{DFK_BASE_URL}}` for the site's own files, and never hard-code the baseUrl in a block.**
  Docusaurus copies `static/` into *every locale's* output, so the same `data/x.tsv` is `/my-site/data/…`
  on the English pages and `/my-site/zh-Hans/data/…` on the Chinese ones: a block that writes the prefix
  out works in one locale and 404s in the other. The baseUrl reaches the browser through the config tag
  `dfkExtensions` injects (per-locale), and the harness through `baseUrl` / `DFK_BASE_URL` / `--base-url`.
- **The editor shows the resolved SQL, not the token.** `DfkSql` expands the placeholders when it fills
  the editor (`DuckDBRuntime.expand()`), so a reader reads — and copies, and edits — the URL that will
  actually be fetched. `execute()` expands again, which is what covers Reset, a hand-edited document and
  the headless harness.
- The default extension is the single file under `static/duckdb-extensions/`.
- **The `duckfn-sql-verify` command still exists** for a framework-free run (`duckfn-sql-verify
  --site .`), sharing the collector, the harness and the extension resolution with the Playwright
  path. Prefer Playwright Test when you want reports and IDE support; the CLI is the fallback.
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

## Colour swatches (`rehypeColorSwatch`)

```ts
// docusaurus.config.ts, in the docs preset's `rehypePlugins` — rehype, not
// remark: the swatch is a `style` attribute on finished HTML, and React refuses
// a string `style` prop on the JSX element a marked `<code>` becomes.
rehypePlugins: [rehypeColorSwatch],
```

Inline code whose whole content is a colour value is painted with that colour (the foreground is picked
between black and white by contrast), so a palette table reads as colour instead of as a column of hex codes.
`scan: false` turns the scan off; a marked element works either way:

````md
`#E69F00`                                   <!-- swatch -->
<code data-color-swatch>#E69F00</code>      <!-- swatch, without the scan -->
<code data-color-swatch="#E69F00">treat</code>
````

- Scanning takes whatever `colord` parses out of the box: `#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`,
  `rgb()` / `rgba()`, `hsl()` / `hsla()`. **CSS colour names work nowhere** — `red`, `white` and
  `transparent` are ordinary words in prose, and telling the name from the word needs a colour table the
  plugin does not load. Write hex, `rgb()` or `hsl()`.
- A fenced code block is never touched, an unparsable colour is left alone, and the plugin adds **no CSS**:
  the two inline styles are the whole effect, so the site's own `code` styling (border, radius, padding) is
  what the swatch sits in.
- **Do not reach for `{.color-swatch}` / `{color="…"}`.** MDX tokenises `{...}` as a JavaScript expression
  before any remark plugin runs: the first fails the build (`Could not parse expression with acorn`) and the
  second throws `ReferenceError: color is not defined` in the browser. The attribute above is the spelling
  that works.

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
