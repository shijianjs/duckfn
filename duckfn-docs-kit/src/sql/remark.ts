import type {Plugin} from 'unified';

/**
 * Turns a fenced SQL block whose metastring is a JSON config with
 * `{"type":"duckfn", …}` into a `<dfk-sql>` custom element, so the docs site
 * can render it as a runnable example.
 *
 * The original `code` node is kept as the element's *child*: Docusaurus'
 * `codeCompatPlugin` still stamps `metastring` onto it and the classic theme
 * renders it as a regular `@theme/CodeBlock`, which `<dfk-sql>` slots in as the
 * static preview. The SQL text and the parsed config travel as string
 * attributes (`sql` / `config`) — React 19 reconciles string props onto custom
 * elements as attributes, so they survive prerendering and hydration.
 *
 * Unlike Docusaurus' own `key=value` metastring format, the config here is
 * JSON, which allows nested fields (`option: {…}`) for future renderers.
 *
 * This is Node-side build code: it must not touch `window` / `document`, and it
 * must not import any browser module (type-only imports are fine).
 */

/** The JSON payload written after the info string of a runnable SQL block. */
export interface RunnableSqlConfig {
  /** Marks the block as a duckfn runnable example; the only value today. */
  type: 'duckfn';
  /**
   * Which result renderer to use. Defaults to `table` at runtime, except that a
   * single-column single-row result degrades to `text` (a bare scalar reads
   * better as a line than as a 1×1 table).
   *
   * `html` and `iframe` are the same renderer: both sandbox the markup in an
   * iframe, so scripts run with an opaque origin.
   */
  show?: 'table' | 'html' | 'iframe' | 'svg' | 'text';
  /**
   * The column holding the markup, for the preview renderers. A single-column
   * result is unambiguous and is used as-is.
   */
  field?: string;
  /** The column to label each preview tab with; falls back to `Row N`. */
  tab_name?: string;
  /** Presentation knobs for the preview renderers; see `option.width` etc. */
  option?: {
    /** CSS length for the preview box (e.g. `'100%'`, `'640px'`). */
    width?: string;
    height?: string;
    /**
     * `sandbox` tokens for the `iframe` renderer, replacing the default
     * `allow-scripts`. Only set this to *widen* what the report may do — the
     * default deliberately omits `allow-same-origin`.
     */
    sandbox?: string;
  };
  /**
   * duckfn community extensions to `LOAD` before running the block. The kit
   * never hard-codes an extension name; the docs source names what it needs.
   */
  extensions?: string[];
  /** A repository serving the extensions, instead of the DuckDB default. */
  repository?: string;
  /**
   * Allows `LOAD` to accept extensions without a valid signature. Opt-in
   * per block, and only meaningful for the *first* block that initialises the
   * shared runtime — `open()` fixes it for the instance.
   */
  allowUnsignedExtensions?: boolean;
  /**
   * Forward-compatible fields: the remark plugin passes the whole object
   * through untouched, so a newer kit version can read new keys without the
   * docs source changing.
   */
  [key: string]: unknown;
}

export interface RunnableSqlOptions {
  /**
   * Reserved for the future extension-loading phase (which duckfn extension a
   * site wants preloaded). Unused in phase 1.
   */
  extensionName?: string;
}

/** The custom element the plugin emits; must match `register.ts`. */
export const DFK_SQL_TAG = 'dfk-sql';

interface CodeNode {
  type: string;
  lang?: string | null;
  meta?: string | null;
  value?: unknown;
  children?: unknown[];
}

interface ParentNode {
  children?: unknown[];
}

/** Parse the metastring; `null` means "not a runnable block, leave it alone". */
function parseConfig(meta: string | null | undefined): RunnableSqlConfig | null {
  if (!meta) {
    return null;
  }
  try {
    const parsed: unknown = JSON.parse(meta);
    if (
      typeof parsed === 'object' &&
      parsed !== null &&
      (parsed as {type?: unknown}).type === 'duckfn'
    ) {
      return parsed as RunnableSqlConfig;
    }
  } catch {
    // Not JSON (a plain `sql` block, or Docusaurus' `key=value` metastrings):
    // a normal code block, nothing to do.
  }
  return null;
}

function wrapRunnableSql(code: CodeNode, config: RunnableSqlConfig): Record<string, unknown> {
  return {
    type: 'mdxJsxFlowElement',
    name: DFK_SQL_TAG,
    attributes: [
      {type: 'mdxJsxAttribute', name: 'config', value: JSON.stringify(config)},
      {type: 'mdxJsxAttribute', name: 'sql', value: String(code.value ?? '')},
    ],
    // The code node stays a child so the classic theme still renders it as a
    // CodeBlock; `dfk-sql` slots it as the static preview.
    children: [code],
  };
}

export const remarkRunnableSql: Plugin<[RunnableSqlOptions?]> =
  (_options = {}) =>
  (tree) => {
    const walk = (node: unknown): void => {
      if (typeof node !== 'object' || node === null) {
        return;
      }
      const parent = node as ParentNode;
      if (!Array.isArray(parent.children)) {
        return;
      }
      parent.children = parent.children.map((child) => {
        if (typeof child !== 'object' || child === null) {
          return child;
        }
        const candidate = child as CodeNode;
        if (candidate.type === 'code' && candidate.lang === 'sql') {
          const config = parseConfig(candidate.meta);
          if (config) {
            return wrapRunnableSql(candidate, config);
          }
        }
        walk(candidate);
        return candidate;
      });
    };

    walk(tree);
  };
