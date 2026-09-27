/**
 * Adds a collapse/expand control to the desktop table of contents.
 *
 * The behaviour lives in duckfn-docs-kit (`toc-toggle/TocToggle` entry) so other
 * extension docs sites can reuse it; this file is only the Docusaurus
 * lifecycle glue: initialise once in the browser and refresh after every
 * route change. The matching CSS comes from
 * `duckfn-docs-kit/src/toc-toggle/TocToggle.css`, pulled in through
 * `src/css/custom.css`.
 */
import {createTocToggle} from 'duckfn-docs-kit/toc-toggle/TocToggle';

const toggle = createTocToggle();

// Client modules are also evaluated by Docusaurus' Node prerender pass, where
// there is no DOM to touch.
if (typeof window !== 'undefined') {
  toggle.init();
}

export function onRouteDidUpdate(): void {
  toggle.refresh();
}
