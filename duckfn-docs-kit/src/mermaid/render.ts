/**
 * The one place the kit talks to mermaid: the load, the render queue, the config
 * the renderer is initialised with, and the two document-level facts a diagram
 * needs (which colour mode the page is in, and how a rendered SVG gets into a
 * shadow tree).
 *
 * Two upstream problems are solved here, and neither can be fixed from a site's
 * `docusaurus.config.ts` or from the diagram source:
 *
 * 1. **The flash on a dark-mode first load, and the blank diagram.** Docusaurus'
 *    `useColorMode()` deliberately lags behind on the first client render (its
 *    state is initialised in an effect, to avoid hydration mismatches), so a
 *    theme-aware renderer paints once with the light palette and then again with
 *    the dark one: the light one lands on screen briefly (the flash), and the two
 *    overlap inside mermaid's mutable singleton, which can resolve with an empty
 *    SVG (an empty container, no error — facebook/docusaurus#8357). This module
 *    reads `<html data-theme>` instead: the attribute the inline script in
 *    `<head>` writes *before* the first paint, and the one the page CSS keys off.
 *    Nothing is rendered until it is known, so each diagram is rendered exactly
 *    once per mode, in the mode the page is really in.
 * 2. **Concurrent renders.** Mermaid is a mutable singleton that cannot render
 *    two diagrams at once (`mermaid.initialize()` sets one global config, and the
 *    renderer mutates it as it goes): docusaurus#8357 asks to "render them
 *    sequentially one after the other", pointing at mermaid-js/mermaid#3577.
 *    Every render therefore goes through a single queue.
 *
 * This is browser-only code: `mermaid` and every DOM API are reached lazily, so
 * Docusaurus' Node prerender can import the module graph without evaluating any
 * of it.
 */

import type {DfkMermaidConfig, MermaidColorMode} from './config';

/** The mermaid singleton, exactly as the package's default export types it. */
type Mermaid = (typeof import('mermaid'))['default'];
type RenderResult = Awaited<ReturnType<Mermaid['render']>>;
type MermaidConfig = Parameters<Mermaid['initialize']>[0];

/**
 * One queue for the whole page: mermaid's `render()` calls are serialised,
 * never concurrent. A rejected task must not poison the queue, hence the
 * `catch` on the tail that the next task chains from.
 */
let queue: Promise<unknown> = Promise.resolve();

function enqueue<T>(task: () => Promise<T>): Promise<T> {
  const result = queue.then(task, task);
  queue = result.catch(() => undefined);
  return result;
}

let mermaidModule: Promise<Mermaid> | null = null;

/**
 * Loads (once) and returns the mermaid singleton. A dynamic `import()` so a page
 * with no diagram never downloads it, and the Node prerender never evaluates it.
 */
function loadMermaid(): Promise<Mermaid> {
  mermaidModule ??= import('mermaid').then((module) => module.default);
  return mermaidModule;
}

/** Keeps the ids mermaid is handed unique across every diagram on a page. */
let sequence = 0;

/** How a diagram is rendered: its source, its config, and its colour mode. */
export interface MermaidRenderRequest {
  source: string;
  config: DfkMermaidConfig;
  colorMode: MermaidColorMode;
}

/** A rendered diagram: the SVG markup, plus mermaid's bind hook for click handlers. */
export interface MermaidRenderOutput {
  svg: string;
  bind?(container: Element): void;
}

/**
 * Renders one diagram, through the page-wide queue. Each render re-initialises
 * mermaid with this diagram's config first: mermaid has two config levels and the
 * site-wide one can only be set through `initialize()`.
 */
export function renderMermaid({
  source,
  config,
  colorMode,
}: MermaidRenderRequest): Promise<MermaidRenderOutput> {
  const id = `dfk-mermaid-svg-${(sequence += 1)}`;
  return enqueue(async () => {
    const mermaid = await loadMermaid();
    const options: MermaidConfig = {
      startOnLoad: false,
      ...config.options,
      // `DfkMermaidConfig.theme` is a plain string on purpose — the kit does not
      // pin mermaid's theme list (the shared config must stay dependency-free,
      // and a site may name a theme a newer mermaid adds). This is the one place
      // the value meets mermaid's own union, so the narrowing happens here.
      theme: config.theme[colorMode] as MermaidConfig['theme'],
    };
    mermaid.initialize(options);
    try {
      const result: RenderResult = await mermaid.render(id, source);
      return {svg: result.svg, bind: result.bindFunctions};
    } catch (error) {
      // Mermaid leaves a stray SVG/message in the DOM on error
      // (https://github.com/mermaid-js/mermaid/issues/3205).
      document.querySelector(`#d${id}`)?.remove();
      throw error;
    }
  });
}

/** The colour mode the page is actually in, read from the pre-paint attribute. */
export function documentColorMode(): MermaidColorMode {
  return document.documentElement.getAttribute('data-theme') === 'dark' ? 'dark' : 'light';
}

/**
 * Calls `listener` whenever the page's colour mode changes, and returns the
 * unsubscribe. The `<head>` script only *writes* the attribute on load, so a
 * mode switch made later has to be observed — that is how a diagram follows the
 * theme toggle.
 */
export function watchColorMode(listener: () => void): () => void {
  const observer = new MutationObserver(listener);
  observer.observe(document.documentElement, {attributeFilter: ['data-theme']});
  return () => observer.disconnect();
}

/**
 * Turns mermaid's SVG markup into a node a shadow tree can host.
 *
 * Mermaid serialises through `innerHTML`, so its output is *HTML*, not strict
 * XML: an HTML label can carry a bare `<br>`, which `image/svg+xml` would reject.
 * Parsing as `text/html` and taking the `<svg>` element gives the right
 * namespace for free (the HTML parser enters foreign content for `<svg>`) and
 * keeps markup out of `innerHTML`, which the kit's conventions rule out.
 *
 * Returns `null` when there is no `<svg>` to be found, so the caller can show an
 * error instead of an empty box.
 */
export function parseMermaidSvg(owner: Document, svg: string): SVGElement | null {
  const parsed = new DOMParser().parseFromString(svg, 'text/html');
  const root = parsed.body.querySelector('svg');
  return root ? (owner.importNode(root, true) as unknown as SVGElement) : null;
}

/** The pan/zoom library, loaded once on first use (see `DfkMermaid`). */
export async function loadPanzoom(): Promise<typeof import('@panzoom/panzoom')['default']> {
  const module = await import('@panzoom/panzoom');
  return module.default;
}