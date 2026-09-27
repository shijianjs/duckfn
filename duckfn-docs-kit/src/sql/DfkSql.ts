import type {IconifyIconHTMLElement} from 'iconify-icon';
import type {RunnableSqlConfig} from './remark';
import {DuckDBRuntime, type QueryResult} from './runtime';
import {rendererFor, type RenderContext} from './renderers';
import {mountSqlEditor, type SqlEditor} from './editor';
import {sqlStyles} from './styles';
import {el, HTMLElementBase} from '../dom';

/**
 * `<dfk-sql>` — a runnable SQL example produced by `remarkRunnableSql`.
 *
 * A code block, a row of icon buttons floating in its top-right corner on
 * hover (run / reset / wrap / copy — the same idiom as Docusaurus' own code
 * blocks), and a result area that appears only once something has run. The
 * CodeMirror editor *is* the code view: there is no read-only preview and no
 * edit mode to enter.
 *
 * Everything but the result is in the shadow root. The result is a slotted
 * light-DOM sibling because VTable injects a *document-level* stylesheet that a
 * shadow boundary could not host — and it is only created when a query runs,
 * long after hydration, so the light DOM still starts empty (AGENTS.md rule 11).
 * The editor has no such problem: it sits in this shadow root, so CodeMirror's
 * style-mod resolves the root to the same tree its styles are used in.
 *
 * Retained-mode: every node is built once in the constructor and held in a
 * field; the `#set*` / `#show*` helpers mutate the nodes they own. Each block
 * keeps its own `#currentSql`, so editing one never touches another.
 *
 * The fullscreen toggle is the exception: it belongs in the result's tab strip,
 * which a renderer builds, so the component keeps the node (and its state) and
 * hands it over through `RenderContext.fullscreenButton`.
 *
 * Content entry is an **attribute seed** (see AGENTS.md rule 5 exception): the
 * `config` / `sql` attributes are read once in `connectedCallback` because the
 * remark-generated JSX cannot hand content through a ref setter. Reading once
 * to initialise is not an attribute→render loop, so the retained-mode contract
 * still holds.
 */

// A `type` alias (not an interface) so it satisfies `RenderContext`'s
// `Record<string, string>` index signature. It carries the renderers' strings
// too, because a renderer resolves everything through the labels it is handed.
type SqlLabels = {
  run: string;
  reset: string;
  /** Tooltip for the wrap toggle while wrapping is *off* (i.e. "turn it on"). */
  wrapOn: string;
  /** Tooltip for the wrap toggle while wrapping is *on* (i.e. "turn it off"). */
  wrapOff: string;
  copy: string;
  copied: string;
  running: string;
  initializing: string;
  loadingExtensions: string;
  initFailed: string;
  error: string;
  fullscreen: string;
  exitFullscreen: string;
  table: string;
  text: string;
  row: string;
  noField: string;
};

const LABELS: Record<string, SqlLabels> = {
  en: {
    run: 'Run',
    reset: 'Reset',
    wrapOn: 'Wrap long lines',
    wrapOff: 'Stop wrapping lines',
    copy: 'Copy',
    copied: 'Copied',
    running: 'Running…',
    initializing: 'Initializing DuckDB…',
    loadingExtensions: 'Loading extensions…',
    initFailed: 'DuckDB failed to start',
    error: 'Error',
    fullscreen: 'Fullscreen',
    exitFullscreen: 'Exit fullscreen',
    table: 'Table',
    text: 'Text',
    row: 'Row',
    noField: 'this result has no markup column; set `field`',
  },
  'zh-hans': {
    run: '执行',
    reset: '重置',
    wrapOn: '折行显示',
    wrapOff: '取消折行',
    copy: '复制',
    copied: '已复制',
    running: '执行中…',
    initializing: '正在初始化 DuckDB…',
    loadingExtensions: '正在加载扩展…',
    initFailed: 'DuckDB 初始化失败',
    error: '错误',
    fullscreen: '全屏',
    exitFullscreen: '退出全屏',
    table: '表格',
    text: '文本',
    row: '行',
    noField: '该结果没有可展示的标记列，请设置 `field`',
  },
};

