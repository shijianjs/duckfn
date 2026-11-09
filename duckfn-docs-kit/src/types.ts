/**
 * Value types the home-page web components accept.
 *
 * These are the shapes the `set*` methods take and the shapes the JSON
 * attributes parse into (`badges`, `features`, `steps`, `primary`,
 * `secondary`). Markup written as child tags produces the same shapes.
 *
 * Text is whatever the caller resolved: a raw tag in Markdown carries the
 * page's own copy, while a React page hands in strings it resolved for the
 * current locale. Internal `href`s stay baseUrl-resolved by the caller
 * (`useBaseUrl`), because a raw `<a href>` inside a custom element gets no
 * Docusaurus prefixing.
 *
 * Icon fields are Iconify icon *names* (e.g. `lucide:arrow-right`), rendered by
 * the official `<iconify-icon>` web component, which fetches the glyph from the
 * public Iconify API. This package ships no icon data of its own.
 */

/** A hero call-to-action. */
export interface HeroLink {
  label: string;
  href: string;
}

/** The secondary hero action (the GitHub button), optionally with a glyph. */
export interface HeroAction extends HeroLink {
  /** Iconify icon name, e.g. `simple-icons:github`. */
  icon?: string;
}

/** A shields.io-style badge in the hero row. Always an external link. */
export interface HeroBadge {
  href: string;
  src: string;
  alt: string;
}

export interface FeatureItem {
  /** Iconify icon name, e.g. `lucide:sparkles`. */
  icon: string;
  title: string;
  details: string;
}

export interface NextStepItem {
  /** Already baseUrl-resolved internal path. */
  href: string;
  title: string;
  details: string;
}