/**
 * Inline SVG glyphs used by the home page.
 *
 * The icon data comes from Iconify, so nothing here is hand-maintained: each
 * glyph is a small `IconifyIcon` object shipped by an `@iconify-icons/*`
 * package. `buildIcon()` turns that data into SVG attributes plus an inner body
 * synchronously, so the glyph renders into the static HTML at build time — no
 * request to the Iconify API at runtime, and no hydration flash. Importing the
 * per-icon file (rather than a whole icon set) keeps the landing page bundle to
 * just these glyphs.
 *
 * Colour stays `currentColor`: the Lucide data strokes with `currentColor` and
 * the Simple Icons GitHub mark fills with it, so a glyph still inherits the
 * brand colour from whatever chip or card it sits in, keeping light and dark
 * mode consistent.
 *
 * Sizing is the consumer's job: the `width`/`height` attributes are only a
 * fallback, so a CSS class with `width`/`height` controls the glyph.
 */
import type {ReactNode} from 'react';

import {buildIcon} from '@iconify/react';
import type {IconifyIcon} from '@iconify/types';
import arrowRight from '@iconify-icons/lucide/arrow-right';
import braces from '@iconify-icons/lucide/braces';
import hash from '@iconify-icons/lucide/hash';
import lifeBuoy from '@iconify-icons/lucide/life-buoy';
import packageIcon from '@iconify-icons/lucide/package';
import shieldCheck from '@iconify-icons/lucide/shield-check';
import sparkles from '@iconify-icons/lucide/sparkles';
import github from '@iconify-icons/simple-icons/github';

export type IconProps = {
  /** Applied to the <svg>; used to size the glyph and pick up a colour. */
  className?: string;
};

function Glyph({icon, className}: {icon: IconifyIcon; className?: string}): ReactNode {
  const {attributes, body} = buildIcon(icon);
  return (
    <svg
      className={className}
      {...attributes}
      aria-hidden="true"
      focusable="false"
      dangerouslySetInnerHTML={{__html: body}}
    />
  );
}

/** Attribute-driven registration: the `#[...]` syntax. */
export function HashIcon({className}: IconProps): ReactNode {
  return <Glyph icon={hash} className={className} />;
}

/** Panic-safe: a panic is caught before it reaches DuckDB. */
export function LifeBuoyIcon({className}: IconProps): ReactNode {
  return <Glyph icon={lifeBuoy} className={className} />;
}

/** Nested types: LIST, MAP, ARRAY and STRUCT nest like the Rust types do. */
export function BracesIcon({className}: IconProps): ReactNode {
  return <Glyph icon={braces} className={className} />;
}

/** No C/C++ glue code: the attribute does the translating. */
export function SparklesIcon({className}: IconProps): ReactNode {
  return <Glyph icon={sparkles} className={className} />;
}

/** No local DuckDB build: headers in, nothing linked. */
export function PackageIcon({className}: IconProps): ReactNode {
  return <Glyph icon={packageIcon} className={className} />;
}

/** Safe by default. */
export function ShieldCheckIcon({className}: IconProps): ReactNode {
  return <Glyph icon={shieldCheck} className={className} />;
}

/** Used by the "where to go next" cards and the showcase link. */
export function ArrowRightIcon({className}: IconProps): ReactNode {
  return <Glyph icon={arrowRight} className={className} />;
}

/** The GitHub mark next to the GitHub button. */
export function GitHubIcon({className}: IconProps): ReactNode {
  return <Glyph icon={github} className={className} />;
}
