// Side-effect import: registers the official `<iconify-icon>` custom element,
// which the `dfk-*` components use for every glyph. It loads icon data on
// demand from the public Iconify API, so this package ships no icon sets.
import 'iconify-icon';
import {DfkFeatures} from './elements/DfkFeatures';
import {DfkHero} from './elements/DfkHero';
import {DfkNextSteps} from './elements/DfkNextSteps';

const TAGS = {
  hero: 'dfk-hero',
  features: 'dfk-features',
  nextSteps: 'dfk-next-steps',
} as const;

/**
 * Defines the `dfk-*` custom elements. Idempotent and SSR-safe: it is a no-op
 * outside the browser, and re-running it never throws
 * "already been registered with the custom element registry".
 *
 * Call it once at module scope in the docs site (and anywhere else that uses
 * the components); importing the classes alone does not register anything.
 */
export function registerDfkElements(): void {
  if (typeof customElements === 'undefined') {
    return;
  }
  for (const [name, ctor] of Object.entries({
    [TAGS.hero]: DfkHero,
    [TAGS.features]: DfkFeatures,
    [TAGS.nextSteps]: DfkNextSteps,
  })) {
    if (!customElements.get(name)) {
      customElements.define(name, ctor);
    }
  }
}
