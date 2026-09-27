import type {QueryResult} from './runtime';
import type {RunnableSqlConfig} from './remark';

/**
 * Result renderers, keyed by the config's `show` field.
 *
 * The registry is the seam the later phases plug into: `html` (isolated
 * iframe), `svg` and the `Preview | Table` tab layout register new entries
 * here without touching `<dfk-sql>` itself. Phase 1 ships `table` (VisActor
 * VTable), plus a `text` fallback and an `error` view every renderer shares.
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
 * Renders `result` into `context.host`. Returns a disposer releasing any
 * resources (table instances) the renderer created; it runs on the next
 * render and when the element disconnects.
 */
export type Renderer = (
  context: RenderContext,
  result: QueryResult,
) => void | Promise<() => void>;

/**
 * Resolves a CSS custom property (with fallback) against an element. VTable
 * paints to a canvas, where `fillStyle: 'var(--ifm-…)'` would not resolve —
 * the theme colours have to become concrete strings before they go in.
 */
function cssColor(element: HTMLElement, property: string, fallback: string): string {
  const value = getComputedStyle(element).getPropertyValue(property).trim();
  return value || fallback;
}

/**
 * VTable renders into real DOM under the light DOM (its style-mod injects
 * global CSS, which a shadow boundary would not host); the component owns the
 * container and passes it in.
 */
const tableRenderer: Renderer = async ({host}, result) => {
  const {ListTable} = await import('@visactor/vtable');
  const record = host.ownerDocument.createElement('div');
  record.className = 'dfk-sql-table';
  // VTable is canvas-rendered and follows the container's box, so the height
  // must be an explicit pixel value — an `auto` container collapses to zero.
  record.style.height = `${Math.min(360, 60 + result.rows.length * 32)}px`;
  host.replaceChildren(record);

  const surface = cssColor(record, '--ifm-background-surface-color', '#fff');
  const text = cssColor(record, '--ifm-font-color-base', '#181818');
  const border = cssColor(record, '--ifm-global-border-color', '#e0e0e0');
  const headerBg = cssColor(record, '--ifm-color-emphasis-100', '#f5f5f5');

  const table = new ListTable({
    container: record,
    records: result.rows,
    columns: result.columns.map((field) => ({field, title: field})),
    // Follow the site's Infima palette, resolved to concrete colours.
    theme: {
      bodyStyle: {bgColor: surface, color: text, borderColor: border},
      headerStyle: {bgColor: headerBg, color: text, borderColor: border},
    },
  });

  return () => table.release();
};

/** Plain-text fallback: one line per row, columns tab-joined. */
const textRenderer: Renderer = ({host}, result) => {
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
export const errorRenderer: Renderer = ({host, labels}, result) => {
  const block = host.ownerDocument.createElement('pre');
  block.className = 'dfk-sql-error';
  block.textContent = `${labels.error ?? 'Error'}: ${result.error ?? 'unknown'}`;
  host.replaceChildren(block);
};

function stringify(value: unknown): string {
  if (value === null || value === undefined) {
    return 'NULL';
  }
  return typeof value === 'object' ? JSON.stringify(value) : String(value);
}

const registry: Record<string, Renderer> = {
  table: tableRenderer,
  text: textRenderer,
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
