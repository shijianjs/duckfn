import type {ListTable, ListTableConstructorOptions} from '@visactor/vtable';
import type {QueryResult} from './runtime';
import type {RunnableSqlConfig} from './remark';
import {PreviewTabs, type PreviewTabItem, type PreviewTableHandle} from './PreviewTabs';
import {el} from '../dom';

/**
 * Result renderers, keyed by the config's `show` field.
 *
 * The registry is the seam later phases plug into. It ships `table` (VisActor
 * VTable), a `text` fallback, the markup previews `iframe` / `html` / `svg`,
 * plus the `error` view every renderer shares.
 *
 * Heavy dependencies (`@visactor/vtable`) load through dynamic `import()`
 * inside the renderer, so a page that never runs a query never pays for them,
 * and Docusaurus' Node prerender never touches them.
 */

export interface RenderContext {
  /** Container element in the light DOM, sized by `sql.css`. */
  host: HTMLElement;
  config: RunnableSqlConfig;
  /** Localised strings resolved by the component (labels-by-html-lang). */
  labels: Record<string, string>;
  /**
   * The component's fullscreen toggle, parked at the right end of the tab
   * strip. `<dfk-sql>` owns the button's state and therefore the node; the
   * renderer only borrows it so every result has the same chrome.
   */
  fullscreenButton: HTMLElement;
}

/**
 * Renders `result` into `context.host`. Resolves with a disposer releasing any
 * resources (table instances) the renderer created; it runs on the next render
 * and when the element disconnects.
 *
 * Every renderer is async — the heavy ones await their dynamic `import()`, and
 * the trivial ones just resolve immediately — so the caller has exactly one
 * shape to handle.
 */
export type Renderer = (
  context: RenderContext,
  result: QueryResult,
) => Promise<void | (() => void)>;

/**
 * The default `sandbox` for the `iframe` renderer: scripts run (HTML reports
 * draw their charts with them), but `allow-same-origin` is deliberately absent,
 * so the frame keeps an opaque origin and cannot reach this page.
 */
const DEFAULT_SANDBOX = 'allow-scripts';

/**
 * VTable's stock row height leaves a lot of air around a short cell; a docs
 * example is an aside, so the rows are tightened up. The header is a touch
 * taller than the body on purpose.
 */
const ROW_HEIGHT = 30;
const HEADER_HEIGHT = 32;

/**
 * Stable `menuKey`s for the context-menu items. VTable fires the click through
 * the `dropdown_menu_click` event (see {@link ResultTable.#onMenu}) with
 * `menuKey = menuItem.menuKey || menuItem.text`, so giving every item an
 * explicit key keeps the dispatch independent of the (localised) label text.
 */
const MENU = {
  copyCell: 'dfk-copy-cell',
  copyAll: 'dfk-copy-all',
  wrap: 'dfk-wrap',
  unwrap: 'dfk-unwrap',
  freeze: 'dfk-freeze',
  unfreeze: 'dfk-unfreeze',
  reset: 'dfk-reset',
} as const;

/** Theme shape VTable accepts in the constructor / `updateTheme`. */
type TableTheme = NonNullable<ListTableConstructorOptions['theme']>;
/** The VTable module namespace from the dynamic `import()` (type-only here). */
type VTableModule = typeof import('@visactor/vtable');
/** The two official themes the table switches between. */
type VTableThemeName = 'DEFAULT' | 'DARK';
type TableColumns = NonNullable<ListTableConstructorOptions['columns']>;
/**
 * The context-menu item shape. Mirrors VTable's `MenuListItem`, which is not
 * re-exported from the package root, so it is spelled out locally.
 */
type TableMenuItem =
  | string
  | {text?: string; type?: 'title' | 'item' | 'split'; menuKey?: string};

/**
 * A locale-aware, numeric-aware comparator shared by every sortable column.
 * `Intl.Collator` is built lazily (it is comparatively cheap but not free, and
 * most tables never sort).
 */
