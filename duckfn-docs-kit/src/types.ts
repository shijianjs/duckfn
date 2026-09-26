/**
 * Data contracts for the home-page web components.
 *
 * Every field is a plain string for the *current* locale: a custom element
 * cannot render Docusaurus' React `<Translate>`, so the docs site resolves the
 * strings with the imperative `translate()` API and hands them in through the
 * component's `data` property. Internal `href`s are already baseUrl-resolved by
 * the caller (`useBaseUrl`), because a raw `<a href>` inside a custom element
 * gets no Docusaurus prefixing.
 *
 * Icon fields are Iconify icon *names* (e.g. `lucide:arrow-right`), rendered by
 * the official `<iconify-icon>` web component, which fetches the glyph from the
 * public Iconify API. The package ships no icon data of its own.
 */

/** A hero call-to-action. `external` adds `target=_blank` + `rel` for the link. */
export interface HeroLink {
  label: string;
  href: string;
  external?: boolean;
}

/** The secondary hero action (the GitHub button), which also carries a glyph. */
export interface HeroAction extends HeroLink {
  /** Iconify icon name, e.g. `simple-icons:github`. */
  icon: string;
}

/** A shields.io-style badge in the hero row. Always an external link. */
export interface HeroBadge {
  href: string;
  src: string;
  alt: string;
}

export interface HeroData {
  /** Already baseUrl-resolved, e.g. `/duckfn/img/duckfn-logo.svg`. */
  logoSrc: string;
  title: string;
  tagline: string;
  primary: HeroLink;
  secondary: HeroAction;
  badges: HeroBadge[];
}

export interface FeatureItem {
  /** Iconify icon name, e.g. `lucide:sparkles`. */
  icon: string;
  title: string;
  details: string;
}

export interface FeaturesData {
  sectionTitle: string;
  items: FeatureItem[];
}

export interface NextStepItem {
  /** Already baseUrl-resolved internal path. */
  href: string;
  title: string;
  details: string;
}

export interface NextStepsData {
  sectionTitle: string;
  items: NextStepItem[];
}
