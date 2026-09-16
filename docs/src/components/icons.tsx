/**
 * Inline SVG glyphs used by the home page.
 *
 * Why hand-written SVG instead of an icon package or emoji:
 *
 * - no new dependency, and no extra network request on the landing page;
 * - `stroke: currentColor` lets a glyph inherit the brand colour from whatever
 *   chip or card it sits in, which is what keeps light and dark mode consistent;
 * - emoji render as a different picture on every operating system.
 *
 * The paths are Lucide icons (https://lucide.dev, ISC licensed), which are drawn
 * on a 24x24 grid with a 1.75px round stroke — that is why every glyph below
 * shares the attributes set in `IconBase`. The GitHub mark is the Simple Icons
 * one (CC0), drawn filled instead of stroked, so it does not go through
 * `IconBase`.
 *
 * Sizing is the consumer's job: the glyphs carry width/height attributes only as
 * a fallback, so a CSS class with `width`/`height` controls them.
 */
import type {ReactNode} from 'react';

export type IconProps = {
  /** Applied to the <svg>; used to size the glyph and pick up a colour. */
  className?: string;
};

type IconBaseProps = IconProps & {
  children: ReactNode;
};

function IconBase({className, children}: IconBaseProps): ReactNode {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      width="24"
      height="24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false">
      {children}
    </svg>
  );
}

/** Attribute-driven registration: the `#[...]` syntax. */
export function HashIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <path d="M4 9h16" />
      <path d="M4 15h16" />
      <path d="M10 3 8 21" />
      <path d="M16 3l-2 18" />
    </IconBase>
  );
}

/** Panic-safe: a panic is caught before it reaches DuckDB. */
export function LifeBuoyIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <circle cx="12" cy="12" r="10" />
      <path d="m4.93 4.93 4.24 4.24" />
      <path d="m14.83 9.17 4.24-4.24" />
      <path d="m14.83 14.83 4.24 4.24" />
      <path d="m9.17 14.83-4.24 4.24" />
      <circle cx="12" cy="12" r="4" />
    </IconBase>
  );
}

/** Nested types: LIST, MAP, ARRAY and STRUCT nest like the Rust types do. */
export function BracesIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <path d="M8 3H7a2 2 0 0 0-2 2v5a2 2 0 0 1-2 2 2 2 0 0 1 2 2v5c0 1.1.9 2 2 2h1" />
      <path d="M16 21h1a2 2 0 0 0 2-2v-5c0-1.1.9-2 2-2a2 2 0 0 1-2-2V5a2 2 0 0 0-2-2h-1" />
    </IconBase>
  );
}

/** No C/C++ glue code: the attribute does the translating. */
export function SparklesIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" />
      <path d="M20 3v4" />
      <path d="M22 5h-4" />
      <path d="M4 17v2" />
      <path d="M5 18H3" />
    </IconBase>
  );
}

/** No local DuckDB build: headers in, nothing linked. */
export function PackageIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <path d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z" />
      <path d="M12 22V12" />
      <path d="m3.3 7 7.703 4.734a2 2 0 0 0 1.994 0L20.7 7" />
    </IconBase>
  );
}

/** Safe by default. */
export function ShieldCheckIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />
      <path d="m9 12 2 2 4-4" />
    </IconBase>
  );
}

/** Used by the "where to go next" cards and the showcase link. */
export function ArrowRightIcon({className}: IconProps): ReactNode {
  return (
    <IconBase className={className}>
      <path d="M5 12h14" />
      <path d="m12 5 7 7-7 7" />
    </IconBase>
  );
}

/** The GitHub mark next to the GitHub button. Filled, not stroked. */
export function GitHubIcon({className}: IconProps): ReactNode {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      width="20"
      height="20"
      fill="currentColor"
      aria-hidden="true"
      focusable="false">
      <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" />
    </svg>
  );
}
