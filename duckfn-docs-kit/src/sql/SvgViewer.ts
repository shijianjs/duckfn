import {el} from '../dom';
import {sectionFileName, type DownloadPayload} from '../download';
import {IconButton} from '../IconButton';
import {PanZoomView} from '../panzoom-view';
import {SourceDialog} from '../source-dialog';

/**
 * The `svg` result view: an inline SVG a reader can zoom (in the result area's
 * fullscreen), edit, and download — the same interactions a diagram gets from
 * `<dfk-mermaid>`, which is why both are built on `PanZoomView` and
 * `SourceDialog`.
 *
 * A plain class rather than a custom element: nothing hands it content through
 * markup, and the SQL renderer already has the parsed node, so there is no
 * upgrade to wait for.
 *
 * Like an embedded `<dfk-mermaid>`, the element contributes no frame — the result
 * area's panel is the frame — and no floating cluster: its two controls travel out
 * through {@link actions} for the result's tab strip, and its file travels out
 * through {@link downloadPayload} for the strip's download button. Zoom is off
 * until {@link setFullscreen} says the result area is expanded.
 *
 * This is browser-only code.
 */

/** The strings a figure's controls need. */
export interface FigureLabels {
  reset: string;
  edit: string;
  editTitle: string;
  apply: string;
  cancel: string;
}

export class SvgViewer {
  /** The box in the tab panel: the panzoom viewport, and the frame's clipping box. */
  readonly root = el('div', {class: 'dfk-sql-svg'});
  /** The controls the result area's tab strip shows while this tab is active. */
  readonly actions = el('div', {class: 'dfk-sql-tab-actions'});
  readonly #content = el('div', {class: 'dfk-sql-svg-content'});
  readonly #view = new PanZoomView(this.root, this.#content);
  readonly #resetBtn = new IconButton('lucide:rotate-ccw', () => this.#view.reset());
  readonly #editBtn: IconButton;
  readonly #dialog: SourceDialog;

  /** The markup as edited, the source for the next re-render and the text export. */
  #markup: string;
  /** The rendered node, or `null` when the markup does not parse as SVG. */
  #svg: SVGElement | null = null;
  /** The file-name base when nothing on the page says anything better. */
  readonly #fallback: string;

  constructor(svg: SVGElement | null, markup: string, labels: FigureLabels, fallback: string) {
    this.#markup = markup;
    this.#fallback = fallback;
    this.#svg = svg;
    this.#editBtn = new IconButton('lucide:pencil', () => this.#openEditor());
    this.#dialog = new SourceDialog({
      title: labels.editTitle,
      apply: labels.apply,
      cancel: labels.cancel,
    });
    this.#resetBtn.setLabel(labels.reset);
    this.#editBtn.setLabel(labels.edit);
    // Zoom is off inline (the result panel is not a viewport until it is
    // expanded), so an inline figure has nothing to reset — the button would be
    // the one dead control in the strip. `setFullscreen` reveals it.
    this.#resetBtn.setHidden(true);
    this.actions.append(this.#resetBtn.root, this.#editBtn.root);
    this.#view.setContent(svg ?? textBlock(markup));
    this.root.append(this.#content, this.#dialog.root);
  }

  /** Turns zoom and pan on or off; the result area calls this with its fullscreen. */
  setFullscreen(value: boolean): void {
    this.#view.setActive(value);
    this.#resetBtn.setHidden(!value);
  }

  /**
   * The file this view would be saved as: the SVG itself, or — when the markup
   * does not parse and the view is showing it as text — that text.
   */
  downloadPayload(): DownloadPayload {
    const name = sectionFileName(this.root, this.#svg === null ? 'txt' : 'svg', {
      fallback: this.#fallback,
    });
    if (this.#svg === null) {
      return {name, mime: 'text/plain;charset=utf-8', text: this.#markup};
    }
    return {
      name,
      mime: 'image/svg+xml;charset=utf-8',
      text: new XMLSerializer().serializeToString(this.#svg),
    };
  }

  dispose(): void {
    this.#view.destroy();
    this.#dialog.destroy();
    this.#dialog.root.remove();
  }

  #openEditor(): void {
    this.#dialog.open(this.#markup, (value) => {
      this.#markup = value;
      this.#svg = parseSvgMarkup(this.root.ownerDocument, value);
      this.#view.setContent(this.#svg ?? textBlock(value));
    });
  }
}

