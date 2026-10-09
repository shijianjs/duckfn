import {IconButton} from '../IconButton';
import {el, HTMLElementBase} from '../dom';
import {saveDownload, sectionFileName, type DownloadPayload} from '../download';
import {PanZoomView} from '../panzoom-view';
import {SourceDialog} from '../source-dialog';
import {parseMermaidConfig, type DfkMermaidConfig, type MermaidColorMode} from './config';
import {documentColorMode, parseMermaidSvg, renderMermaid, serializeMermaidSvg, watchColorMode} from './render';
import {mermaidStyles} from './styles';

/**
 * `<dfk-mermaid>` — a ```mermaid fence rendered as a diagram, produced by
 * `remarkMermaid`.
 *
 * This element *is* the kit's mermaid integration: it replaces
 * `@docusaurus/theme-mermaid`, whose React component cannot avoid the two
 * upstream defects `./render.ts` documents (the dark-mode first-load flash and
 * the empty diagram, both from rendering twice and concurrently). Because the
 * rendering happens here rather than inside a React tree, the same code serves
 * the runnable-SQL `mermaid` output — see the renderer in `sql/renderers.ts`.
 *
 * Two ways to use the element:
 *
 * - **Standalone** (a fence, the default) — the diagram floats the usual icon
 *   cluster in its top-right corner: reset zoom, source editing, download, and a
 *   fullscreen toggle of its own; zoom and pan turn on only in that fullscreen.
 * - **Embedded** (`embedded`, set by the runnable-SQL renderer) — the element
 *   sheds its frame and its floating cluster, because the SQL result area already
 *   draws both. Its own zoom controls travel out as {@link actions} (reset zoom /
 *   source editing) for the result's tab strip, its download travels out as
 *   {@link downloadPayload} for the same strip's download button, and zoom is
 *   driven from outside by {@link setFullscreen} — the result area's fullscreen,
 *   which the element then fills.
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
 * `source` / `config` / `embedded` are read once, because none of the producers —
 * the remark plugin, the runnable-SQL `mermaid` renderer — has a React mount point
 * to call a setter from. Reading once to initialise is not an attribute→render
 * loop, so the retained-mode contract still holds. Seeding is deferred until the
 * first need ({@link #ensureSeeded}) rather than pinned to `connectedCallback`:
 * the SQL renderer has to reach {@link actions} *before* inserting the element, to
 * hand the container to the tab strip it is building.
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

/** What an embedded diagram is called when nothing on the page says otherwise. */
const FALLBACK_FILE = 'mermaid-diagram';

