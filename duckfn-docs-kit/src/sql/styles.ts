// The `<dfk-sql>` code box's shadow-root CSS as an inline string (Vite `?inline`
// import), mirroring `home/styles.ts`. It covers the CodeMirror editor and the
// action cluster, both of which live in this tree. Only the *result* container
// stays in the light DOM, styled by `sql.css` through `kit.css` instead: VTable
// injects a document-level stylesheet, which a shadow boundary could not host.
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
