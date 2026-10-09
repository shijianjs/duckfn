import type {ListTable, ListTableConstructorOptions} from '@visactor/vtable';
import type {SearchComponent} from '@visactor/vtable-search';
import type {QueryResult} from './runtime';
import type {RunnableSqlConfig} from './remark';
import {PreviewTabs, type PreviewTabItem} from './PreviewTabs';
import {SvgViewer, parseSvgMarkup, type FigureLabels} from './SvgViewer';
import {TerminalView} from './terminal';
import {el} from '../dom';
import {sectionFileName, type DownloadPayload} from '../download';
import {IconButton} from '../IconButton';

/**
 * Result renderers, keyed by the config's `show` field.
 *
 * The registry is the seam later phases plug into. It ships `table` (VisActor
 * VTable), a `text` fallback, the markup previews `iframe` / `html` / `svg`,
 * `mermaid` (which hands the cell to the kit's own `<dfk-mermaid>` element, so a
 * query can produce a diagram the reader can zoom, expand, edit and download),
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
  /**
   * Subscribes to the result area's fullscreen state, calling back with the
   * current value straight away and returning the unsubscribe. A renderer whose
   * figure zooms only in fullscreen (`svg`, `mermaid`) forwards the value to its
   * viewer; the subscription must be released by the renderer's disposer.
   */
  onFullscreenChange(listener: (value: boolean) => void): () => void;
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