export class DfkMermaid extends HTMLElementBase {
  readonly #canvas = el('div', {class: 'dfk-mermaid-canvas', hidden: true});
  /**
   * The panzoom viewport, which also clips: the transform runs on the content
   * box *inside* it, so a zoomed diagram cannot spill over the page.
   */
  readonly #viewport = el('div', {class: 'dfk-mermaid-viewport'});
  /** The transform target; holds the rendered `<svg>` and nothing else. */
  readonly #content = el('div', {class: 'dfk-mermaid-content'});
  readonly #view = new PanZoomView(this.#viewport, this.#content);
  /**
   * The zoom/source controls. Placed inside `#canvas` (floating) when standalone
   * and handed out through {@link actions} when embedded; the class name is set at
   * seed time, because which stylesheet has to reach it depends on that.
   */
  readonly #actions = el('div');
  readonly #message = el('p', {
    class: 'dfk-mermaid-message',
    attrs: {'aria-live': 'polite'},
    hidden: true,
  });
  readonly #resetBtn = new IconButton('lucide:rotate-ccw', () => this.#view.reset());
  readonly #editBtn = new IconButton('lucide:pencil', () => this.#openEditor());
  /** Standalone only; embedded diagrams download through the result chrome. */
  #downloadBtn: IconButton | null = null;
  /** Standalone only; embedded diagrams zoom in the result area's fullscreen. */
  #fullscreenBtn: IconButton | null = null;
  readonly #dialog = new SourceDialog({
    title: LABELS.en.editTitle,
    apply: LABELS.en.apply,
    cancel: LABELS.en.cancel,
  });

  #labels: MermaidLabels = LABELS.en;
  #config: DfkMermaidConfig = parseMermaidConfig(null);
  #source = '';
  #embedded = false;
  /**
   * The diagram currently on screen, held as a *node* rather than as mermaid's
   * returned string: the download re-serialises it (`serializeMermaidSvg`), which
   * is the only way to get well-formed SVG out of mermaid's HTML-serialised
   * output. `null` until a diagram renders.
   */
  #svg: SVGElement | null = null;
  #colorMode: MermaidColorMode | null = null;
  #seeded = false;
  /** Standalone fullscreen, which is also this element's zoom switch. */
  #expanded = false;
  /** Embedded fullscreen, driven from outside; also the zoom switch. */
  #fullscreen = false;
  /** Whether the document-level Esc handler is currently attached. */
  #escBound = false;
  /** Invalidates an in-flight render when a newer one starts or the element leaves. */
  #renderToken = 0;
  #unwatchColorMode: (() => void) | null = null;

  readonly #onEsc = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && this.#expanded && !this.#dialog.root.open) {
      this.#setExpanded(false);
    }
  };
  readonly #onColorModeChange = (): void => {
    if (documentColorMode() !== this.#colorMode) {
      void this.#render();
    }
  };

  constructor() {
    super();
    this.#viewport.appendChild(this.#content);
    this.#canvas.appendChild(this.#viewport);

    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [mermaidStyles()];
    // No slot: nothing is ever handed in as a child (the source travels as an
    // attribute), so the light DOM stays empty and there is nothing to hide.
    shadow.append(this.#canvas, this.#message, this.#dialog.root);
    this.#applyLabels();
  }

  /**
   * The zoom/source controls an embedded diagram offers, for the host to place in
   * its own chrome. Empty (and unused) when standalone — the element keeps them.
   */
  get actions(): HTMLElement {
    this.#ensureSeeded();
    return this.#actions;
  }

  connectedCallback(): void {
    this.#ensureSeeded();
    // Registered here rather than in the constructor: a listener on an *external*
    // object has to be paired with a removal, and connect/disconnect is where
    // that pairing is observable (React may remount the element).
    this.#unwatchColorMode ??= watchColorMode(this.#onColorModeChange);
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
    this.#view.destroy();
    this.#renderToken += 1;
    this.#dialog.destroy();
  }

  /**
   * Turns zoom/pan on or off for an embedded diagram. The host calls this with its
   * own fullscreen state: an embedded diagram zooms exactly where its result area
   * is expanded, and fills that area while it is.
   */
  setFullscreen(value: boolean): void {
    this.#ensureSeeded();
    this.#fullscreen = value;
    this.classList.toggle('dfk-mermaid-fullscreen', value);
    this.#view.setActive(value);
    this.#syncActions();
  }

  /**
   * The file this diagram would be saved as, or `null` while nothing is rendered.
   * An embedded diagram hands this to the result chrome's download button, which
   * is why the element does not save it itself.
   */
  downloadPayload(): DownloadPayload | null {
    if (this.#svg === null) {
      return null;
    }
    return {
      name: sectionFileName(this, 'svg', {source: this.#source, fallback: FALLBACK_FILE}),
      mime: 'image/svg+xml;charset=utf-8',
      text: serializeMermaidSvg(this.#svg),
    };
  }

  /**
   * Reads the `source` / `config` / `embedded` attributes once, and builds the
   * part of the structure that depends on the mode. Idempotent, and callable
   * before connection (see {@link actions}): the SQL renderer reads the attributes
   * it set through `el()` before it inserts the element.
   */
  #ensureSeeded(): void {
    if (this.#seeded) {
      return;
    }
    this.#seeded = true;
    this.#seed();
  }

  #seed(): void {
    this.#labels =
      LABELS[(document.documentElement.getAttribute('lang') ?? 'en').toLowerCase()] ??
      LABELS.en;
    const source = this.getAttribute('source');
    if (source !== null) {
      this.#source = source;
    }
    this.#config = parseMermaidConfig(this.getAttribute('config'));
    this.#embedded = this.hasAttribute('embedded');
    if (this.#embedded) {
      // The result chrome places this container and styles it (see `sql.css`);
      // the class is neutral because a shadow boundary is not involved here.
      this.#actions.className = 'dfk-sql-tab-actions';
      this.#actions.append(this.#resetBtn.root, this.#editBtn.root);
    } else {
      this.#downloadBtn = new IconButton('lucide:download', () => this.#download());
      this.#fullscreenBtn = new IconButton('lucide:maximize', () =>
        this.#setExpanded(!this.#expanded),
      );
      this.#actions.className = 'dfk-mermaid-actions';
      this.#actions.append(
        this.#resetBtn.root,
        this.#editBtn.root,
        this.#downloadBtn.root,
        this.#fullscreenBtn.root,
      );
      this.#canvas.appendChild(this.#actions);
    }
    this.#applyLabels();
    this.#syncActions();
  }

  #applyLabels(): void {
    this.#resetBtn.setLabel(this.#labels.reset);
    this.#editBtn.setLabel(this.#labels.edit);
    this.#downloadBtn?.setLabel(this.#labels.download);
    this.#fullscreenBtn?.setLabel(
      this.#expanded ? this.#labels.exitFullscreen : this.#labels.fullscreen,
    );
    this.#dialog.setLabels({
      title: this.#labels.editTitle,
      apply: this.#labels.apply,
      cancel: this.#labels.cancel,
    });
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
      this.#view.setContent(svg);
      // Mermaid's own hook for click handlers on nodes; it takes the container
      // that holds the SVG.
      output.bind?.(this.#content);
      this.#canvas.hidden = false;
      this.#setMessage('', false);
    } catch (error) {
      if (token !== this.#renderToken || !this.isConnected) {
        return;
      }
      this.#svg = null;
      this.#view.setContent(null);
      this.#canvas.hidden = true;
      this.#setMessage(`${this.#labels.renderFailed}: ${messageOf(error)}`, true);
    } finally {
      this.#syncActions();
    }
  }

  #setMessage(text: string, isError: boolean): void {
    this.#message.textContent = text;
    this.#message.hidden = text === '';
    this.#message.classList.toggle('dfk-mermaid-message-error', isError);
  }

  /**
   * Editing, downloading and resetting only mean something once a diagram is on
   * screen — and resetting only while zooming is possible, which is this element's
   * own fullscreen when standalone and the result area's when embedded. Derived
   * from the state rather than handed in, so every transition (render, expand,
   * external fullscreen) calls the same thing.
   */
  #syncActions(): void {
    const rendered = this.#svg !== null;
    this.#editBtn.setHidden(!rendered);
    this.#downloadBtn?.setHidden(!rendered);
    this.#resetBtn.setHidden(!rendered || !this.#zoomActive());
  }

  /** Where this diagram's zoom lives: its own fullscreen, or the result area's. */
  #zoomActive(): boolean {
    return this.#embedded ? this.#fullscreen : this.#expanded;
  }

  // --- Fullscreen ------------------------------------------------------------

  /** The standalone fullscreen toggle; also switches zoom on and off. */
  #setExpanded(value: boolean): void {
    this.#expanded = value;
    this.#canvas.classList.toggle('dfk-mermaid-expanded', value);
    this.#view.setActive(value);
    this.#syncActions();
    this.#fullscreenBtn?.setIcon(value ? 'lucide:minimize' : 'lucide:maximize');
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

  /** Saves the diagram as a standalone `.svg` file (see {@link downloadPayload}). */
  #download(): void {
    const payload = this.downloadPayload();
    if (payload) {
      saveDownload(payload);
    }
  }

  // --- Source editing --------------------------------------------------------

  /** Opens the source dialog; the edited text is re-rendered on Apply. */
  #openEditor(): void {
    this.#dialog.open(this.#source, (value) => {
      this.#source = value;
      void this.#render();
    });
  }
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
