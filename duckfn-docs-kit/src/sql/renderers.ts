import type {ListTable} from '@visactor/vtable';
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

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

/**
 * Resolves a CSS custom property (with fallback) against an element. VTable
 * paints to a canvas, where `fillStyle: 'var(--ifm-…)'` would not resolve —
 * the theme colours have to become concrete strings before they go in.
 */
function cssColor(element: HTMLElement, property: string, fallback: string): string {
  const value = getComputedStyle(element).getPropertyValue(property).trim();
  return value || fallback;
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
 * Mounts a VTable list into `parent`, creating the `.dfk-sql-table` box itself.
 *
 * VTable is canvas-rendered and measures its container at construction time, so
 * the box gets an explicit height through a custom property (which the
 * fullscreen rule overrides by specificity rather than `!important`). A
 * `ResizeObserver` re-measures whenever that box changes shape — which is also
 * how the table follows the fullscreen toggle, without any resize plumbing
 * through the component.
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
  record.style.setProperty('--dfk-sql-table-height', `${Math.min(360, 60 + result.rows.length * 32)}px`);
  parent.appendChild(record);

  const surface = cssColor(record, '--ifm-background-surface-color', '#fff');
  const text = cssColor(record, '--ifm-font-color-base', '#181818');
  const border = cssColor(record, '--ifm-global-border-color', '#e0e0e0');
  const headerBg = cssColor(record, '--ifm-color-emphasis-100', '#f5f5f5');

  let table: ListTable;
  try {
    const {ListTable: ListTableCtor} = await import('@visactor/vtable');
    table = new ListTableCtor({
      container: record,
      records: result.rows,
      columns: result.columns.map((field) => ({field, title: field})),
      // Follow the site's Infima palette, resolved to concrete colours.
      theme: {
        bodyStyle: {bgColor: surface, color: text, borderColor: border},
        headerStyle: {bgColor: headerBg, color: text, borderColor: border},
      },
    });
  } catch (error) {
    record.appendChild(errorBlock(document, errorText(labels, messageOf(error))));
    return {dispose: () => {}};
  }

  const observer = new ResizeObserver(() => table.resize());
  observer.observe(record);
  return {
    dispose: () => {
      observer.disconnect();
      table.release();
    },
  };
}

const tableRenderer: Renderer = async ({host, labels}, result) => {
  host.replaceChildren();
  const handle = await mountTable(host, result, labels);
  return () => handle.dispose();
};

/** Plain-text fallback: one line per row, columns tab-joined. */
const textRenderer: Renderer = async ({host}, result) => {
  const pre = host.ownerDocument.createElement('pre');
  pre.className = 'dfk-sql-text';
  const lines = [result.columns.join('\t')];
  for (const row of result.rows) {
    lines.push(result.columns.map((c) => stringify(row[c])).join('\t'));
  }
  pre.textContent = lines.join('\n');
  host.replaceChildren(pre);
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
  return async ({host, config, labels}, result) => {
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

    const tabs = new PreviewTabs(host, items, labels.table ?? 'Table', (panel) =>
      mountTable(panel, result, labels),
    );
    return () => tabs.dispose();
  };
}

function stringify(value: unknown): string {
  if (value === null || value === undefined) {
    return 'NULL';
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