let collator: Intl.Collator | undefined;
function compareText(a: string, b: string): number {
  collator ??= new Intl.Collator(undefined, {numeric: true, sensitivity: 'base'});
  return collator.compare(a, b);
}

/**
 * Coerces a value to a number when it is genuinely numeric (so `9` sorts before
 * `10`), otherwise `null` so the caller falls back to a text comparison.
 * DuckDB-Wasm hands back native `number`/`bigint`/`Date`/`boolean` values.
 */
function numericValue(value: unknown): number | null {
  if (typeof value === 'number') {
    return Number.isNaN(value) ? null : value;
  }
  if (typeof value === 'bigint') {
    return Number(value);
  }
  if (value instanceof Date) {
    const time = value.getTime();
    return Number.isNaN(time) ? null : time;
  }
  if (typeof value === 'boolean') {
    return value ? 1 : 0;
  }
  return null;
}

/**
 * VTable's per-column `sort` callback. When a custom comparator is supplied,
 * VTable calls it with the current `order` and uses the result verbatim (it
 * does NOT flip for `desc` the way its built-in comparator does), so the
 * direction has to be applied here. NULL is pinned last in both directions (a
 * custom comparator must handle empty values itself), numbers compare
 * numerically and everything else textually.
 */
function compareValues(a: unknown, b: unknown, order: string): -1 | 0 | 1 {
  const aEmpty = a === null || a === undefined;
  const bEmpty = b === null || b === undefined;
  if (aEmpty || bEmpty) {
    if (aEmpty && bEmpty) {
      return 0;
    }
    // NULL last, regardless of direction.
    return aEmpty ? 1 : -1;
  }
  const numeric = numericValue(a);
  const otherNumeric = numericValue(b);
  let raw: number;
  if (numeric !== null && otherNumeric !== null) {
    raw = numeric === otherNumeric ? 0 : numeric < otherNumeric ? -1 : 1;
  } else {
    raw = compareText(stringify(a), stringify(b));
  }
  const sign: -1 | 0 | 1 = raw === 0 ? 0 : raw < 0 ? -1 : 1;
  if (sign === 0) {
    return 0;
  }
  return (String(order).toLowerCase() === 'desc' ? -sign : sign) as -1 | 0 | 1;
}

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** The shared error view: a styled block, never a thrown exception. */
function errorBlock(document: Document, text: string): HTMLPreElement {
  const pre = document.createElement('pre');
  pre.className = 'dfk-sql-error';
  pre.textContent = text;
  return pre;
}

function errorText(labels: Record<string, string>, detail: string): string {
  return `${labels.error ?? 'Error'}: ${detail}`;
}

/**
 * A VTable result grid with the interaction layer a docs example wants:
 * sortable columns, clipboard copy, resizable rows/columns, draggable headers,
 * cross-highlight on hover, per-column text wrapping and column freezing — all
 * driven through VTable's own options and events so the canvas stays the single
 * source of truth (nothing is re-laid-out in DOM).
 *
 * All styling comes from VTable's own themes (`themes.DEFAULT` / `themes.DARK`,
 * picked below by the document's colour scheme). The table deliberately does
 * not hand-pick colours property by property: vendor themes are complete, and
 * repainting them by hand is how a theme change or a select/hover state ends up
 * losing its text.
 *
 * The instance owns its box (`#record`); callers only reach it through
 * {@link dispose}.
 */
class ResultTable {
  readonly #table: ListTable;
  readonly #record: HTMLElement;
  readonly #result: QueryResult;
  readonly #labels: Record<string, string>;
  /** VTable's theme namespace, taken from the dynamic `import()`. */
  readonly #themes: VTableModule['themes'];
  /** Fields whose column currently wraps (row height switches to `auto`). */
  readonly #wrapped = new Set<string>();
  #frozen = 0;
  #frame = 0;
  /** The official theme currently applied, so a repaint happens only on change. */
  #appliedTheme?: VTableThemeName;
  #observer?: ResizeObserver;
  #themeObserver?: MutationObserver;

