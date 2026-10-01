import type {PanzoomObject} from '@panzoom/panzoom';
import type {CodeEditor} from '../codemirror';
import {mountCodeEditor} from '../codemirror';
import {IconButton} from '../IconButton';
import {el, HTMLElementBase} from '../dom';
import {parseMermaidConfig, type DfkMermaidConfig, type MermaidColorMode} from './config';
import {
  documentColorMode,
  loadPanzoom,
  parseMermaidSvg,
  renderMermaid,
  serializeMermaidSvg,
  watchColorMode,
} from './render';
import {mermaidStyles} from './styles';

/**
 * `<dfk-mermaid>` — a ```mermaid fence rendered as a diagram, produced by
 * `remarkMermaid` (or built by hand, see `setSource`).
 *
 * This element *is* the kit's mermaid integration: it replaces
 * `@docusaurus/theme-mermaid`, whose React component cannot avoid the two
 * upstream defects `./render.ts` documents (the dark-mode first-load flash and
 * the empty diagram, both from rendering twice and concurrently). Because the
 * rendering happens here rather than inside a React tree, the same code serves
 * the runnable-SQL `mermaid` output — see the renderer in `sql/renderers.ts`.
 *
 * What a reader gets on top of the diagram:
 *
 * - **Zoom and pan** — wheel to zoom, drag to pan (`@panzoom/panzoom` on the
 *   content box, so the SVG node itself is never mutated; the download
 *   re-serialises that same untouched node, see `serializeMermaidSvg`). Panning
 *   only engages once the diagram is zoomed, which is what lets the page keep
 *   scrolling normally over a diagram that fits.
 * - **Reset zoom**, **fullscreen**, **source editing** (a CodeMirror dialog),
 *   and **download SVG** as floating icon buttons in the top-right corner, the
 *   same idiom as the kit's code blocks.
 *
 * Everything lives in the shadow root, diagram included: mermaid ships an inline
 * `<style>` inside every SVG it renders, and one shadow root per diagram is what
 * keeps those styles from leaking into the page (and into each other).
 *
 * Retained-mode: the structure is built once in the constructor and held in
 * fields; the `#render*` / `#set*` helpers mutate the nodes they own. The only
 * wholesale replacement is the rendered SVG itself — a new render *is* a new
 * document, the same way a new query result is.
 *
 * Content entry is an **attribute seed** (see CONVENTIONS.md rule 5 exception):
 * `source` / `config` are read once in `connectedCallback`, because neither
 * producer — the remark plugin, or the runnable-SQL `mermaid` renderer — has a
 * React mount point to call a setter from. Reading once to initialise is not an
 * attribute→render loop, so the retained-mode contract still holds.
 */

type MermaidLabels = {
  reset: string;
  fullscreen: string;
  exitFullscreen: string;
  edit: string;
  download: string;
  apply: string;
  cancel: string;
  editTitle: string;
  rendering: string;
  renderFailed: string;
};

const LABELS: Record<string, MermaidLabels> = {
  en: {
    reset: 'Reset zoom',
    fullscreen: 'Fullscreen',
    exitFullscreen: 'Exit fullscreen',
    edit: 'Edit diagram source',
    download: 'Download SVG',
    apply: 'Apply',
    cancel: 'Cancel',
    editTitle: 'Mermaid source',
    rendering: 'Rendering diagram…',
    renderFailed: 'Could not render the diagram',
  },
  'zh-hans': {
    reset: '还原缩放',
    fullscreen: '全屏',
    exitFullscreen: '退出全屏',
    edit: '编辑图表源码',
    download: '下载 SVG',
    apply: '应用',
    cancel: '取消',
    editTitle: 'Mermaid 源码',
    rendering: '正在渲染图表…',
    renderFailed: '图表渲染失败',
  },
};

/**
 * The file a downloaded diagram is written to. Diagrams have no name of their
 * own (the fence carries none), so this is a fixed stem; a browser that sees a
 * second download adds a suffix of its own.
 */
const DOWNLOAD_NAME = 'mermaid-diagram.svg';

