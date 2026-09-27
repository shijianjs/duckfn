// The `<dfk-sql>` toolbar's shadow-root CSS as an inline string (Vite `?inline`
// import), mirroring `home/styles.ts`. The editor/result containers live in the
// light DOM and are styled by `sql.css` through `kit.css` instead — CodeMirror
// and VTable inject their own global stylesheets, which a shadow boundary
// could not host.
//
// The type declaration lives in `vite-env.d.ts`; `tsc` never resolves the
// `?inline` suffix, so the module graph stays buildable without Vite running.
import sqlCss from './DfkSql.css?inline';

let sheet: CSSStyleSheet | null = null;

/**
 * The one parsed stylesheet shared by every `<dfk-sql>` shadow root.
 * Created lazily: `CSSStyleSheet` does not exist during Docusaurus' Node
 * prerender, and only the browser ever calls this.
 */
export function sqlStyles(): CSSStyleSheet {
  if (!sheet) {
    sheet = new CSSStyleSheet();
    sheet.replaceSync(sqlCss);
  }
  return sheet;
}