  constructor(
    vtable: VTableModule,
    record: HTMLElement,
    result: QueryResult,
    labels: Record<string, string>,
  ) {
    this.#record = record;
    this.#result = result;
    this.#labels = labels;
    this.#themes = vtable.themes;
    this.#table = new vtable.ListTable(this.#options());
    this.#appliedTheme = this.#themeName();

    // `resize()` re-measures and repaints inside `record`, so running it straight
    // from the observer callback feeds the resulting box change back into the very
    // delivery pass that is still going — the browser reports that as
    // "ResizeObserver loop completed with undelivered notifications" (and
    // webpack-dev-server turns it into a full-screen error overlay). Deferring to
    // the next frame keeps the notification and the re-measure in separate passes.
    this.#observer = new ResizeObserver(() => {
      cancelAnimationFrame(this.#frame);
      this.#frame = requestAnimationFrame(() => {
        this.#frame = 0;
        this.#table.resize();
      });
    });
    this.#observer.observe(record);

    // The canvas paints with concrete colours, so a light/dark flip has to be
    // observed and the theme re-applied — a `data-theme` attribute change never
    // reaches the canvas by itself.
    this.#themeObserver = new MutationObserver(() => {
      this.#applyTheme();
    });
    this.#themeObserver.observe(record.ownerDocument.documentElement, {
      attributeFilter: ['data-theme', 'class'],
    });

    this.#table.on('dropdown_menu_click', (args) => this.#onMenu(args));
  }

  #columns(order: readonly string[]): TableColumns {
    return order.map((field) => ({
      field,
      title: field,
      sort: compareValues,
      style: {autoWrapText: this.#wrapped.has(field)},
    }));
  }

  /**
   * The fields in their current display order. A header drag reorders the
   * layout (and `options.columns` with it), so the order is read back from the
   * table rather than assumed to still match the query result.
   */
  #displayOrder(): string[] {
    const order: string[] = [];
    for (let col = 0; col < this.#table.colCount; col += 1) {
      const field: unknown = this.#table.getHeaderField(col, 0);
      if (typeof field !== 'string' && typeof field !== 'number') {
        // Unexpected shape (or a layout mid-rebuild): keep the query order.
        return [...this.#result.columns];
      }
      order.push(String(field));
    }
    return order.length === this.#result.columns.length ? order : [...this.#result.columns];
  }

  /**
   * The official theme for the current colour mode: `themes.DEFAULT` in light,
   * `themes.DARK` in dark. Both are complete palettes (text, zebra rows, hover
   * tints, a *translucent* selection fill, the frozen-column shadow, sort
   * icons), so nothing has to be overridden by hand.
   *
   * `themes.of()` passes a `TableTheme` instance through unchanged, and
   * `TableTheme.extends()` is the official way to layer a small delta on top,
   * should one ever be needed.
   */
  #theme(): TableTheme {
    return this.#themes[this.#themeName()];
  }

  /** Which official theme the document's `data-theme` asks for. */
  #themeName(): VTableThemeName {
    const mode = this.#record.ownerDocument.documentElement.getAttribute('data-theme');
    return mode === 'dark' ? 'DARK' : 'DEFAULT';
  }

  /**
   * Re-applies the theme for the current colour mode. `updateTheme` repaints
   * the whole table (and rebuilds its components), so it is skipped unless the
   * mode actually changed.
   */
  #applyTheme(): void {
    const name = this.#themeName();
    if (name !== this.#appliedTheme) {
      this.#appliedTheme = name;
      this.#table.updateTheme(this.#theme());
    }
  }

  /** The official theme's own body text colour (the tip follows the theme). */
  #themeText(): string {
    const theme = this.#theme();
    const value = theme.bodyStyle?.color ?? theme.defaultStyle?.color;
    return typeof value === 'string' ? value : '#000';
  }

  /** The official theme's own body font size. */
  #themeFontSize(): number {
    const theme = this.#theme();
    const value = theme.bodyStyle?.fontSize ?? theme.defaultStyle?.fontSize;
    return typeof value === 'number' ? value : 12;
  }

  #options(): ListTableConstructorOptions {
    return {
      container: this.#record,
      records: this.#result.rows,
      columns: this.#columns(this.#result.columns),
      theme: this.#theme(),
      // The container width is handed to the columns: each keeps its measured
      // content as its share (the header measurement already includes the sort
      // icon), so the initial view fills the box instead of starting from a
      // default too narrow for its titles. Content-heavy columns get more room
      // than a flat "equal share" would give them; a very long unaliased header
      // is capped by `limitMaxAutoWidth` (450) before the share is computed.
      // It re-fills on container resizes (the fullscreen toggle included), and a
      // column the user resized by hand is excluded while the rest re-fill
      // around it.
      widthMode: 'adaptive',
      // `auto` row height is what actually lets a wrapped column grow its rows;
      // a fixed height would clip the extra lines even with `autoWrapText`.
      defaultRowHeight: this.#wrapped.size > 0 ? 'auto' : ROW_HEIGHT,
      defaultHeaderRowHeight: HEADER_HEIGHT,
      // Resizing is on by default, but stating it keeps the intent readable and
      // guards a future change of default.
      columnResizeMode: 'all',
      rowResizeMode: 'all',
      // Header drag-to-reorder is off by default; enable columns only (there is
      // no row header to drag). VTable requires the header cell to be selected
      // before it can be dragged, and its `fixedFrozenCount` default keeps the
      // frozen *count* stable while the frozen membership follows the new order.
      dragHeaderMode: 'column',
      // Cross highlight is what the "hover lights up the whole row + column"
      // behaviour maps to; the default is per-cell only. The tints themselves
      // come from the official theme's `hover` styles.
      hover: {highlightMode: 'cross'},
      // There is no default keyboard config, so the copy/select flags have to be
      // listed or Ctrl+C / Ctrl+A stay inert.
      keyboardOptions: {
        copySelected: true,
        selectAllOnCtrlA: true,
      },
      // Overflow tooltip: shows the full text of a clipped cell on hover.
      tooltip: {isShowOverflowTextTooltip: true},
      menu: {
        renderMode: 'html',
        contextMenuItems: (field) => this.#menuItems(String(field)),
      },
      frozenColCount: this.#frozen,
      emptyTip: {
        text: this.#labels.noData ?? 'No rows',
        // The stock empty tip paints `#000` whatever the theme; take both
        // values from the official theme instead of inventing colours.
        textStyle: {fontSize: this.#themeFontSize(), color: this.#themeText()},
      },
    };
  }

  #menuItems(field: string): TableMenuItem[] {
    const labels = this.#labels;
    const wrapped = this.#wrapped.has(field);
    const freezeOrUnfreeze: TableMenuItem =
      this.#frozen > 0
        ? {text: labels.unfreezeColumns ?? 'Unfreeze columns', menuKey: MENU.unfreeze}
        : {text: labels.freezeColumn ?? 'Freeze up to here', menuKey: MENU.freeze};
    return [
      {text: field, type: 'title'},
      {type: 'split'},
      {text: labels.copy ?? 'Copy cell', menuKey: MENU.copyCell},
      {text: labels.copyAll ?? 'Copy table', menuKey: MENU.copyAll},
      {
        text: wrapped
          ? (labels.unwrapColumn ?? 'Stop wrapping column')
          : (labels.wrapColumn ?? 'Wrap column'),
        menuKey: wrapped ? MENU.unwrap : MENU.wrap,
      },
      freezeOrUnfreeze,
      {type: 'split'},
      {text: labels.resetView ?? 'Reset view', menuKey: MENU.reset},
    ];
  }

  /**
   * Dispatch a context-menu click.
   *
   * VTable fires this through `dropdown_menu_click`, NOT `context_menu_click`:
   * in 1.26.8 the html menu's own click handler emits `dropdown_menu_click`
   * (with `menuKey = menuItem.menuKey || menuItem.text`), and the
   * `context_menu_click` event type is defined but never fired — listening to
   * it silently does nothing.
   */
  #onMenu(args: {col?: number; row?: number; menuKey?: string}): void {
    const col = args.col ?? -1;
    const row = args.row ?? -1;
    const menuKey = args.menuKey;
    if (!menuKey) {
      return;
    }
    // The owning column's field; resolves for body cells and header cells alike.
    const info = col >= 0 && row >= 0 ? this.#table.getCellInfo(col, row) : undefined;
    const field = info?.field === undefined ? '' : String(info.field);
    switch (menuKey) {
      case MENU.copyCell:
        void this.#copy(this.#cellText(col, row));
        break;
      case MENU.copyAll:
        void this.#copy(this.#allText());
        break;
      case MENU.wrap:
        if (field) {
          this.#toggleWrap(field, true);
        }
        break;
      case MENU.unwrap:
        if (field) {
          this.#toggleWrap(field, false);
        }
        break;
      case MENU.freeze:
        this.#freeze(col);
        break;
      case MENU.unfreeze:
        this.#freeze(-1);
        break;
      case MENU.reset:
        this.#reset();
        break;
    }
  }

  #cellText(col: number, row: number): string {
    if (col < 0 || row < 0) {
      return '';
    }
    return stringify(this.#table.getCellRawValue(col, row));
  }

  #toggleWrap(field: string, on: boolean): void {
    if (on) {
      this.#wrapped.add(field);
    } else {
      this.#wrapped.delete(field);
    }
    this.#table.defaultRowHeight = this.#wrapped.size > 0 ? 'auto' : ROW_HEIGHT;
    // Clear the row-height cache so rows regrow, but keep the column-width
    // cache — widths the user dragged (and any row heights they resized) stay.
    // Rebuild from the *display* order: `updateColumns` applies the array it is
    // given verbatim, so the query order would undo any header drag.
    this.#table.updateColumns(this.#columns(this.#displayOrder()), {
      clearColWidthCache: false,
      clearRowHeightCache: true,
    });
  }

  #freeze(col: number): void {
    // `setFrozenColCount` clamps to the table width and collapses to 0 once it
    // would freeze every column, so read the effective count back instead of
    // assuming `col + 1` stuck.
    this.#table.setFrozenColCount(col < 0 ? 0 : col + 1);
    this.#frozen = this.#table.frozenColCount;
  }

  #reset(): void {
    this.#wrapped.clear();
    this.#frozen = 0;
    // `updateOption` (unlike `updateColumns`) also resets the sort state, and
    // with both caches cleared it drops the dragged widths/heights too — a true
    // "back to the initial view". `#options()` carries the query's column order,
    // so a header drag is undone as well.
    void this.#table.updateOption(this.#options(), {
      clearColWidthCache: true,
      clearRowHeightCache: true,
    });
  }

  /** The whole visible grid as tab-separated text, in current display order. */
  #allText(): string {
    const table = this.#table;
    const lines = [this.#result.columns.join('\t')];
    for (let row = table.columnHeaderLevelCount; row < table.rowCount; row += 1) {
      const cells: string[] = [];
      for (let col = 0; col < table.colCount; col += 1) {
        cells.push(stringify(table.getCellRawValue(col, row)));
      }
      lines.push(cells.join('\t'));
    }
    return lines.join('\n');
  }

  async #copy(text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Clipboard access can be denied (permissions, insecure context); a
      // silent no-op beats throwing from a click handler.
    }
  }

  dispose(): void {
    // The box may have shrunk a frame ago; never resize a released table.
    cancelAnimationFrame(this.#frame);
    this.#observer?.disconnect();
    this.#themeObserver?.disconnect();
    this.#table.release();
  }
}

