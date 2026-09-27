// The home components' CSS as an inline string, bundled into the JS by Vite's
// `?inline` import (see `vite.config.ts`). The components inject it into their
// shadow roots, so a consuming site never imports `home.css` — the only CSS
// files that stay global source are `theme/tokens.css` (the `--duckfn-*`
// palette, which must live on `:root` / `[data-theme]`) and
// `toc-toggle/TocToggle.css` (light-DOM rules that have to sit inside
// `@layer docusaurus.theme-classic`).
//
// The type declaration lives in `vite-env.d.ts`; `tsc` never resolves the
// `?inline` suffix, so the module graph stays buildable without Vite running.
import homeCss from './home.css?inline';

let sheet: CSSStyleSheet | null = null;

/**
 * The one parsed stylesheet shared by every `dfk-*` shadow root.
 *
 * Created lazily: Docusaurus imports this module into Node during
 * prerendering, where `CSSStyleSheet` does not exist — only the browser ever
 * calls this.
 */
export function homeStyles(): CSSStyleSheet {
  if (!sheet) {
    sheet = new CSSStyleSheet();
    sheet.replaceSync(homeCss);
  }
  return sheet;
}