export class DfkMermaid extends HTMLElementBase {
  readonly #canvas = el('div', {class: 'dfk-mermaid-canvas', hidden: true});
  /**
   * The panzoom viewport, which also clips: the transform runs on the content
   * box *inside* it, so a zoomed diagram cannot spill over the page.
   */
  readonly #viewport = el('div', {class: 'dfk-mermaid-viewport'});
  /** The transform target; holds the rendered `<svg>` and nothing else. */
  readonly #content = el('div', {class: 'dfk-mermaid-content'});
  readonly #actions = el('div', {class: 'dfk-mermaid-actions'});
  readonly #message = el('p', {
    class: 'dfk-mermaid-message',
    attrs: {'aria-live': 'polite'},
    hidden: true,
  });
  readonly #resetBtn: IconButton;
  readonly #editBtn: IconButton;
  readonly #downloadBtn: IconButton;
  readonly #fullscreenBtn: IconButton;
  readonly #dialog = el('dialog', {class: 'dfk-mermaid-dialog'});
  readonly #dialogTitle = el('h2', {class: 'dfk-mermaid-dialog-title'});
  /** Where the CodeMirror editor mounts, inside the dialog. */
  readonly #editorHost = el('div', {class: 'dfk-mermaid-dialog-editor'});
  readonly #applyBtn = el('button', {
    class: 'dfk-mermaid-dialog-button dfk-mermaid-dialog-apply',
    type: 'button',
  });
  readonly #cancelBtn = el('button', {class: 'dfk-mermaid-dialog-button', type: 'button'});

  #labels: MermaidLabels = LABELS.en;
  #config: DfkMermaidConfig = parseMermaidConfig(null);
  #source = '';
  /**
   * The diagram currently on screen, held as a *node* rather than as mermaid's
   * returned string: the download re-serialises it (`serializeMermaidSvg`), which
   * is the only way to get well-formed SVG out of mermaid's HTML-serialised
   * output. `null` until a diagram renders.
   */
  #svg: SVGElement | null = null;
  #colorMode: MermaidColorMode | null = null;
  #seeded = false;
  #expanded = false;
  /** Whether the document-level Esc handler is currently attached. */
  #escBound = false;
  /** Invalidates an in-flight render when a newer one starts or the element leaves. */
  #renderToken = 0;
  #panzoom: PanzoomObject | null = null;
  #panzoomLoading = false;
  #editor: CodeEditor | null = null;
  #editorLoading = false;
  #unwatchColorMode: (() => void) | null = null;

  readonly #onEsc = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && this.#expanded && !this.#dialog.open) {
      this.#setExpanded(false);
    }
  };
  readonly #onWheel = (event: WheelEvent): void => {
    this.#panzoom?.zoomWithWheel(event);
  };
  readonly #onColorModeChange = (): void => {
    if (documentColorMode() !== this.#colorMode) {
      void this.#render();
    }
  };

  constructor() {
    super();
    this.#resetBtn = new IconButton('lucide:rotate-ccw', () => this.#resetView());
    this.#editBtn = new IconButton('lucide:pencil', () => this.#openEditor());
    this.#downloadBtn = new IconButton('lucide:download', () => this.#download());
    this.#fullscreenBtn = new IconButton('lucide:maximize', () =>
      this.#setExpanded(!this.#expanded),
    );

    this.#actions.append(
      this.#resetBtn.root,
      this.#editBtn.root,
      this.#downloadBtn.root,
      this.#fullscreenBtn.root,
    );
    this.#viewport.appendChild(this.#content);
    this.#canvas.append(this.#viewport, this.#actions);

    this.#applyBtn.addEventListener('click', () => this.#applyEdit());
    this.#cancelBtn.addEventListener('click', () => this.#dialog.close());
    this.#dialog.append(
      el('div', {class: 'dfk-mermaid-dialog-body'}, (body) =>
        body.append(
          this.#dialogTitle,
          this.#editorHost,
          el('div', {class: 'dfk-mermaid-dialog-footer'}, (footer) =>
            footer.append(this.#cancelBtn, this.#applyBtn),
          ),
        ),
      ),
    );

    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [mermaidStyles()];
    // No slot: nothing is ever handed in as a child (the source travels as an
    // attribute), so the light DOM stays empty and there is nothing to hide.
    shadow.append(this.#canvas, this.#message, this.#dialog);
    this.#applyLabels();
  }

  connectedCallback(): void {
    if (!this.#seeded) {
      this.#seed();
      this.#seeded = true;
    }
    // Registered here rather than in the constructor: a listener on an *external*
    // object has to be paired with a removal, and connect/disconnect is where
    // that pairing is observable (React may remount the element).
    this.#unwatchColorMode ??= watchColorMode(this.#onColorModeChange);
    void this.#ensurePanzoom();
    // Only when there is nothing on screen: a reconnect after a move already
    // carries its diagram, and re-rendering it would flash for no reason.
    if (this.#source && !this.#content.hasChildNodes()) {
      void this.#render();
    }
  }

  disconnectedCallback(): void {
    this.#setExpanded(false);
    this.#unwatchColorMode?.();
    this.#unwatchColorMode = null;
    // Panzoom binds move/up on `document`, so it outlives the element unless it
    // is torn down here.
    this.#viewport.removeEventListener('wheel', this.#onWheel);
    this.#panzoom?.destroy();
    this.#panzoom = null;
    this.#renderToken += 1;
    this.#editor?.destroy();
    this.#editor = null;
    if (this.#dialog.open) {
      this.#dialog.close();
    }
  }

  /**
   * Reads the `source` / `config` attributes once.
   *
   * This is the element's *only* content entry, for both producers: the remark
   * plugin emits the attributes at build time, and the runnable-SQL `mermaid`
   * renderer sets them on the element it creates. Neither has a React mount point
   * to call a setter from, which is the exception CONVENTIONS.md rule 5 makes for
   * plugin-generated elements — and reading them once to initialise is not an
   * attribute→render loop, so the retained-mode contract still holds.
   *
   * `source` is only applied when the attribute is actually present, so a caller
   * that sets the attribute before inserting the element keeps it: the attribute
   * is read at *upgrade* time, which for an element created by
   * `document.createElement` after `customElements.define` is the insertion.
   */
  #seed(): void {
    this.#labels =
      LABELS[(document.documentElement.getAttribute('lang') ?? 'en').toLowerCase()] ??
      LABELS.en;
    const source = this.getAttribute('source');
    if (source !== null) {
      this.#source = source;
    }
    this.#config = parseMermaidConfig(this.getAttribute('config'));
    this.#applyLabels();
  }

  #applyLabels(): void {
    this.#resetBtn.setLabel(this.#labels.reset);
    this.#editBtn.setLabel(this.#labels.edit);
    this.#downloadBtn.setLabel(this.#labels.download);
    this.#fullscreenBtn.setLabel(
      this.#expanded ? this.#labels.exitFullscreen : this.#labels.fullscreen,
    );
    this.#dialogTitle.textContent = this.#labels.editTitle;
    this.#cancelBtn.textContent = this.#labels.cancel;
    this.#applyBtn.textContent = this.#labels.apply;
  }

  // --- Rendering -------------------------------------------------------------

  /**
   * Renders the source for the page's current colour mode and puts the result on
   * screen. Re-entrancy is handled by {@link #renderToken}: a render that was
   * superseded (a newer one started, or the element was disconnected) drops its
   * result instead of racing the newer one into the DOM.
   *
   * The queue inside `render.ts` is what makes this safe at all — mermaid is one
   * mutable singleton, so two diagrams rendering at once corrupt each other.
   */
  async #render(): Promise<void> {
    if (!this.#source) {
      return;
    }
    const token = (this.#renderToken += 1);
    const colorMode = documentColorMode();
    this.#colorMode = colorMode;
    // A re-render keeps the old diagram up while the new one is in flight, so the
    // status line is only for the first paint (or for an error that left nothing).
    if (this.#svg === null) {
      this.#setMessage(this.#labels.rendering, false);
    }
    try {
      const output = await renderMermaid({
        source: this.#source,
        config: this.#config,
        colorMode,
      });
      if (token !== this.#renderToken || !this.isConnected) {
        return;
      }
      const svg = parseMermaidSvg(this.ownerDocument, output.svg);
      if (!svg) {
        throw new Error(this.#labels.renderFailed);
      }
      this.#svg = svg;
      this.#content.replaceChildren(svg);
      // Mermaid's own hook for click handlers on nodes; it takes the container
      // that holds the SVG.
      output.bind?.(this.#content);
      this.#canvas.hidden = false;
      this.#setMessage('', false);
      this.#setActionsAvailable(true);
      this.#resetView();
    } catch (error) {
      if (token !== this.#renderToken || !this.isConnected) {
        return;
      }
      this.#svg = null;
      this.#content.replaceChildren();
      this.#canvas.hidden = true;
      this.#setActionsAvailable(false);
      this.#setMessage(`${this.#labels.renderFailed}: ${messageOf(error)}`, true);
    }
  }

  #setMessage(text: string, isError: boolean): void {
    this.#message.textContent = text;
    this.#message.hidden = text === '';
    this.#message.classList.toggle('dfk-mermaid-message-error', isError);
  }

  /** Reset zoom and download only mean something once a diagram is on screen. */
  #setActionsAvailable(available: boolean): void {
    this.#resetBtn.root.hidden = !available;
    this.#downloadBtn.root.hidden = !available;
  }

  // --- Zoom and pan ----------------------------------------------------------

  /**
   * Binds panzoom to the content box. `@panzoom/panzoom` is an enhancement, not a
   * requirement: if its chunk never arrives, the diagram still renders, only
   * wheel zoom and dragging stay inert.
   *
   * `panOnlyWhenZoomed` is the setting that keeps a docs page usable — a diagram
   * that already fits does not swallow drags, so the pointer still selects text
   * and `touchAction: 'pan-y'` leaves vertical page scrolling to the browser.
   * Touch-action is a deliberate trade: pinch and horizontal drags go to the
   * diagram, the page keeps scrolling.
   */
  async #ensurePanzoom(): Promise<void> {
    if (this.#panzoom !== null || this.#panzoomLoading) {
      return;
    }
    this.#panzoomLoading = true;
    try {
      const Panzoom = await loadPanzoom();
      if (!this.isConnected) {
        return;
      }
      this.#viewport.addEventListener('wheel', this.#onWheel, {passive: false});
      this.#panzoom = Panzoom(this.#content, {
        maxScale: 8,
        minScale: 0.5,
        step: 0.25,
        cursor: 'grab',
        panOnlyWhenZoomed: true,
        touchAction: 'pan-y',
      });
    } catch {
      // Nothing to report: the diagram is already usable without pan/zoom.
    } finally {
      this.#panzoomLoading = false;
    }
  }

  #resetView(): void {
    this.#panzoom?.reset({
      // Zooming back to fit is a transition the reader did not ask to skip, but
      // one they may have asked not to have.
      animate: !prefersReducedMotion(),
    });
  }

  // --- Fullscreen ------------------------------------------------------------

  #setExpanded(value: boolean): void {
    this.#expanded = value;
    this.#canvas.classList.toggle('dfk-mermaid-expanded', value);
    this.#fullscreenBtn.setIcon(value ? 'lucide:minimize' : 'lucide:maximize');
    this.#applyLabels();
    if (value !== this.#escBound) {
      if (value) {
        document.addEventListener('keydown', this.#onEsc);
      } else {
        document.removeEventListener('keydown', this.#onEsc);
      }
      this.#escBound = value;
    }
  }

  // --- Download --------------------------------------------------------------

  /**
   * Saves the diagram as a standalone `.svg` file.
   *
   * The markup is re-serialised from the rendered node, not taken from mermaid's
   * return value — that one is HTML, and its void elements come out unclosed,
   * which a browser opening the file as XML rejects. See `serializeMermaidSvg`.
   */
  #download(): void {
    if (this.#svg === null) {
      return;
    }
    const markup = serializeMermaidSvg(this.#svg);
    const blob = new Blob([markup], {type: 'image/svg+xml;charset=utf-8'});
    const url = URL.createObjectURL(blob);
    const link = el('a', {href: url, download: DOWNLOAD_NAME});
    // Anchored in the shadow tree for the click; a detached anchor is ignored by
    // some browsers, and by then the download has already been handed to it.
    this.shadowRoot?.appendChild(link);
    link.click();
    link.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
  }

  // --- Source editing --------------------------------------------------------

  /**
   * Opens the source dialog, mounting the editor on first use. The dialog is
   * shown *before* the editor mounts: CodeMirror measures its container as it is
   * constructed, and a `display: none` dialog measures to zero.
   */
  #openEditor(): void {
    this.#dialog.showModal();
    if (this.#editor) {
      this.#editor.setValue(this.#source);
      return;
    }
    void this.#mountEditor();
  }

  async #mountEditor(): Promise<void> {
    if (this.#editorLoading) {
      return;
    }
    this.#editorLoading = true;
    try {
      // No language: there is no first-party CodeMirror grammar for mermaid, and
      // the dialog is for touching up a diagram, not writing SQL.
      const editor = await mountCodeEditor(this.#editorHost, this.#source, () => undefined);
      if (!this.isConnected || !this.#dialog.open) {
        // Closed while the modules were loading: nothing will ever dispose this
        // editor, so dispose it here.
        editor.destroy();
        return;
      }
      editor.setWrap(true);
      this.#editor = editor;
    } catch {
      // The dialog stays open with an empty editor box; a second click retries.
    } finally {
      this.#editorLoading = false;
    }
  }

  #applyEdit(): void {
    const edited = this.#editor?.getValue();
    this.#dialog.close();
    if (edited !== undefined && edited !== this.#source) {
      this.#source = edited;
      void this.#render();
    }
  }
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function prefersReducedMotion(): boolean {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}