/**
 * Mounts a VTable list into `parent`, creating the `.dfk-sql-table` box itself.
 *
 * VTable is canvas-rendered and measures its container at construction time, so
 * the box gets an explicit height through a custom property (which the
 * fullscreen rule overrides by specificity rather than `!important`); the
 * `ResizeObserver` inside {@link ResultTable} re-measures whenever that box
 * changes shape — which is also how the table follows the fullscreen toggle
 * without any resize plumbing through the component.
 *
 * A failed `import()` (offline, CDN blocked) degrades to the error view instead
 * of rejecting the render.
 */
async function mountTable(
  parent: HTMLElement,
  result: QueryResult,
  labels: Record<string, string>,
): Promise<PreviewTableHandle> {
  const document = parent.ownerDocument;
  const record = el('div', {class: 'dfk-sql-table'});
  record.style.setProperty('--dfk-sql-table-height', `${Math.min(360, 36 + result.rows.length * ROW_HEIGHT)}px`);
  parent.appendChild(record);

  try {
    const vtable = await import('@visactor/vtable');
    const table = new ResultTable(vtable, record, result, labels);
    return {dispose: () => table.dispose()};
  } catch (error) {
    record.appendChild(errorBlock(document, errorText(labels, messageOf(error))));
    return {dispose: () => {}};
  }
}

