import {el} from '../dom';
import {sectionFileName, type DownloadPayload} from '../download';
import {ansiRuns, plainAnsiText, type AnsiRun} from './ansi';

/**
 * The `terminal` result view: one captured terminal frame, drawn as a terminal.
 *
 * A query can return what a CLI would have printed — SGR colours, box-drawing and
 * braille characters, the lot — and `text` can only show it flat. This view is
 * that frame as a terminal panel: monospace, dark, never wrapped, rows tiled at
 * `line-height: 1` so the characters connect the way they do in a terminal (that
 * is what makes a plot drawn from U+2800 braille cells read as one image).
 *
 * Like the embedded `<dfk-mermaid>` and the `svg` view, it contributes no frame
 * — the result panel is the frame — and no floating cluster: it has no controls
 * of its own, so its file travels out through {@link downloadPayload} for the
 * strip's download button.
 *
 * **Nothing here is built from markup.** `ansi.ts` hands back styled runs and
 * this class turns each into a text node or a `<span>` with inline styles, which
 * is why the ANSI parsing could be a library that offers tokens instead of an
 * HTML string.
 *
 * This is browser-only code.
 */

export class TerminalView {
  /** The panel: the scroller, the frame's contents and the file-name base. */
  readonly root = el('div', {class: 'dfk-sql-terminal'});
  /** `white-space: pre` is what keeps the frame a grid. */
  readonly #screen = el('div', {class: 'dfk-sql-terminal-screen'});
  /** The frame as it arrived, for the download and for a re-render. */
  #raw = '';
  /** The file base when nothing on the page says anything better. */
  readonly #fallback: string;

  constructor(fallback: string) {
    this.#fallback = fallback;
    this.root.appendChild(this.#screen);
  }

  /**
   * Draws one frame.
   *
   * The parser (`ansi.ts` and the `anser` behind it) is imported statically, like
   * the kit's other small dependencies (`filenamify`, `colord`): at ~10kB gzipped
   * it is not worth a dynamic `import()`, and a static import cannot fail *late* —
   * if it is going to be missing, the element never upgrades at all.
   *
   * A parser that *throws* is a different matter: the words are still the answer,
   * so the frame degrades to the text with the colour lost instead of leaving an
   * empty panel.
   */
  async render(raw: string): Promise<void> {
    this.#raw = raw;
    let runs: AnsiRun[];
    try {
      runs = ansiRuns(raw);
    } catch {
      runs = [{text: plainAnsiText(raw), fg: null, bg: null, decorations: []}];
    }
    this.#screen.replaceChildren(...runs.map((run) => this.#runNode(run)));
  }

  /**
   * The file this frame would be saved as: the **raw** text, escapes and all —
   * a `.txt` a reader can pipe back into a terminal, which is the point of
   * having the dump at all.
   */
  downloadPayload(): DownloadPayload {
    return {
      name: sectionFileName(this.root, 'txt', {fallback: this.#fallback}),
      mime: 'text/plain;charset=utf-8',
      text: this.#raw,
    };
  }

  /**
   * One run as a node: a bare text node when nothing is styled (the common case
   * for the spaces a grid is mostly made of), a `<span>` with inline styles
   * otherwise.
   *
   * The decoration mapping follows what a terminal does rather than what CSS
   * would do on its own: `hidden` becomes `visibility: hidden` rather than
   * `opacity: 0` so the cell keeps its place in the grid, and `blink` has no
   * meaning in a still frame so it is dropped. `reverse` needs nothing either —
   * `ansi.ts` has already swapped the colours, which is why a reversed run shows
   * up here with the colours reversed and no decoration to apply.
   */
  #runNode(run: AnsiRun): Node {
    const document = this.root.ownerDocument;
    if (!run.fg && !run.bg && run.decorations.length === 0) {
      return document.createTextNode(run.text);
    }
    const span = document.createElement('span');
    if (run.fg) {
      span.style.color = `rgb(${run.fg})`;
    }
    if (run.bg) {
      span.style.backgroundColor = `rgb(${run.bg})`;
    }
    const decorations = new Set(run.decorations);
    if (decorations.has('bold')) {
      span.style.fontWeight = '700';
    }
    if (decorations.has('dim')) {
      span.style.opacity = '0.6';
    }
    if (decorations.has('italic')) {
      span.style.fontStyle = 'italic';
    }
    const lines = [
      decorations.has('underline') ? 'underline' : '',
      decorations.has('strikethrough') ? 'line-through' : '',
    ]
      .filter(Boolean)
      .join(' ');
    if (lines) {
      span.style.textDecoration = lines;
    }
    if (decorations.has('hidden')) {
      span.style.visibility = 'hidden';
    }
    span.textContent = run.text;
    return span;
  }
}