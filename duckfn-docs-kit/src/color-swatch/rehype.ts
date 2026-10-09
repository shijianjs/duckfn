import a11yPlugin from 'colord/plugins/a11y';
import {colord, extend} from 'colord';
import type {Plugin} from 'unified';

/**
 * Paints inline code that *is* a colour value with that colour, so a palette
 * table or a theme's colour list reads as colour instead of as a column of hex
 * codes.
 *
 * Infima draws inline code as a grey tag; `#E69F00` therefore looks the same as
 * `x_axis.wrap`. This plugin instead puts the colour behind the text, with a
 * black or white foreground picked by **contrast** (WCAG, through
 * `colord/plugins/a11y`), so the swatch reads the same in both colour modes.
 * Nothing is added to the page's CSS: the two inline styles are the whole
 * effect, so a swatch is right even on a site that never imported the kit's
 * stylesheet, and the border, radius and padding stay Infima's.
 *
 * Two ways in:
 *
 * 1. **Scanning** (default on): any inline code whose whole content parses as a
 *    CSS colour is painted. Parsing and the colour maths are `colord`'s —
 *    hex (3 / 4 / 6 / 8 digits), `rgb()` / `rgba()`, `hsl()` / `hsla()`, and
 *    whatever else it parses out of the box — and nothing here re-implements a
 *    colour parser. CSS colour *names* are **not** accepted anywhere, including
 *    on a marked element: they need `colord/plugins/names`, which is not loaded,
 *    because the scan cannot tell `red` the colour from `red` the word. Write
 *    hex, `rgb()` or `hsl()` instead.
 * 2. **Explicitly**: `<code data-color-swatch>#E69F00</code>` (the content is
 *    the colour — useful with `scan: false`), or
 *    `<code data-color-swatch="#E69F00">any text</code>` to paint something that
 *    is not itself a colour value. The author's own `class` / `style` are kept;
 *    the swatch is appended to them.
 *
 * **Why rehype, and why not `{.color-swatch}`.** Both halves are forced by MDX,
 * which is what Docusaurus compiles a page with:
 *
 * - MDX tokenises `{...}` as a JavaScript expression **before** any remark
 *   plugin runs, so `` `#E69F00`{.color-swatch} `` fails the build
 *   (`Could not parse expression with acorn`) and `` `x`{color="#E69F00"} ``
 *   compiles but throws `ReferenceError: color is not defined` in the browser.
 *   `<code data-color-swatch="…">` is the MDX-native spelling of the same idea.
 * - A `style` written onto a **JSX element** (what an `<code>` in Markdown
 *   becomes) is a React prop, and React rejects a string there
 *   (`The 'style' prop expects a mapping from style properties to values`).
 *   Running in the rehype phase instead lets the plugin set the `style`
 *   *attribute* on the finished HTML element, which is what it is.
 *
 * This is Node-side build code: it must not touch `window` / `document`, and it
 * must not import any browser module (type-only imports are fine).
 */

/** The class every painted inline code gets, so a site can restyle it in CSS. */
export const COLOR_SWATCH_CLASS = 'dfk-color-swatch';

/**
 * Marks an inline code explicitly — see the module docs. With a value, that
 * value is the colour; without one, the element's own text is.
 */
export const COLOR_SWATCH_ATTRIBUTE = 'data-color-swatch';

extend([a11yPlugin]);

export interface RehypeColorSwatchOptions {
  /**
   * Paint every inline code whose whole content is a colour value (default
   * `true`). Turn it off where inline code is routinely colour-shaped —
   * `#section` anchors, `hsl()` in prose about CSS — and mark the ones you want
   * with `data-color-swatch`.
   */
  scan?: boolean;
}

interface Node {
  type: string;
  tagName?: string;
  value?: unknown;
  properties?: Record<string, unknown>;
  children?: Node[];
}

export const rehypeColorSwatch: Plugin<[RehypeColorSwatchOptions?]> =
  (options = {}) =>
  (tree) => {
    walk(tree as Node, options.scan !== false, false);
  };

function walk(node: Node, scan: boolean, insidePre: boolean): void {
  const inPre = insidePre || node.tagName === 'pre';
  for (const child of node.children ?? []) {
    // A fenced code block is source, not a sample: a colour written inside one
    // keeps the block's own styling.
    if (!inPre && child.type === 'element' && child.tagName === 'code') {
      const color = markedColor(child) ?? (scan ? colorOf(textContent(child)) : null);
      if (color !== null) {
        paint(child, color);
      }
      continue;
    }
    walk(child, scan, inPre);
  }
}

/**
 * The colour of an explicitly marked `<code>`, or `null` when it is not marked.
 *
 * The attribute is read under both spellings: a JSX attribute reaches this phase
 * as the hast property `dataColorSwatch`, while one written as raw HTML can stay
 * `data-color-swatch`.
 */
function markedColor(node: Node): string | null {
  const properties = node.properties ?? {};
  const marker = properties.dataColorSwatch ?? properties['data-color-swatch'];
  if (marker === undefined || marker === null || marker === false) {
    return null;
  }
  // A value-less attribute means "the text is the colour", which is what a page
  // writes when it wants the scan behaviour without the scan.
  const explicit = typeof marker === 'string' ? marker : '';
  return colorOf(explicit.length > 0 ? explicit : textContent(node));
}

/** The colour as an opaque hex value, or `null` when it does not parse. */
function colorOf(value: string): string | null {
  const parsed = colord(value.trim());
  return parsed.isValid() ? parsed.alpha(1).toHex() : null;
}

/** Adds the swatch to an element in place, keeping the author's own attributes. */
function paint(node: Node, color: string): void {
  const properties = (node.properties ??= {});

  const classes = Array.isArray(properties.className) ? properties.className : [];
  if (!classes.includes(COLOR_SWATCH_CLASS)) {
    properties.className = [...classes, COLOR_SWATCH_CLASS];
  }

  const existing = typeof properties.style === 'string' ? properties.style.trim() : '';
  const swatch = styleFor(color);
  properties.style = existing.length > 0 ? `${existing}; ${swatch}` : swatch;

  // The marker has done its job and does not reach the page.
  delete properties.dataColorSwatch;
  delete properties['data-color-swatch'];
}

/**
 * `background-color` plus whichever of black/white has more contrast against it.
 * The pair is chosen by comparing the two WCAG contrast ratios rather than by a
 * brightness threshold, so a mid-tone colour lands on the readable side.
 */
function styleFor(color: string): string {
  const background = colord(color);
  const foreground = background.contrast('#ffffff') >= background.contrast('#000000')
    ? '#ffffff'
    : '#000000';
  return `background-color: ${background.toHex()}; color: ${foreground}`;
}

/** The plain text inside a node, ignoring anything that is not a text node. */
function textContent(node: Node): string {
  return (node.children ?? [])
    .filter((child) => child.type === 'text' && typeof child.value === 'string')
    .map((child) => child.value as string)
    .join('');
}
