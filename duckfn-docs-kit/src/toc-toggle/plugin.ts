import {createRequire} from 'node:module';
import path from 'node:path';

/**
 * `duckfn-docs-kit/toc-toggle/plugin` — wires the kit's TOC collapse control
 * into a Docusaurus site.
 *
 * The button and its behaviour live in `toc-toggle/TocToggle` +
 * `TocToggle.css`; what a site used to hand-write as a client module is now
 * injected by this plugin, so a site's own `clientModules` only carries client
 * code the site itself owns.
 *
 * This is Node-side build code: it must not import any browser module, and the
 * browser side must not import this file (the glue it injects is
 * `./client.ts`, resolved through the package exports map).
 */

/**
 * The slice of Docusaurus' plugin API this module touches, typed structurally
 * (like `sql/extensions.ts`) so the kit keeps zero Docusaurus dependencies.
 */
export interface DfkTocToggleContext {
  siteDir: string;
}

export interface DfkTocTogglePlugin {
  name: string;
  getClientModules(): string[];
}

/** The plugin module Docusaurus calls with its `LoadContext` and options. */
export type DfkTocTogglePluginModule = (context: DfkTocToggleContext) => DfkTocTogglePlugin;

/**
 * Builds the plugin. Usage in `docusaurus.config.ts`:
 *
 * ```ts
 * import {dfkTocToggle} from 'duckfn-docs-kit/toc-toggle/plugin';
 *
 * plugins: [dfkTocToggle()],
 * ```
 */
export function dfkTocToggle(): DfkTocTogglePluginModule {
  return (context) => ({
    name: 'dfk-toc-toggle',
    getClientModules() {
      // Resolved from the consuming site, so the config bundler cannot break
      // the lookup; the exports map (`./*` -> dist) keeps the subpath valid
      // even if the entry moves.
      const requireFromSite = createRequire(path.join(context.siteDir, 'package.json'));
      return [requireFromSite.resolve('duckfn-docs-kit/toc-toggle/client')];
    },
  });
}
