/**
 * The kit's client bootstrap for pages that use the `dfk-*` elements,
 * injected by the `dfkExtensions` Docusaurus plugin on every page (through
 * `getClientModules()`) so a consuming site does not need a client-module file
 * of its own just to register the custom elements. The home page keeps its own
 * module-scope `registerDfkElements()` call (it imports the barrel anyway);
 * registering twice is a no-op.
 *
 * Docusaurus evaluates this module in its Node prerender pass too, where there
 * is nothing to register — hence the guard.
 */
import {registerDfkElements} from '../register';

if (typeof window !== 'undefined') {
  registerDfkElements();
}