/** What a mounted VTable hands back: its release hook, and its export. */
interface PreviewTableHandle {
  dispose(): void;
  /** The visible grid as CSV, in the current display order. */
  csv(): string;
}

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
 * Stable `menuKey`s for the context menu. VTable fires a context-menu click
 * through the `dropdown_menu_click` event (see {@link ResultTable.#onMenu}) with
 * `menuKey = menuItem.menuKey || menuItem.text`, so giving every item an
 * explicit key keeps the dispatch independent of the (localised) label text.
 *
 * Only the per-cell and per-column items live here now: the actions that act on
 * the grid as a whole (search, copy the table, the width modes, reset, unfreeze)
 * moved to the result area's tab strip, where they neither cover the cells nor
 * sit in a menu a reader has to find.
 */
const MENU = {
  copyCell: 'dfk-copy-cell',
  wrap: 'dfk-wrap',
  unwrap: 'dfk-unwrap',
  freeze: 'dfk-freeze',
} as const;

/**
 * The column-width view modes the tab strip's width button cycles through. Each
 * is a plain pair of official VTable options:
 *
 * - `adaptive` (the default) hands the container width to the columns: every
 *   column keeps its measured content as its share, so the table always fills
 *   the box.
 * - `standard` keeps each column at its measured content width and scrolls
 *   sideways when the total overflows.
 * - `standard` + `autoFillWidth` keeps content widths but stretches them to
 *   fill when the content happens to be narrower than the box.
 *
 * `id` is the button's identity (there is no context-menu entry to key it to any
 * more); `label`/`fallback` are the labels key and its English default, so a
 * consumer that only provides a few strings still gets text for every item.
 */
const WIDTH_MODES = [
  {
    id: 'dfk-width-adaptive',
    label: 'widthAdaptive',
    widthMode: 'adaptive',
    autoFillWidth: false,
    fallback: 'Fill the width',
  },
  {
    id: 'dfk-width-standard',
    label: 'widthStandard',
    widthMode: 'standard',
    autoFillWidth: false,
    fallback: 'Content widths, scroll sideways',
  },
  {
    id: 'dfk-width-fill',
    label: 'widthFill',
    widthMode: 'standard',
    autoFillWidth: true,
    fallback: 'Content first, fill when it fits',
  },
] as const;

/** The width-mode table row for an `id` (the default when unknown). */
function widthModeOption(id: string): (typeof WIDTH_MODES)[number] {
  return WIDTH_MODES.find((mode) => mode.id === id) ?? WIDTH_MODES[0];
}

/** Theme shape VTable accepts in the constructor / `updateTheme`. */
type TableTheme = NonNullable<ListTableConstructorOptions['theme']>;
/** The VTable module namespace from the dynamic `import()` (type-only here). */
type VTableModule = typeof import('@visactor/vtable');
/** The two official themes the table switches between. */
type VTableThemeName = 'DEFAULT' | 'DARK';
type TableColumns = NonNullable<ListTableConstructorOptions['columns']>;
/**
 * The search component's highlight style. Its option type demands a complete
 * `CellStyle` where only the background is read, so this names that one
 * property and the tint can be handed over without inventing 26 more.
 */
type HighlightStyle = NonNullable<
  ConstructorParameters<typeof SearchComponent>[0]['highlightCellStyle']
>;
/**
 * The context-menu item shape. Mirrors VTable's `MenuListItem`, which is not
 * re-exported from the package root, so it is spelled out locally.
 */
type TableMenuItem =
  | string
  | {
      text?: string;
      type?: 'title' | 'item' | 'split';
      menuKey?: string;
      children?: TableMenuItem[];
    };

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
 * One CSV field, quoted per RFC 4180: a value containing a comma, a double quote
 * or a line break is wrapped in quotes, and its own quotes are doubled. Nothing
 * else is touched, so plain values stay readable in a raw diff.
 */
function csvCell(value: string): string {
  return /[",\r\n]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
}

/**
 * A VTable result grid with the interaction layer a docs example wants:
 * sortable columns, clipboard copy, resizable rows/columns, draggable headers,
 * cross-highlight on hover, per-column text wrapping, column freezing and a
 * search — all driven through VTable's own options and events so the canvas
 * stays the single source of truth (nothing is re-laid-out in DOM).
 *
 * The actions that act on the grid as a whole (search, copy the table, the
 * column-width view modes, reset the view, unfreeze) live in the result area's
 * tab strip, not in a floating cluster over the canvas: the table is as wide as
 * the page, and anything floating would cover cells. They are built here, into
 * the container the renderer made for this tab's {@link PreviewTabItem.actions}.
 * Only the per-cell and per-column items stay in the context menu.
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
  /** The strip's container for this table's controls. */
  readonly #actions: HTMLElement;
  readonly #searchBtn: IconButton;
  readonly #searchInput: HTMLInputElement;
  readonly #searchCount: HTMLElement;
  readonly #widthBtn: IconButton;
  readonly #unfreezeBtn: IconButton;
  /** Fields whose column currently wraps (row height switches to `auto`). */
  readonly #wrapped = new Set<string>();
  #frozen = 0;
  #frame = 0;
  /** The official theme currently applied, so a repaint happens only on change. */
  #appliedTheme?: VTableThemeName;
  /** The active width mode's `id` (see `WIDTH_MODES`; default: adaptive). */
  #widthMode: string = WIDTH_MODES[0].id;
  #search: SearchComponent | null = null;
  #searchLoading = false;
  #observer?: ResizeObserver;
  #themeObserver?: MutationObserver;

  constructor(
    vtable: VTableModule,
    record: HTMLElement,
    result: QueryResult,
    labels: Record<string, string>,
    actions: HTMLElement,
  ) {
    this.#record = record;
    this.#result = result;
    this.#labels = labels;
    this.#themes = vtable.themes;
    this.#actions = actions;
    this.#searchBtn = new IconButton('lucide:search', () => this.#toggleSearch());
    this.#searchInput = el('input', {
      class: 'dfk-sql-search-input',
      type: 'search',
      hidden: true,
      attrs: {placeholder: labels.search ?? 'Search', 'aria-label': labels.search ?? 'Search'},
    });
    this.#searchCount = el('span', {class: 'dfk-sql-search-count', hidden: true});
    this.#widthBtn = new IconButton('lucide:stretch-horizontal', () => this.#cycleWidthMode());
    this.#unfreezeBtn = new IconButton('lucide:pin-off', () => this.#freeze(-1));
    this.#table = new vtable.ListTable(this.#options());
    this.#appliedTheme = this.#themeName();
    this.#buildActions();

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

  /**
   * Builds the strip's controls, once. Each is a plain button or input held in a
   * field — the search box is revealed in place rather than opened as a popover,
   * because the strip has the room and a popover over a full-width table is the
   * thing this arrangement exists to avoid.
   */
  #buildActions(): void {
    const copyAll = new IconButton('lucide:copy', () => void this.#copy(this.#allText()));
    copyAll.setLabel(this.#labels.copyAll ?? 'Copy table');
    const reset = new IconButton('lucide:rotate-ccw', () => this.#reset());
    reset.setLabel(this.#labels.resetView ?? 'Reset view');
    this.#searchBtn.setLabel(this.#labels.search ?? 'Search');
    this.#unfreezeBtn.setLabel(this.#labels.unfreezeColumns ?? 'Unfreeze columns');
    this.#searchInput.addEventListener('input', () => this.#runSearch());
    this.#searchInput.addEventListener('keydown', (event) => this.#onSearchKey(event));
    this.#actions.append(
      this.#searchBtn.root,
      this.#searchCount,
      this.#searchInput,
      copyAll.root,
      this.#widthBtn.root,
      reset.root,
      this.#unfreezeBtn.root,
    );
    this.#updateWidthLabel();
    this.#updateUnfreeze();
  }

  // --- Search ----------------------------------------------------------------

  /** `@visactor/vtable-search`, loaded on first use like every other heavy dep. */
  async #ensureSearch(): Promise<SearchComponent | null> {
    if (this.#search !== null || this.#searchLoading) {
      return this.#search;
    }
    this.#searchLoading = true;
    try {
      const {SearchComponent} = await import('@visactor/vtable-search');
      // The highlight has to be chosen per colour mode: a translucent amber that
      // reads as "found" over a light cell is glaring over a dark one. The
      // vendor's option type asks for a *complete* `CellStyle` even though it
      // only reads the background, so the tint is cast — see `HighlightStyle`.
      const dark = this.#themeName() === 'DARK';
      this.#search = new SearchComponent({
        table: this.#table,
        // The header row is not content; matching a column title would point the
        // reader at a cell they cannot compare with anything.
        skipHeader: true,
        highlightCellStyle: {
          bgColor: dark ? 'rgba(255, 214, 0, 0.25)' : 'rgba(255, 214, 0, 0.45)',
        } as HighlightStyle,
        focusHighlightCellStyle: {
          bgColor: dark ? 'rgba(255, 152, 0, 0.55)' : 'rgba(255, 152, 0, 0.7)',
        } as HighlightStyle,
      });
    } catch {
      // Unavailable (offline, CDN blocked): the box stays, the search does not.
    } finally {
      this.#searchLoading = false;
    }
    return this.#search;
  }

  #toggleSearch(): void {
    if (!this.#searchInput.hidden) {
      this.#closeSearch();
      return;
    }
    this.#searchInput.hidden = false;
    this.#searchBtn.setOn(true);
    this.#searchInput.focus();
  }

  #closeSearch(): void {
    this.#searchInput.value = '';
    this.#searchInput.hidden = true;
    this.#searchCount.hidden = true;
    this.#searchCount.textContent = '';
    this.#searchBtn.setOn(false);
    this.#search?.clear();
  }

  #onSearchKey(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      this.#closeSearch();
      return;
    }
    if (event.key !== 'Enter') {
      return;
    }
    // Enter walks forward through the matches, Shift+Enter back.
    event.preventDefault();
    const query = this.#searchInput.value.trim();
    if (!query || this.#search === null) {
      return;
    }
    this.#reportSearch(event.shiftKey ? this.#search.prev() : this.#search.next());
  }

  async #runSearch(): Promise<void> {
    const query = this.#searchInput.value.trim();
    if (!query) {
      this.#search?.clear();
      this.#reportSearch(null);
      return;
    }
    const search = await this.#ensureSearch();
    if (search === null) {
      return;
    }
    this.#reportSearch(search.search(query));
  }

  /** Shows `current / total` while a query is active, nothing otherwise. */
  #reportSearch(result: {index: number; results: unknown[]} | null): void {
    const total = result?.results.length ?? 0;
    const current = total === 0 ? 0 : (result?.index ?? 0) + 1;
    if (this.#searchInput.value.trim() === '') {
      this.#searchCount.textContent = '';
      this.#searchCount.hidden = true;
      return;
    }
    this.#searchCount.textContent = `${current}/${total}`;
    this.#searchCount.hidden = false;
  }

  // --- Strip actions ---------------------------------------------------------

  /** Cycles adaptive → content → content-fill → adaptive (see `WIDTH_MODES`). */
  #cycleWidthMode(): void {
    const index = WIDTH_MODES.findIndex((mode) => mode.id === this.#widthMode);
    this.#setWidthMode(WIDTH_MODES[(index + 1) % WIDTH_MODES.length]);
    this.#updateWidthLabel();
  }

  /** The tooltip names the mode now active, since the icon alone cannot. */
  #updateWidthLabel(): void {
    const mode = widthModeOption(this.#widthMode);
    this.#widthBtn.setLabel(this.#labels[mode.label] ?? mode.fallback);
  }

  #updateUnfreeze(): void {
    this.#unfreezeBtn.root.hidden = this.#frozen === 0;
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
    const width = widthModeOption(this.#widthMode);
    return {
      container: this.#record,
      records: this.#result.rows,
      columns: this.#columns(this.#result.columns),
      theme: this.#theme(),
      // How the container width is shared out; see `WIDTH_MODES`. `adaptive`
      // (the default) hands it to the columns — each keeps its measured content
      // (the header measurement already includes the sort icon) as its share, so
      // the initial view fills the box instead of starting from a default too
      // narrow for its titles, and a very long unaliased header is capped by
      // `limitMaxAutoWidth` (450) before the share is computed. It re-fills on
      // container resizes (the fullscreen toggle included), and a column the
      // user resized by hand is excluded while the rest re-fill around it.
      widthMode: width.widthMode,
      autoFillWidth: width.autoFillWidth,
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

  /**
   * The context menu, which now holds only what acts on *one cell or one
   * column*: the field name, copy, wrap and freeze. The grid-wide actions sit in
   * the tab strip instead (see {@link #buildActions}), so the menu cannot grow
   * into a second, hidden toolbar.
   */
  #menuItems(field: string): TableMenuItem[] {
    const labels = this.#labels;
    const wrapped = this.#wrapped.has(field);
    return [
      {text: field, type: 'title'},
      {type: 'split'},
      {text: labels.copy ?? 'Copy cell', menuKey: MENU.copyCell},
      {
        text: wrapped
          ? (labels.unwrapColumn ?? 'Stop wrapping column')
          : (labels.wrapColumn ?? 'Wrap column'),
        menuKey: wrapped ? MENU.unwrap : MENU.wrap,
      },
      {text: labels.freezeColumn ?? 'Freeze up to here', menuKey: MENU.freeze},
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
    this.#updateUnfreeze();
  }

  /**
   * Switches how the container width is shared out (see `WIDTH_MODES`).
   *
   * The two option setters only store the values, so the re-layout is driven
   * through `updateColumns`: rebuilding the scene graph re-measures the columns
   * under the new mode, and unlike `updateOption` it leaves the sort state
   * alone. The columns go in as the *display* order (a header drag survives),
   * and clearing only the column-width cache drops the manual widths — a mode
   * change starts from a clean slate — while the user's row heights survive.
   */
  #setWidthMode(mode: (typeof WIDTH_MODES)[number]): void {
    if (mode.id === this.#widthMode) {
      return;
    }
    this.#widthMode = mode.id;
    this.#table.widthMode = mode.widthMode;
    this.#table.autoFillWidth = mode.autoFillWidth;
    this.#table.updateColumns(this.#columns(this.#displayOrder()), {
      clearColWidthCache: true,
      clearRowHeightCache: false,
    });
  }

  #reset(): void {
    this.#wrapped.clear();
    this.#frozen = 0;
    // The width mode is a view switch too, so "reset" returns it to the default.
    this.#widthMode = WIDTH_MODES[0].id;
    // `updateOption` (unlike `updateColumns`) also resets the sort state, and
    // with both caches cleared it drops the dragged widths/heights too — a true
    // "back to the initial view". `#options()` carries the query's column order,
    // so a header drag is undone as well.
    void this.#table.updateOption(this.#options(), {
      clearColWidthCache: true,
      clearRowHeightCache: true,
    });
    this.#updateWidthLabel();
    this.#updateUnfreeze();
  }

  /** The whole visible grid as tab-separated text, in current display order. */
  #allText(): string {
    const table = this.#table;
    const lines = [this.#displayOrder().join('\t')];
    for (let row = table.columnHeaderLevelCount; row < table.rowCount; row += 1) {
      const cells: string[] = [];
      for (let col = 0; col < table.colCount; col += 1) {
        cells.push(stringify(table.getCellRawValue(col, row)));
      }
      lines.push(cells.join('\t'));
    }
    return lines.join('\n');
  }

  /**
   * The whole visible grid as CSV, in the current display order (the header
   * included). Distinct from {@link #allText}, which is tab-separated and only
   * ever lands on the clipboard: a downloaded file is opened in a spreadsheet,
   * so it has to be genuinely comma-separated and quoted.
   */
  csv(): string {
    const table = this.#table;
    const lines = [this.#displayOrder().map(csvCell).join(',')];
    for (let row = table.columnHeaderLevelCount; row < table.rowCount; row += 1) {
      const cells: string[] = [];
      for (let col = 0; col < table.colCount; col += 1) {
        cells.push(csvCell(stringify(table.getCellRawValue(col, row))));
      }
      lines.push(cells.join(','));
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

/** How many skeleton rows and columns the table placeholder draws. */
const SKELETON_ROWS = 2;
const SKELETON_COLUMNS = 4;

/**
 * The two-row ghost of a table, shown inside `.dfk-sql-table` while VTable's
 * bundle is in flight — it is the largest of the lazy `import()`s in a runnable
 * block, and the box would otherwise be a blank rectangle. Two rows is the
 * smallest shape that still reads as "a table is coming"; the columns line up
 * between them because every row is built the same way.
 *
 * The row rhythm comes from `ROW_HEIGHT`, handed over as a custom property so the
 * one number stays in this file. `sql.css` clips the ghost to the box: a result
 * with a single row is shorter than two skeleton rows.
 */
function tableSkeleton(): HTMLElement {
  const box = el('div', {class: 'dfk-sql-table-skeleton'});
  box.style.setProperty('--dfk-sql-skeleton-row-height', `${ROW_HEIGHT}px`);
  for (let row = 0; row < SKELETON_ROWS; row += 1) {
    box.appendChild(
      el('div', {class: 'dfk-sql-table-skeleton-row'}, (line) => {
        for (let column = 0; column < SKELETON_COLUMNS; column += 1) {
          line.appendChild(el('span', {class: 'dfk-sql-table-skeleton-cell'}));
        }
      }),
    );
  }
  return box;
}

/**
 * Mounts a VTable list into `parent`, creating the `.dfk-sql-table` box itself,
 * and fills `actions` with the controls that act on the table as a whole.
 *
 * VTable is canvas-rendered and measures its container at construction time, so
 * the box gets an explicit height through a custom property (which the
 * fullscreen rule overrides by specificity rather than `!important`); the
 * `ResizeObserver` inside {@link ResultTable} re-measures whenever that box
 * changes shape — which is also how the table follows the fullscreen toggle
 * without any resize plumbing through the component.
 *
 * A failed `import()` (offline, CDN blocked) degrades to the error view instead
 * of rejecting the render; the CSV export keeps working from the raw rows.
 */
async function mountTable(
  parent: HTMLElement,
  result: QueryResult,
  labels: Record<string, string>,
  actions: HTMLElement,
): Promise<PreviewTableHandle> {
  const document = parent.ownerDocument;
  const record = el('div', {class: 'dfk-sql-table'});
  record.style.setProperty('--dfk-sql-table-height', `${Math.min(360, 36 + result.rows.length * ROW_HEIGHT)}px`);
  const skeleton = tableSkeleton();
  record.appendChild(skeleton);
  parent.appendChild(record);

  try {
    const vtable = await import('@visactor/vtable');
    const table = new ResultTable(vtable, record, result, labels, actions);
    // VTable draws into the same box; the ghost is one sibling too many. It is
    // dropped after construction (not by VTable) so a failed import can still
    // hand the box over to the error view.
    skeleton.remove();
    return {dispose: () => table.dispose(), csv: () => table.csv()};
  } catch (error) {
    skeleton.remove();
    record.appendChild(errorBlock(document, errorText(labels, messageOf(error))));
    return {dispose: () => {}, csv: () => csvText(result)};
  }
}

/**
 * The trailing `Table` tab, as a plain {@link PreviewTabItem} like any other.
 *
 * It is built up front — before the tab is ever shown — because the tab strip
 * needs the item's `actions` container in its constructor, while the table
 * itself is only created on first activation (VTable is the heaviest lazy
 * import in the kit). `download` therefore reads a live handle, and answers
 * `null` until the table exists rather than exporting something else.
 */
function tableItem(
  result: QueryResult,
  labels: Record<string, string>,
  host: HTMLElement,
): PreviewTabItem {
  const actions = el('div', {class: 'dfk-sql-tab-actions'});
  let handle: PreviewTableHandle | null = null;
  return {
    label: labels.table ?? 'Table',
    actions,
    mount: async (panel) => {
      handle = await mountTable(panel, result, labels, actions);
      return () => {
        handle?.dispose();
        handle = null;
      };
    },
    download: (): DownloadPayload | null => {
      if (handle === null) {
        return null;
      }
      return {
        name: sectionFileName(host, 'csv', {fallback: 'table'}),
        mime: 'text/csv;charset=utf-8',
        text: handle.csv(),
      };
    },
  };
}

const tableRenderer: Renderer = async ({host, labels, fullscreenButton}, result) => {
  const tabs = new PreviewTabs(
    host,
    [tableItem(result, labels, host)],
    labels.download ?? 'Download',
    fullscreenButton,
  );
  return () => tabs.dispose();
};

/** Plain text: one line per row, columns tab-joined. */
function rowsText(result: QueryResult): string {
  const lines = [result.columns.join('\t')];
  for (const row of result.rows) {
    lines.push(result.columns.map((column) => stringify(row[column])).join('\t'));
  }
  return lines.join('\n');
}

/** The raw rows as CSV; the export used when VTable itself never loaded. */
function csvText(result: QueryResult): string {
  const lines = [result.columns.map(csvCell).join(',')];
  for (const row of result.rows) {
    lines.push(result.columns.map((column) => csvCell(stringify(row[column]))).join(','));
  }
  return lines.join('\n');
}

/**
 * Text results get the same chrome as every other result: a `Text` tab plus the
 * trailing `Table` tab, so a scalar can still be inspected as a table.
 */
const textRenderer: Renderer = async ({host, labels, fullscreenButton}, result) => {
  const text = rowsText(result);
  const tabs = new PreviewTabs(
    host,
    [
      {
        label: labels.text ?? 'Text',
        mount: (panel) => {
          panel.appendChild(el('pre', {class: 'dfk-sql-text', text}));
        },
        download: () => ({
          name: sectionFileName(host, 'txt', {fallback: 'text'}),
          mime: 'text/plain;charset=utf-8',
          text,
        }),
      },
      tableItem(result, labels, host),
    ],
    labels.download ?? 'Download',
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

/** What a preview tab shows for one row of the result. */
type PreviewKind = 'iframe' | 'svg' | 'mermaid' | 'terminal';

/** A result cell as markup: `NULL` means there is nothing to show. */
function markupOf(value: unknown): string {
  return value === null || value === undefined ? '' : String(value);
}

/**
 * Builds one figure tab: the row's markup, shown the way its `kind` calls for.
 *
 * All four kinds are built *before* the tab is shown, because the tab strip
 * needs their `actions` (svg / mermaid) in its own constructor; `mount` only
 * appends a node that already exists. That is what lets `svg` and `mermaid` sit
 * inside the result panel with no frame of their own — the panel is the frame —
 * and hand their controls to the strip rather than floating them over the
 * content, which is also why the file travels out as a payload instead of a
 * button. An embedded `<dfk-mermaid>` is driven from the outside through
 * `setFullscreen`; an `iframe` has no viewer and so no fullscreen interest, and
 * neither has a terminal frame — it is text that is already the size it wants.
 */
function createFigure(
  kind: PreviewKind,
  markup: string,
  label: string,
  config: RunnableSqlConfig,
  host: HTMLElement,
  figureLabels: FigureLabels,
): PreviewTabItem {
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
    return {
      label,
      mount: (panel) => {
        panel.appendChild(frame);
      },
      download: () => ({
        name: sectionFileName(host, 'html', {fallback: 'preview'}),
        mime: 'text/html;charset=utf-8',
        text: markup,
      }),
    };
  }

  if (kind === 'mermaid') {
    // The cell is handed to the kit's own diagram element rather than rendered
    // here: `<dfk-mermaid>` loads mermaid through the page-wide render queue, so
    // a `mermaid` result and a ```mermaid fence share one implementation — and
    // one place that knows about the dark-mode-first-load fix.
    //
    // `embedded` makes it shed its frame and its floating cluster; the source
    // travels as an attribute (the element's attribute seed), so this works
    // whether or not the element has been upgraded yet. Reading `actions` seeds
    // the element, which is why it is safe to ask before inserting it.
    const element = el('dfk-mermaid', {attrs: {embedded: '', source: markup}});
    return {
      label,
      actions: element.actions,
      mount: (panel) => {
        panel.appendChild(element);
      },
      download: () => element.downloadPayload(),
      setFullscreen: (value) => element.setFullscreen(value),
    };
  }

  if (kind === 'terminal') {
    // A captured terminal frame. It brings no controls, so the strip's right end
    // is left to the download button and the fullscreen toggle alone, and it has
    // nothing to zoom — which is also why it does not subscribe to the fullscreen
    // state below.
    const view = new TerminalView('terminal');
    applyPreviewSize(view.root, config);
    return {
      label,
      mount: async (panel) => {
        panel.appendChild(view.root);
        await view.render(markup);
      },
      download: () => view.downloadPayload(),
    };
  }

  const viewer = new SvgViewer(parseSvgMarkup(host.ownerDocument, markup), markup, figureLabels, 'figure');
  applyPreviewSize(viewer.root, config);
  return {
    label,
    actions: viewer.actions,
    mount: (panel) => {
      panel.appendChild(viewer.root);
      return () => viewer.dispose();
    },
    download: () => viewer.downloadPayload(),
    setFullscreen: (value) => viewer.setFullscreen(value),
  };
}

/**
 * Builds a preview renderer: one tab per row, then the raw rows in the trailing
 * `Table` tab. `iframe`, `svg` and `mermaid` share everything except how a
 * figure is built.
 */
function previewRenderer(kind: PreviewKind): Renderer {
  return async ({host, config, labels, fullscreenButton, onFullscreenChange}, result) => {
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

    // A figure zooms only in the result area's fullscreen, which the renderer
    // hears about through the subscription below.
    const figureLabels: FigureLabels = {
      reset: labels.resetZoom ?? 'Reset zoom',
      edit: labels.editSource ?? 'Edit source',
      editTitle: labels.svgSource ?? 'SVG source',
      apply: labels.apply ?? 'Apply',
      cancel: labels.cancel ?? 'Cancel',
    };
    const tabName = config.tab_name;
    const items: PreviewTabItem[] = result.rows.map((row, index) => {
      const value = tabName ? row[tabName] : undefined;
      const label = value === null || value === undefined ? `${labels.row ?? 'Row'} ${index + 1}` : stringify(value);
      return createFigure(kind, markupOf(row[field]), label, config, host, figureLabels);
    });
    items.push(tableItem(result, labels, host));

    const tabs = new PreviewTabs(host, items, labels.download ?? 'Download', fullscreenButton);
    // Only the kinds that *zoom inside* the result area care about its fullscreen
    // state; an `iframe` and a terminal frame have nothing to switch.
    const zooms = kind === 'svg' || kind === 'mermaid';
    const unsubscribe = zooms ? onFullscreenChange((value) => tabs.setFullscreen(value)) : null;
    return () => {
      unsubscribe?.();
      tabs.dispose();
    };
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
  mermaid: previewRenderer('mermaid'),
  terminal: previewRenderer('terminal'),
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