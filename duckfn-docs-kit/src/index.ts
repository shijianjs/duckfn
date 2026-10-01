import type {HTMLAttributes} from 'react';
// Imported as well as re-exported: the `HTMLElementTagNameMap` augmentation below
// names the classes, and a module augmentation can only refer to local bindings.
import {DfkFeatures} from './home/DfkFeatures';
import {DfkHero} from './home/DfkHero';
import {DfkMermaid} from './mermaid/DfkMermaid';
import {DfkNextSteps} from './home/DfkNextSteps';
import {DfkSql} from './sql/DfkSql';

/**
 * Browser entry for duckfn-docs-kit: the custom elements it registers and the
 * value types their `set*` methods accept.
 *
 * The components are retained-mode (build once, then mutate held nodes) and
 * expose named setters such as `setTitle()` / `setBadges()`. They render into
 * shadow roots and inject their own stylesheet, so the consuming site imports
 * only the global CSS the kit cannot host in a shadow (`src/kit.css`: brand
 * tokens + TOC toggle, imported as `duckfn-docs-kit/src/kit.css`). Icons are
 * rendered by the official `<iconify-icon>`
 * web component, which `registerDfkElements()` registers as a side effect, so
 * this package ships no icon data.
 *
 * `TocToggle` and the remark plugins keep their own subpaths
 * (`duckfn-docs-kit/toc-toggle/TocToggle`, `duckfn-docs-kit/remark`,
 * `duckfn-docs-kit/sql/remark`, `duckfn-docs-kit/mermaid/remark`) instead of
 * being merged here: a Docusaurus config file must never pull browser code into
 * Node, and a site that only wants the TOC collapse button should not pay for
 * the bundled `iconify-icon`.
 */
export {DfkFeatures, DfkHero, DfkMermaid, DfkNextSteps, DfkSql};
export {registerDfkElements} from './register';
export type {DfkMermaidConfig, DfkMermaidConfigInput} from './mermaid/config';
export type {RunnableSqlConfig} from './sql/remark';
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

/**
 * The kit's tags in the DOM's own tag map, so `document.createElement('dfk-sql')`
 * (and the kit's `el()` helper) is typed as the class it upgrades to. Without
 * this, every place that builds a `dfk-*` element from scratch — `sql/renderers.ts`
 * building a `<dfk-mermaid>` for a `mermaid` result, `docs/src/pages/index.tsx`
 * mounting the home elements — would have to cast the result.
 */
declare global {
  interface HTMLElementTagNameMap {
    'dfk-hero': DfkHero;
    'dfk-features': DfkFeatures;
    'dfk-next-steps': DfkNextSteps;
    'dfk-sql': DfkSql;
    'dfk-mermaid': DfkMermaid;
  }
}

declare module 'react' {
  namespace JSX {
    interface IntrinsicElements {
      'dfk-hero': DfkElementProps;
      'dfk-features': DfkElementProps;
      'dfk-next-steps': DfkElementProps;
      'dfk-sql': DfkElementProps;
      'dfk-mermaid': DfkElementProps;
    }
  }
}