const tableRenderer: Renderer = async ({host, labels, fullscreenButton}, result) => {
  const tabs = new PreviewTabs(
    host,
    [],
    labels.table ?? 'Table',
    (panel) => mountTable(panel, result, labels),
    fullscreenButton,
  );
  return () => tabs.dispose();
};

/** Plain text: one line per row, columns tab-joined. */
function textBlock(document: Document, result: QueryResult): HTMLPreElement {
  const pre = document.createElement('pre');
  pre.className = 'dfk-sql-text';
  const lines = [result.columns.join('\t')];
  for (const row of result.rows) {
    lines.push(result.columns.map((c) => stringify(row[c])).join('\t'));
  }
  pre.textContent = lines.join('\n');
  return pre;
}

/**
 * Text results get the same chrome as every other result: a `Text` tab plus the
 * trailing `Table` tab, so a scalar can still be inspected as a table.
 */
const textRenderer: Renderer = async ({host, labels, fullscreenButton}, result) => {
  const tabs = new PreviewTabs(
    host,
    [
      {
        label: labels.text ?? 'Text',
        mount: (panel) => panel.appendChild(textBlock(panel.ownerDocument, result)),
      },
    ],
    labels.table ?? 'Table',
    (panel) => mountTable(panel, result, labels),
    fullscreenButton,
  );
  return () => tabs.dispose();
};

