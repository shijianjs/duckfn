import type {Plugin} from 'unified';
import type {DfkMermaidConfigInput} from './config';

/**
 * Turns a ```mermaid fence into a `<dfk-mermaid>` custom element, so the docs
 * site renders diagrams through this kit instead of through
 * `@docusaurus/theme-mermaid`.
 *
 * That theme's React component cannot avoid the two upstream defects
 * `./render.ts` documents — it colours by `useColorMode()`, which lags behind on
 * the first client render, so a dark-mode first load paints a light diagram and
 * then a dark one (the flash, and occasionally an empty SVG), and mermaid's
 * mutable singleton renders two diagrams at once. Doing the render in a custom
 * element instead puts both fixes in one place that every duckfn-family docs site
 * shares, and lets the runnable-SQL `mermaid` output reuse the same renderer.
 *
 * The source travels as the `source` attribute and the palette as the `config`
 * one (see `./config`): React 19 reconciles string props onto a custom element as
 * attributes, so both survive prerendering and hydration. The element has no
 * children — the diagram is built in its shadow root, so there is no prerendered
 * markup to hide (contrast `sql/remark.ts`, whose code node is the fallback the
 * editor replaces).
 *
 * This is Node-side build code: it must not touch `window` / `document`, and it
 * must not import any browser module (type-only imports are fine).
 */

/** The custom element the plugin emits; must match `register.ts`. */
export const DFK_MERMAID_TAG = 'dfk-mermaid';

export interface RemarkMermaidOptions {
  /**
   * Overrides for the kit's default look and palette, merged by the element (see
   * `resolveMermaidConfig`). This is where a site picks its mermaid colours — the
   * one part of a diagram that is site-specific — so no docs site has to fork the
   * kit to change two theme names.
   *
   * Omitted, no `config` attribute is written at all and every element falls back
   * to the kit's default (`DEFAULT_MERMAID_CONFIG` in `./config`).
   */
  config?: DfkMermaidConfigInput;
}

interface CodeNode {
  type: string;
  lang?: string | null;
  value?: unknown;
}

interface ParentNode {
  children?: unknown[];
}

export const remarkMermaid: Plugin<[RemarkMermaidOptions?]> =
  (options = {}) =>
  (tree) => {
    // One JSON string for the whole page rather than one per fence: the attribute
    // is identical everywhere, and building it once keeps the walk cheap.
    const config = options.config === undefined ? null : JSON.stringify(options.config);

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
        if (candidate.type === 'code' && candidate.lang === 'mermaid') {
          return wrapMermaid(candidate, config);
        }
        walk(candidate);
        return candidate;
      });
    };

    walk(tree);
  };

function wrapMermaid(code: CodeNode, config: string | null): Record<string, unknown> {
  const attributes = [
    {type: 'mdxJsxAttribute', name: 'source', value: String(code.value ?? '')},
  ];
  if (config !== null) {
    attributes.push({type: 'mdxJsxAttribute', name: 'config', value: config});
  }
  return {
    type: 'mdxJsxFlowElement',
    name: DFK_MERMAID_TAG,
    attributes,
    children: [],
  };
}