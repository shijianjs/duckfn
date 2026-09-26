import {DfkFeatures} from './elements/features';
import {DfkHero} from './elements/hero';
import {DfkNextSteps} from './elements/next-steps';

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