/** Shared error view: a styled block, never a thrown exception. */
export const errorRenderer: Renderer = async ({host, labels}, result) => {
  host.replaceChildren(errorBlock(host.ownerDocument, errorText(labels, result.error ?? 'unknown')));
};

/**
 * The column holding the markup. `field` wins; a single-column result is
 * unambiguous, so it is used as-is.
 */
function resolveField(config: RunnableSqlConfig, result: QueryResult): string | null {
  if (config.field) {
    return config.field;
  }
  return result.columns.length === 1 ? result.columns[0] : null;
}

/**
 * Parses SVG markup from a result cell into a node this document can host.
 *
 * `image/svg+xml` is strict XML: malformed markup (unclosed tags, a bare `&`,
 * an HTML `<br>`) comes back as a `<parsererror>` element rather than throwing,
 * so the caller can degrade to text. Script-bearing and event-handler content
 * is stripped — inline SVG is *not* isolated (use the `iframe` renderer for
 * untrusted markup).
 */
function parseSvgMarkup(document: Document, markup: string): SVGElement | null {
  if (!markup.trim()) {
    return null;
  }
  const parsed = new DOMParser().parseFromString(markup, 'image/svg+xml');
  const root = parsed.documentElement;
  if (!root || root.localName === 'parsererror' || root.namespaceURI !== SVG_NAMESPACE) {
    return null;
  }
  stripActiveContent(root);
  return document.importNode(root, true) as unknown as SVGElement;
}

/** Removes the parts of an SVG document that could execute or navigate. */
function stripActiveContent(root: Element): void {
  for (const node of root.querySelectorAll('script, foreignObject')) {
    node.remove();
  }
  const visit = (element: Element): void => {
    for (const attribute of [...element.attributes]) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.replace(/\s/g, '').toLowerCase();
      if (name.startsWith('on') || ((name === 'href' || name === 'xlink:href') && value.startsWith('javascript:'))) {
        element.removeAttribute(attribute.name);
      }
    }
    for (const child of element.children) {
      visit(child);
    }
  };
  visit(root);
}

