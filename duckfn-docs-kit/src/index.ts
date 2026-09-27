import type {HTMLAttributes} from 'react';

/**
 * Browser entry for duckfn-docs-kit: the home-page custom elements and the
 * value types their `set*` methods accept.
 *
 * The components are retained-mode (build once, then mutate held nodes) and
 * expose named setters such as `setTitle()` / `setBadges()`. They render into
 * shadow roots and inject their own stylesheet, so the consuming site imports
 * only the global CSS the kit cannot host in a shadow (`css/kit.css`: brand
 * tokens + TOC toggle). Icons are rendered by the official `<iconify-icon>`
 * web component, which `registerDfkElements()` registers as a side effect, so
 * this package ships no icon data.
 *
 * `TocToggle` and the remark plugin keep their own subpaths
 * (`duckfn-docs-kit/TocToggle`, `duckfn-docs-kit/remark`) instead of being
 * merged here: a Docusaurus config file must never pull browser code into Node,
 * and a site that only wants the TOC collapse button should not pay for the
 * bundled `iconify-icon` that this entry carries.
 */
export {DfkFeatures} from './home/DfkFeatures';
export {DfkHero} from './home/DfkHero';
export {DfkNextSteps} from './home/DfkNextSteps';
export {registerDfkElements} from './register';
export type {
  FeatureItem,
  HeroAction,
  HeroBadge,
  HeroLink,
  NextStepItem,
} from './types';

/**
 * Props accepted by the `dfk-*` tags when they are created from React.
 *
 * The content is *not* passed as a prop — an object prop would never reach the
 * component through hydration (React only reconciles strings onto custom
 * elements). The caller mounts the element and drives it through its setters
 * from a callback ref, so all React needs to know about is `ref`.
 *
 * The alias is exported (rather than written inline) because the emitted `.d.ts`
 * for a module augmentation can only name types that are themselves reachable
 * from the declaration file.
 */
export type DfkElementProps = HTMLAttributes<HTMLElement>;

declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'dfk-hero': DfkElementProps;
      'dfk-features': DfkElementProps;
      'dfk-next-steps': DfkElementProps;
    }
  }
}