/** The fallback view for markup that is not SVG: the text, not an empty box. */
function textBlock(markup: string): HTMLElement {
  return el('pre', {class: 'dfk-sql-text', text: markup});
}

const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';

/**
 * Parses SVG markup from a result cell into a node this document can host.
 *
 * `image/svg+xml` is strict XML: malformed markup (unclosed tags, a bare `&`,
 * an HTML `<br>`) comes back as a `<parsererror>` element rather than throwing,
 * so the caller can degrade to text. Script-bearing and event-handler content
 * is stripped — inline SVG is *not* isolated (use the `iframe` renderer for
 * untrusted markup).
 *
 * The result is *sized* by `sql.css` (`max-width: 100%` + `height: auto`), which
 * needs a `viewBox` to mean anything: see {@link ensureViewBox}.
 */
export function parseSvgMarkup(document: Document, markup: string): SVGElement | null {
  if (!markup.trim()) {
    return null;
  }
  const parsed = new DOMParser().parseFromString(markup, 'image/svg+xml');
  const root = parsed.documentElement;
  if (!root || root.localName === 'parsererror' || root.namespaceURI !== SVG_NAMESPACE) {
    return null;
  }
  stripActiveContent(root);
  ensureViewBox(root);
  return document.importNode(root, true) as unknown as SVGElement;
}

/** An SVG `width` / `height` attribute as a positive number of px, or `null`. */
const PX_LENGTH = /^\s*([+-]?(?:\d+\.?\d*|\.\d+))\s*(?:px)?\s*$/i;

function pxLength(value: string | null): number | null {
  const match = value === null ? null : PX_LENGTH.exec(value);
  if (!match) {
    return null;
  }
  const parsed = Number.parseFloat(match[1]);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

/**
 * Gives a `viewBox`-less root the coordinate system it needs to be *scalable*.
 *
 * An SVG with no `viewBox` does not scale at all: one user unit **is** one CSS
 * pixel, so a stylesheet can narrow the element's box but the drawing keeps its
 * own size and is simply cut off at the root's viewport. That is what a plotter
 * which writes a fixed size and no viewBox produces — kuva's SVG backend emits
 * exactly `<svg width="800" height="600">` — and it shows up as a figure wider
 * than the result panel losing its right-hand side instead of getting smaller.
 *
 * Declaring that same rectangle as the viewBox turns the attributes into the
 * figure's *natural* size, which is what `sql.css` needs: `max-width: 100%`
 * shrinks the whole drawing, `height: auto` follows the aspect ratio, and the
 * panel grows to the scaled height. The attributes are left in place — they are
 * the figure's intrinsic size, and the downloaded / re-edited file should still
 * open at it.
 *
 * Only a pixel length can be turned into a coordinate system (`800`, `800px`).
 * A percentage (`width="100%"`) says nothing about the drawing's own extent, and
 * without a size there is nothing to infer from; those roots keep whatever the
 * markup declared.
 */
function ensureViewBox(root: Element): void {
  if (root.hasAttribute('viewBox')) {
    return;
  }
  const width = pxLength(root.getAttribute('width'));
  const height = pxLength(root.getAttribute('height'));
  if (width === null || height === null) {
    return;
  }
  root.setAttribute('viewBox', `0 0 ${width} ${height}`);
}

/** Removes the parts of an SVG document that could execute or navigate. */
function stripActiveContent(root: Element): void {
  for (const node of root.querySelectorAll('script, foreignObject')) {
    node.remove();
  }
  const visit = (element: Element): void => {
    for (const attribute of [...element.attributes]) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.replace(/\s/g, '').toLowerCase();
      if (name.startsWith('on') || ((name === 'href' || name === 'xlink:href') && value.startsWith('javascript:'))) {
        element.removeAttribute(attribute.name);
      }
    }
    for (const child of element.children) {
      visit(child);
    }
  };
  visit(root);
}
