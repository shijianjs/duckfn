/**
 * The mermaid look and palette, shared by the Node-side remark plugin (which
 * stamps it onto every `<dfk-mermaid>` it emits) and the browser element (whose
 * fallback when no config travels with the element).
 *
 * Pure data with no imports, like `sql/runtimeConfig.ts`: both sides have to
 * agree on the shape, and neither may drag the other into its bundle — the
 * remark plugin runs in Docusaurus' Node build, the element in the browser.
 *
 * Why the config travels per element instead of living in the component: the
 * palette is the one part of a diagram that is *site-specific*. Passing it
 * through the site's `docusaurus.config.ts` (as an option of `remarkMermaid`)
 * keeps that choice where the rest of the site's theme is decided, and keeps a
 * downstream docs site from having to fork the kit to change two colour names.
 */

/** The two colour modes a diagram is rendered for. */
export type MermaidColorMode = 'light' | 'dark';

/** A partial config, as a site writes it. */
export interface DfkMermaidConfigInput {
  /** Mermaid theme name per colour mode; missing sides fall back to the default. */
  theme?: Partial<Record<MermaidColorMode, string>>;
  /** Extra mermaid options, spread into `initialize()` after `theme`. */
  options?: Record<string, unknown>;
}

/** A resolved config: both themes present, options ready to spread. */
export interface DfkMermaidConfig {
  theme: Record<MermaidColorMode, string>;
  options: Record<string, unknown>;
}

/**
 * The kit's default: the `neo` look (mermaid's flatter, rounder chrome) with the
 * redux palette — `redux-color` in light mode, `redux-dark-color` in dark.
 *
 * `look` has no per-mode counterpart in mermaid, so it belongs in `options`;
 * `theme` is the per-mode one. Both are easy to get wrong *silently*: mermaid
 * ignores an unrecognised value and falls back, so a change is verified in a
 * browser, not by a build.
 */
export const DEFAULT_MERMAID_CONFIG: DfkMermaidConfig = {
  theme: {light: 'redux-color', dark: 'redux-dark-color'},
  options: {look: 'neo'},
};

/** Merges a site's overrides over {@link DEFAULT_MERMAID_CONFIG}. */
export function resolveMermaidConfig(input?: DfkMermaidConfigInput | null): DfkMermaidConfig {
  return {
    theme: {
      light: input?.theme?.light ?? DEFAULT_MERMAID_CONFIG.theme.light,
      dark: input?.theme?.dark ?? DEFAULT_MERMAID_CONFIG.theme.dark,
    },
    options: {...DEFAULT_MERMAID_CONFIG.options, ...input?.options},
  };
}

/**
 * Parses the `config` attribute the remark plugin writes. Anything unreadable
 * falls back to the default rather than failing a diagram, and only the two
 * known keys are read — the attribute is content, not a channel for arbitrary
 * configuration.
 */
export function parseMermaidConfig(raw: string | null | undefined): DfkMermaidConfig {
  if (!raw) {
    return resolveMermaidConfig();
  }
  try {
    return resolveMermaidConfig(JSON.parse(raw) as DfkMermaidConfigInput);
  } catch {
    return resolveMermaidConfig();
  }
}