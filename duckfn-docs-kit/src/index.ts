import type {HTMLAttributes} from 'react';
import type {FeaturesData, HeroData, NextStepsData} from './types';

/**
 * Browser entry for duckfn-docs-kit: the home-page custom elements, the icon
 * set and the shared data types.
 *
 * Import `duckfn-docs-kit/remark` (Node build code) and `duckfn-docs-kit/toc-toggle`
 * from their own subpaths instead of here, so a Docusaurus config file never
 * pulls browser code into Node.
 */
export {DfkFeatures} from './elements/features';
export {DfkHero} from './elements/hero';
export {DfkNextSteps} from './elements/next-steps';
export {
  applyIconMask,
  iconArrowRight,
  iconBraces,
  iconGithub,
  iconHash,
  iconLifeBuoy,
  iconPackage,
  iconShieldCheck,
  iconSparkles,
  iconToDataUrl,
} from './icons';
export {registerDfkElements} from './register';
export type {
  FeatureItem,
  FeaturesData,
  HeroAction,
  HeroBadge,
  HeroData,
  HeroLink,
  NextStepItem,
  NextStepsData,
} from './types';

/**
 * Teach the JSX namespace about the custom elements. React 19 sets non-primitive
 * props on custom elements as *properties* (only strings become attributes), so
 * `data={{...}}` reaches the element's `data` setter directly.
 *
 * The props interface is exported (not written inline) because the emitted
 * `.d.ts` for a module augmentation can only name types that are themselves
 * reachable from the declaration file.
 */
export interface DfkElementProps<TData> extends HTMLAttributes<HTMLElement> {
  data?: TData;
}

declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'dfk-hero': DfkElementProps<HeroData>;
      'dfk-features': DfkElementProps<FeaturesData>;
      'dfk-next-steps': DfkElementProps<NextStepsData>;
    }
  }
}
