// The home components' CSS as constructed stylesheets. Each file is a `?inline`
// import (see `vite.config.ts`), so a consuming site never imports these files
// directly — the components adopt the sheets into their shadow roots. The only
// CSS that stays global source is `theme/tokens.css` (the `--duckfn-*` palette,
// which must live on `:root` / `[data-theme]` for the custom properties to
// inherit into the shadow trees) and `toc-toggle/TocToggle.css` (light-DOM rules
// that have to sit inside `@layer docusaurus.theme-classic`).
//
// One sheet per component class: the rules are static, so they are parsed once
// and the same object serves every instance — the model Lit uses for
// `static styles`. A `<style>` node per instance would repeat both the text and
// the parse cost for every card on the page.
import baseCss from './base.css?inline';
import featureCss from './features.css?inline';
import heroCss from './hero.css?inline';
import nextStepsCss from './next-steps.css?inline';
import sectionCss from './section.css?inline';

/**
 * Builds the sheet on first use.
 *
 * Docusaurus imports this module into Node during prerendering, where
 * `CSSStyleSheet` does not exist — only the browser ever calls these getters.
 */
function lazySheet(css: string): () => CSSStyleSheet {
  let sheet: CSSStyleSheet | null = null;
  return () => {
    if (!sheet) {
      sheet = new CSSStyleSheet();
      sheet.replaceSync(css);
    }
    return sheet;
  };
}

/** `:host` + the box-sizing reset; adopted by all three home elements. */
export const baseStyles = lazySheet(baseCss);

/** The card band `<dfk-features>` and `<dfk-next-steps>` share. */
export const sectionStyles = lazySheet(sectionCss);

export const heroStyles = lazySheet(heroCss);

export const featureStyles = lazySheet(featureCss);

export const nextStepsStyles = lazySheet(nextStepsCss);