/** How long the copy button shows its "copied" confirmation, in ms. */
const COPIED_HOLD = 1600;

export class DfkSql extends HTMLElementBase {
  /** The CodeMirror host; it lives in the shadow tree, so `DfkSql.css` styles it. */
  readonly #editorHost = el('div', {class: 'dfk-sql-editor'});
  /** Wraps the editor and anchors the floating action cluster. */
  readonly #code = el('div', {class: 'dfk-sql-code'});
  readonly #actions = el('div', {class: 'dfk-sql-actions'});
  readonly #status = el('span', {class: 'dfk-sql-status', attrs: {'aria-live': 'polite'}});
  readonly #runBtn: IconButton;
  readonly #resetBtn: IconButton;
  readonly #wrapBtn: IconButton;
  readonly #copyBtn: IconButton;
  readonly #fullscreenBtn: IconButton;

  #labels: SqlLabels = LABELS.en;
  #config: RunnableSqlConfig = {type: 'duckfn'};
  #originalSql = '';
  #currentSql = '';
  /** Long SQL lines are the norm in a docs example, so wrapping starts on. */
  #wrapped = true;
  #running = false;
  #seeded = false;
  #mounting = false;
  #expanded = false;
  /** Whether the document-level Esc handler is currently attached. */
  #escBound = false;
  /** Pending "copied" reset timer; 0 when none is scheduled. */
  #copyTimer = 0;
  readonly #onEsc = (event: KeyboardEvent): void => {
    if (event.key === 'Escape') {
      this.#setExpanded(false);
    }
  };

  #resultHost: HTMLElement | null = null;
  #editor: SqlEditor | null = null;
  #disposeResult: (() => void) | null = null;

  constructor() {
    super();
    this.#runBtn = new IconButton('lucide:play', () => void this.#onRun());
    this.#resetBtn = new IconButton('lucide:rotate-ccw', () => this.#onReset());
    this.#wrapBtn = new IconButton('lucide:wrap-text', () => this.#onToggleWrap());
    this.#copyBtn = new IconButton('lucide:copy', () => void this.#onCopy());
    // Not in this subtree: a renderer parks it at the right end of the result's
    // tab strip, so it sits exactly where the result's chrome is.
    this.#fullscreenBtn = new IconButton('lucide:maximize', () =>
      this.#setExpanded(!this.#expanded),
    );

    this.#actions.append(
      this.#status,
      this.#runBtn.root,
      this.#resetBtn.root,
      this.#wrapBtn.root,
      this.#copyBtn.root,
    );
    this.#code.append(this.#editorHost, this.#actions);

    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [sqlStyles()];
    // No default slot: the original code node the remark plugin keeps as a
    // child is the prerendered fallback only, and `sql.css` hides it — the
    // editor is the one code view.
    shadow.append(this.#code, el('slot', {attrs: {name: 'dfk-result'}}));
    this.#applyLabels();
  }

  connectedCallback(): void {
    if (!this.#seeded) {
      this.#seed();
      this.#seeded = true;
    }
    void this.#mountEditor();
  }

  disconnectedCallback(): void {
    this.#setExpanded(false);
    this.#disposeResult?.();
    this.#disposeResult = null;
    this.#editor?.destroy();
    this.#editor = null;
    if (this.#copyTimer !== 0) {
      window.clearTimeout(this.#copyTimer);
      this.#copyTimer = 0;
    }
  }

  /** Reads the `config` / `sql` attributes once. */
  #seed(): void {
    this.#labels =
      LABELS[(document.documentElement.getAttribute('lang') ?? 'en').toLowerCase()] ??
      LABELS.en;
    const raw = this.getAttribute('config');
    if (raw) {
      try {
        this.#config = JSON.parse(raw) as RunnableSqlConfig;
      } catch {
        // Malformed config: fall back to the default `table` view.
      }
    }
    this.#originalSql = this.getAttribute('sql') ?? '';
    this.#currentSql = this.#originalSql;
    this.#applyLabels();
  }

  #applyLabels(): void {
    this.#runBtn.setLabel(this.#labels.run);
    this.#resetBtn.setLabel(this.#labels.reset);
    this.#wrapBtn.setLabel(this.#wrapped ? this.#labels.wrapOff : this.#labels.wrapOn);
    this.#wrapBtn.setOn(this.#wrapped);
    this.#copyBtn.setLabel(this.#labels.copy);
    this.#fullscreenBtn.setLabel(
      this.#expanded ? this.#labels.exitFullscreen : this.#labels.fullscreen,
    );
  }

  // --- Editor ----------------------------------------------------------------

  async #mountEditor(): Promise<void> {
    if (this.#editor || this.#mounting) {
      return;
    }
    this.#mounting = true;
    try {
      const editor = await mountSqlEditor(this.#editorHost, this.#currentSql, (value) => {
        this.#currentSql = value;
      });
      if (!this.isConnected) {
        // Disconnected while the CodeMirror modules were loading: nothing will
        // ever dispose this editor, so dispose it here.
        editor.destroy();
        return;
      }
      editor.setWrap(this.#wrapped);
      this.#editor = editor;
    } catch (error) {
      this.#setStatus(messageOf(error));
    } finally {
      this.#mounting = false;
    }
  }

  // --- Code-block actions ----------------------------------------------------

  #onReset(): void {
    this.#currentSql = this.#originalSql;
    this.#editor?.setValue(this.#currentSql);
    this.#clearResult();
  }

  #onToggleWrap(): void {
    this.#wrapped = !this.#wrapped;
    this.#editor?.setWrap(this.#wrapped);
    this.#applyLabels();
  }

  async #onCopy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(this.#currentSql);
    } catch {
      // Clipboard access can be denied (insecure context, permissions); the
      // button simply keeps its idle look.
      return;
    }
    this.#copyBtn.setIcon('lucide:check');
    this.#copyBtn.setLabel(this.#labels.copied);
    if (this.#copyTimer !== 0) {
      window.clearTimeout(this.#copyTimer);
    }
    this.#copyTimer = window.setTimeout(() => {
      this.#copyTimer = 0;
      this.#copyBtn.setIcon('lucide:copy');
      this.#copyBtn.setLabel(this.#labels.copy);
    }, COPIED_HOLD);
  }

  // --- Fullscreen ------------------------------------------------------------

  /**
   * The whole fullscreen state: the result panel becomes a fixed overlay and
   * its own tab strip carries the toggle back out (now "Exit fullscreen"). Esc
   * exits too, from anywhere on the page.
   */
  #setExpanded(value: boolean): void {
    this.#expanded = value;
    this.#resultHost?.classList.toggle('dfk-sql-result-expanded', value);
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

  // --- Run -------------------------------------------------------------------

  async #onRun(): Promise<void> {
    if (this.#running) {
      return; // Guard against double-clicks running the same example twice.
    }
    this.#running = true;
    this.#setBusy(true);
    const runtime = DuckDBRuntime.getInstance();
    try {
      if (runtime.state !== 'ready') {
        this.#setStatus(this.#labels.initializing);
      }
      try {
        // The instance-wide settings come from whichever block initialises the
        // shared runtime first; see `RuntimeOptions`.
        await runtime.init({
          allowUnsignedExtensions: this.#config.allowUnsignedExtensions === true,
        });
      } catch {
        this.#showInitError();
        return;
      }

      const extensions = this.#config.extensions ?? [];
      if (extensions.length > 0) {
        this.#setStatus(this.#labels.loadingExtensions);
        try {
          for (const name of extensions) {
            await runtime.loadExtension(name, {repository: this.#config.repository});
          }
        } catch (error) {
          this.#renderInto(errorText(this.#labels, messageOf(error)));
          return;
        }
      }

      this.#setStatus(this.#labels.running);
      const result = await runtime.execute(this.#currentSql);
      this.#showResult(result);
    } finally {
      this.#running = false;
      this.#setBusy(false);
      this.#setStatus('');
    }
  }

  #setBusy(busy: boolean): void {
    this.#runBtn.setDisabled(busy);
    this.#resetBtn.setDisabled(busy);
    // A run started from a click keeps the cluster on screen, so the progress
    // status in it is actually readable.
    this.#actions.classList.toggle('dfk-sql-actions-busy', busy);
  }

  #setStatus(text: string): void {
    this.#status.textContent = text;
  }

  #showInitError(): void {
    const message = DuckDBRuntime.getInstance().message;
    this.#renderInto(this.#labels.initFailed + (message ? `: ${message}` : ''));
  }

  // --- Result rendering ------------------------------------------------------

  #ensureResultHost(): HTMLElement {
    if (!this.#resultHost) {
      this.#resultHost = el('div', {class: 'dfk-sql-result'});
      this.#resultHost.slot = 'dfk-result';
      this.appendChild(this.#resultHost);
    }
    this.#resultHost.hidden = false;
    return this.#resultHost;
  }

  #showResult(result: QueryResult): void {
    this.#disposeResult?.();
    this.#disposeResult = null;
    // An error view carries no chrome, so a fullscreen overlay built from a
    // previous result would leave no visible way out (only Esc).
    if (result.error) {
      this.#setExpanded(false);
    }
    const host = this.#ensureResultHost();
    const context: RenderContext = {
      host,
      config: this.#config,
      labels: this.#labels,
      fullscreenButton: this.#fullscreenBtn.root,
    };
    const renderer = rendererFor(this.#config, result);
    void renderer(context, result).then((dispose) => {
      this.#disposeResult = dispose ?? null;
    });
  }

  #renderInto(text: string): void {
    this.#setExpanded(false);
    const host = this.#ensureResultHost();
    const pre = host.ownerDocument.createElement('pre');
    pre.className = 'dfk-sql-error';
    pre.textContent = text;
    host.replaceChildren(pre);
  }

  #clearResult(): void {
    this.#setExpanded(false);
    this.#disposeResult?.();
    this.#disposeResult = null;
    if (this.#resultHost) {
      this.#resultHost.replaceChildren();
      this.#resultHost.hidden = true;
    }
  }
}

