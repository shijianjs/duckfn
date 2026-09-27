/**
 * Registers the `dfk-*` custom elements (including `<dfk-sql>`) on every page.
 *
 * The home page already calls `registerDfkElements()` at module scope, but the
 * runnable SQL blocks live inside docs pages, whose React tree never imports
 * the kit — so registration has to happen once per app boot from a client
 * module. `registerDfkElements()` is idempotent and a no-op during Docusaurus'
 * Node prerender pass, and route changes do not re-register anything (custom
 * element definitions are global and permanent), so no `onRouteDidUpdate` hook
 * is needed here.
 */
import {registerDfkElements} from 'duckfn-docs-kit';

if (typeof window !== 'undefined') {
  registerDfkElements();
}
