import filenamify from 'filenamify';
import {el} from './dom';

/**
 * Saving something the kit rendered to a file: the payload shape, the click that
 * performs the save, and how the file is named.
 *
 * Shared by `<dfk-mermaid>` (a downloaded diagram) and the SQL result chrome (a
 * downloaded result of any kind), so a docs page has one definition of "what a
 * download is" rather than one per component.
 *
 * This is browser-only code.
 */

/** One file's worth of text, ready to save. */
export interface DownloadPayload {
  /** The file name, extension included. */
  name: string;
  /** The blob's MIME type. */
  mime: string;
  /** The file's contents — text for every format the kit produces. */
  text: string;
}

/**
 * Saves a payload through a temporary object URL.
 *
 * The anchor is in the document for the click — some browsers ignore a detached
 * one — and the URL is revoked on the next tick, once the download has been
 * handed to the browser.
 */
export function saveDownload(payload: DownloadPayload): void {
  const blob = new Blob([payload.text], {type: payload.mime});
  const url = URL.createObjectURL(blob);
  const link = el('a', {href: url, download: payload.name});
  document.body.appendChild(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

/** Used when nothing on the page says anything about what this is. */
const DEFAULT_BASE = 'diagram';

/**
 * The length cap. `filenamify` truncates by grapheme, so a CJK heading is cut at
 * 80 *characters*, not bytes, and an emoji survives whole.
 */
const MAX_LENGTH = 80;

/** How a reserved character is spelled in the file name (`2. Registration: …`). */
const REPLACEMENT = '-';

/**
 * The *format* characters (zero-width space, word joiner, byte-order mark…) and
 * whitespace that a title can wear at its edges.
 */
const EDGE_NOISE = /^[\s\p{Cf}]+|[\s\p{Cf}]+$/gu;

/**
 * Trims a candidate down to its text, dropping format characters at the edges.
 *
 * Plain whitespace trimming is not enough: Docusaurus gives every heading an
 * anchor link whose label is a zero-width space, so `textContent` of a heading is
 * `"2. Registration\u200B"`. `filenamify` turns a format character into the
 * replacement rather than dropping it, which would produce
 * `2. Registration-.svg`. Only the *edges*: an interior zero-width joiner is what
 * holds an emoji together.
 */
function cleanTitle(value: string | undefined): string {
  return value === undefined ? '' : value.replace(EDGE_NOISE, '');
}

/**
 * What a downloaded figure is called.
 *
 * The name is derived from the page, not from the figure: a diagram or an SVG
 * result has no name of its own, and the reader downloading one is after "the
 * figure from that section", not `diagram.svg`.
 *
 * Three sources, most specific first, each falling back to the next:
 *
 * 1. **The figure's own title** — mermaid's frontmatter (`---\ntitle: …\n---`,
 *    which mermaid itself draws above the diagram); only read when `options.source`
 *    is given.
 * 2. **The nearest heading above it** — the section the figure belongs to, which
 *    on a docs page is what a reader would call it.
 * 3. **The document title** — the browser tab's title, for a figure that sits
 *    above every heading on its page.
 *
 * Nothing found → `options.fallback` (or {@link DEFAULT_BASE}), with `extension`
 * appended either way.
 */
export function sectionFileName(
  element: Element,
  extension: string,
  options: {source?: string; fallback?: string} = {},
): string {
  const candidates = [
    options.source === undefined ? undefined : frontmatterTitle(options.source),
    precedingHeading(element),
    element.ownerDocument.title,
  ].map(cleanTitle);
  const title = candidates.find((candidate) => candidate !== '');
  if (title === undefined) {
    return `${options.fallback ?? DEFAULT_BASE}.${extension}`;
  }
  // Filenames are a filesystem concern, so they go through a library rather than a
  // hand-rolled character class. Three things it does that matter here and that a
  // browser does *not*: it strips what Windows and macOS reject (`:`, `?`, `*`,
  // `"`, `<`, `>`, `|`, the path separators and control characters), it trims the
  // trailing dots and spaces Windows silently drops, and it avoids the reserved
  // device names (`con`, `nul`, …). The `download` attribute's own sanitisation
  // covers only `/` and `\`.
  //
  // It also normalises Unicode whitespace and drops format characters, which
  // quietly cleans up Docusaurus' heading anchors: the `<a>` it appends to every
  // heading contributes a zero-width space to `textContent`.
  return `${filenamify(title, {replacement: REPLACEMENT, maxLength: MAX_LENGTH})}.${extension}`;
}

/**
 * mermaid's frontmatter block, if the source opens with one.
 *
 * Only the top-level `title:` key is read. Mermaid's frontmatter is YAML, and the
 * rest of it (a nested `config:`, `displayMode:`, …) is none of this module's
 * business: a key that is always a plain scalar on one line does not justify a
 * YAML parser, and anything indented — a nested key — is skipped by anchoring the
 * match at the line start.
 */
function frontmatterTitle(source: string): string | undefined {
  const block = /^---\r?\n([\s\S]*?)\r?\n---/.exec(source.trimStart())?.[1];
  if (block === undefined) {
    return undefined;
  }
  const value = /^title\s*:\s*(.+)$/m.exec(block)?.[1];
  return value === undefined ? undefined : unquote(value.trim());
}

/** Strips one pair of matching quotes, so `title: "A: B"` keeps its colon. */
function unquote(value: string): string {
  const first = value[0];
  return (first === '"' || first === "'") && value.endsWith(first) ? value.slice(1, -1) : value;
}

/**
 * The text of the last heading that precedes the figure, or `undefined`.
 *
 * The search is scoped to the enclosing `<article>` where there is one: a docs
 * page's navbar, sidebar and footer are full of headings that have nothing to do
 * with this figure, and the article is the one container that holds the page's
 * own content.
 */
function precedingHeading(element: Element): string | undefined {
  const scope: ParentNode = element.closest('article') ?? element.ownerDocument;
  let found: string | undefined;
  for (const heading of scope.querySelectorAll('h1, h2, h3, h4, h5, h6')) {
    // `FOLLOWING` means the heading comes before the element in document order.
    if (!(heading.compareDocumentPosition(element) & Node.DOCUMENT_POSITION_FOLLOWING)) {
      // Headings are in document order, so nothing later can precede it either.
      break;
    }
    const text = heading.textContent?.trim();
    if (text) {
      found = text;
    }
  }
  return found;
}