/** Applies `option.width` / `option.height` as custom properties the CSS consumes. */
function applyPreviewSize(node: HTMLElement, config: RunnableSqlConfig): void {
  const {width, height} = config.option ?? {};
  if (width) {
    node.style.setProperty('--dfk-sql-preview-width', width);
  }
  if (height) {
    node.style.setProperty('--dfk-sql-preview-height', height);
  }
}

function mountPreviewPanel(
  kind: 'iframe' | 'svg',
  panel: HTMLElement,
  value: unknown,
  label: string,
  config: RunnableSqlConfig,
): void {
  const markup = value === null || value === undefined ? '' : String(value);
  if (kind === 'iframe') {
    const frame = el('iframe', {
      class: 'dfk-sql-frame',
      srcdoc: markup,
      attrs: {
        // `sandbox` is a DOMTokenList on the element, so it can only travel
        // through `attrs`; the default is never an unsandboxed frame.
        sandbox: config.option?.sandbox ?? DEFAULT_SANDBOX,
        loading: 'lazy',
        referrerpolicy: 'no-referrer',
        title: label,
      },
    });
    applyPreviewSize(frame, config);
    panel.appendChild(frame);
    return;
  }

  const svg = parseSvgMarkup(panel.ownerDocument, markup);
  if (!svg) {
    // Not SVG: show the markup as text rather than an empty panel.
    panel.appendChild(el('pre', {class: 'dfk-sql-text', text: markup}));
    return;
  }
  const holder = el('div', {class: 'dfk-sql-svg'});
  holder.appendChild(svg);
  applyPreviewSize(holder, config);
  panel.appendChild(holder);
}

/**
 * Builds a preview renderer: one tab per row, then the raw rows in the trailing
 * `Table` tab. `iframe` and `svg` share everything except how a panel is filled.
 */
function previewRenderer(kind: 'iframe' | 'svg'): Renderer {
  return async ({host, config, labels, fullscreenButton}, result) => {
    const field = resolveField(config, result);
    if (!field) {
      host.replaceChildren(
        errorBlock(
          host.ownerDocument,
          errorText(labels, labels.noField ?? 'this result has no markup column; set `field`'),
        ),
      );
      return;
    }

    const tabName = config.tab_name;
    const items: PreviewTabItem[] = result.rows.map((row, index) => {
      const value = tabName ? row[tabName] : undefined;
      return {
        label: value === null || value === undefined ? `${labels.row ?? 'Row'} ${index + 1}` : stringify(value),
        mount: (panel) => mountPreviewPanel(kind, panel, row[field], '', config),
      };
    });

    const tabs = new PreviewTabs(
      host,
      items,
      labels.table ?? 'Table',
      (panel) => mountTable(panel, result, labels),
      fullscreenButton,
    );
    return () => tabs.dispose();
  };
}

function stringify(value: unknown): string {
  if (value === null || value === undefined) {
    return 'NULL';
  }
  if (value instanceof Date) {
    return value.toLocaleString();
  }
  return typeof value === 'object' ? JSON.stringify(value) : String(value);
}

const registry: Record<string, Renderer> = {
  table: tableRenderer,
  text: textRenderer,
  iframe: previewRenderer('iframe'),
  // `html` is the historical spelling of the same renderer; both stay valid.
  html: previewRenderer('iframe'),
  svg: previewRenderer('svg'),
};

/**
 * Picks the renderer for a result: the config's `show` wins, otherwise a
 * single-column single-row result degrades to `text` (a bare scalar like
 * `SELECT 1;` reads better as a line than as a 1×1 table), otherwise `table`.
 */
export function rendererFor(config: RunnableSqlConfig, result: QueryResult): Renderer {
  if (result.error) {
    return errorRenderer;
  }
  const show = config.show ?? (result.columns.length === 1 && result.rows.length === 1 ? 'text' : 'table');
  return registry[show] ?? tableRenderer;
}