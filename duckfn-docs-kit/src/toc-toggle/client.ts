/**
 * The TOC toggle's browser glue, injected by `toc-toggle/plugin` so a
 * consuming site does not keep a client module of its own. The behaviour lives
 * in `toc-toggle/TocToggle`; this module is only the Docusaurus lifecycle
 * glue: initialise once in the browser and refresh after every route change.
 *
 * Docusaurus evaluates this module in its Node prerender pass too, where there
 * is no DOM to touch — hence the guard.
 */
import {createTocToggle} from './TocToggle';

const toggle = createTocToggle();

if (typeof window !== 'undefined') {
  toggle.init();
}

export function onRouteDidUpdate(): void {
  toggle.refresh();
}