/**
 * A compact icon-only button with a hover tooltip, built once. The tooltip is
 * also the accessible name — an icon-only control has no text to fall back on.
 */
class IconButton {
  readonly root = el('button', {class: 'dfk-sql-icon-button', type: 'button'});
  readonly #icon: IconifyIconHTMLElement = el('iconify-icon', {
    class: 'dfk-sql-icon',
    attrs: {'aria-hidden': 'true'},
  });

  constructor(icon: string, onClick: () => void) {
    this.root.appendChild(this.#icon);
    this.root.addEventListener('click', onClick);
    this.setIcon(icon);
  }

  setIcon(icon: string): void {
    this.#icon.setAttribute('icon', icon);
  }

  setLabel(text: string): void {
    this.root.setAttribute('data-tip', text);
    this.root.setAttribute('aria-label', text);
  }

  /** Marks a toggle as currently on (the wrap button). */
  setOn(on: boolean): void {
    this.root.classList.toggle('dfk-sql-icon-on', on);
  }

  setDisabled(disabled: boolean): void {
    if (disabled) {
      this.root.setAttribute('disabled', '');
    } else {
      this.root.removeAttribute('disabled');
    }
  }
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function errorText(labels: SqlLabels, detail: string): string {
  return `${labels.error}: ${detail}`;
}