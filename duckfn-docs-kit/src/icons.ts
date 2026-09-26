import type {IconifyIcon} from '@iconify/types';
import iconArrowRight from '@iconify-icons/lucide/arrow-right';
import iconBraces from '@iconify-icons/lucide/braces';
import iconHash from '@iconify-icons/lucide/hash';
import iconLifeBuoy from '@iconify-icons/lucide/life-buoy';
import iconPackage from '@iconify-icons/lucide/package';
import iconShieldCheck from '@iconify-icons/lucide/shield-check';
import iconSparkles from '@iconify-icons/lucide/sparkles';
import iconGithub from '@iconify-icons/simple-icons/github';

/**
 * The icon set the docs home page uses, re-exported from Iconify so no SVG path
 * is hand-maintained here. Lucide glyphs are stroked and the GitHub mark is
 * filled, but both paint with `currentColor`, which is what lets them be used
 * as a CSS mask (below) and still inherit the brand colour.
 */
export {
  iconArrowRight,
  iconBraces,
  iconGithub,
  iconHash,
  iconLifeBuoy,
  iconPackage,
  iconShieldCheck,
  iconSparkles,
};

/**
 * Wraps an icon's SVG body in a standalone document and returns it as a
 * `url("data:image/svg+xml,…")` value.
 *
 * This is the same technique the site already used for the TOC chevron. Note
 * what it is *not*: it builds an image URL, not page markup, so it stays within
 * the package's "no HTML-string assembly" rule. Inside the isolated SVG the
 * `currentColor` in the body resolves to black, and a mask keeps the alpha
 * channel — so the painted (stroked or filled) area becomes opaque and shows
 * whatever `background-color` the masked element carries.
 */
export function iconToDataUrl(icon: IconifyIcon): string {
  const width = icon.width ?? 24;
  const height = icon.height ?? 24;
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}">` +
    `${icon.body}</svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

/**
 * Attaches an icon to `element` as a CSS mask by setting the `--dfk-icon`
 * custom property. The matching `mask` / `background-color` declarations live
 * in `home.css` (`.dfk-icon`), which keeps the `-webkit-` prefix in CSS instead
 * of in JS.
 */
export function applyIconMask(element: HTMLElement, icon: IconifyIcon): void {
  element.style.setProperty('--dfk-icon', iconToDataUrl(icon));
}
