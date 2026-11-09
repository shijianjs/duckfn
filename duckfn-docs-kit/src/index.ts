import type {DetailedHTMLProps, HTMLAttributes} from 'react';
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
 * Props every `dfk-*` tag accepts from React: the plain HTML ones plus `ref`.
 *
 * `DetailedHTMLProps` (rather than bare `HTMLAttributes`) is what puts `ref`
 * into the props. The alias is exported (rather than written inline) because
 * the emitted `.d.ts` for a module augmentation can only name types that are
 * themselves reachable from the declaration file; `key` comes from React's own
 * `IntrinsicAttributes` and needs no mention.
 */
export type DfkElementProps = DetailedHTMLProps<
  HTMLAttributes<HTMLElement>,
  HTMLElement
>;

/**
 * Content the home components read from their attributes.
 *
 * Scalars are plain text; structures are JSON. Each element parses them in
 * `attributeChangedCallback` and calls the matching setter, so React drives it
 * by diffing attributes — no ref and no mount helper. The names are kebab-case
 * because HTML lower-cases attribute names.
 */
export type DfkHeroProps = DfkElementProps & {
  'logo-src'?: string;
  title?: string;
  tagline?: string;
  /** JSON `HeroLink`. */
  primary?: string;
  /** JSON `HeroAction`. */
  secondary?: string;
  /** JSON `HeroBadge[]`. */
  badges?: string;
};

export type DfkFeaturesProps = DfkElementProps & {
  'section-title'?: string;
  /** JSON `FeatureItem[]`. */
  features?: string;
};

export type DfkNextStepsProps = DfkElementProps & {
  'section-title'?: string;
  /** JSON `NextStepItem[]`. */
  steps?: string;
};

/**
 * The child tags the home components read on connect.
 *
 * They are *not* registered custom elements — just data carriers that keep the
 * markup readable where JSON inside an attribute would not be (hand-written
 * HTML, MDX, which does not even accept single-quoted attribute values). Their
 * parent reads them once and renders its shadow tree; `kit.css` hides them so
 * the raw text never shows.
 */
export type DfkActionProps = DfkElementProps & {
  kind?: 'primary' | 'secondary';
  href?: string;
  icon?: string;
};

export type DfkBadgeProps = DfkElementProps & {
  href?: string;
  src?: string;
  alt?: string;
};

export type DfkFeatureProps = DfkElementProps & {
  icon?: string;
  title?: string;
};

export type DfkStepProps = DfkElementProps & {
  href?: string;
  title?: string;
};

/**
 * The kit's tags in the DOM's own tag map, so `document.createElement('dfk-sql')`
 * (and the kit's `el()` helper) is typed as the class it upgrades to. Without
 * this, every place that builds a `dfk-*` element from scratch — `sql/renderers.ts`
 * building a `<dfk-mermaid>` for a `mermaid` result, a site writing the home
 * tags — would have to cast the result.
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
      'dfk-hero': DfkHeroProps;
      'dfk-features': DfkFeaturesProps;
      'dfk-next-steps': DfkNextStepsProps;
      'dfk-sql': DfkElementProps;
      'dfk-mermaid': DfkElementProps;
      'dfk-action': DfkActionProps;
      'dfk-badge': DfkBadgeProps;
      'dfk-feature': DfkFeatureProps;
      'dfk-step': DfkStepProps;
    }
  }
}