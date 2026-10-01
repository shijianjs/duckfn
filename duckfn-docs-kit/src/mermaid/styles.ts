// The `<dfk-mermaid>` shadow-root CSS as an inline string (Vite `?inline`
// import), mirroring `home/styles.ts` and `sql/styles.ts`. Everything the
// component draws lives in the shadow tree — the diagram included, which is what
// keeps mermaid's inline `<style>` (it ships one inside every SVG) scoped to this
// one component instead of leaking into the page or into a sibling diagram.
//
// The type declaration lives in `vite-env.d.ts`; `tsc` never resolves the
// `?inline` suffix, so the module graph stays buildable without Vite running.
import mermaidCss from './DfkMermaid.css?inline';

let sheet: CSSStyleSheet | null = null;

/**
 * The one parsed stylesheet shared by every `<dfk-mermaid>` shadow root.
 * Created lazily: `CSSStyleSheet` does not exist during Docusaurus' Node
 * prerender, and only the browser ever calls this.
 */
export function mermaidStyles(): CSSStyleSheet {
  if (!sheet) {
    sheet = new CSSStyleSheet();
    sheet.replaceSync(mermaidCss);
  }
  return sheet;
}