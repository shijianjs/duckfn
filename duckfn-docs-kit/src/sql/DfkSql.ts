import type {RunnableSqlConfig} from './remark';
import {DuckDBRuntime, type QueryResult} from './runtime';
import {rendererFor, type RenderContext} from './renderers';
import {mountSqlEditor, type SqlEditor} from './editor';
import {sqlStyles} from './styles';
import {el, HTMLElementBase} from '../dom';

/**
 * `<dfk-sql>` — a runnable SQL example produced by `remarkRunnableSql`.
 *
 * Static by default: the original code block is rendered by the classic theme
 * and flows through the shadow's default `<slot>`, so it looks like any other
 * docs code block. Clicking **Edit** lazily mounts CodeMirror (in the light
 * DOM, so its global styles apply) into a named slot; clicking **Run** executes
 * the current text through the shared {@link DuckDBRuntime} and renders the
 * result with the configured {@link rendererFor} renderer.
 *
 * Retained-mode: the toolbar and slots are built once in the constructor; the
 * `#set*` / `#show*` helpers only mutate the nodes they own. Each block keeps
 * its own `#currentSql`, so editing one never touches another.
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
  edit: string;
  preview: string;
  run: string;
  reset: string;
  running: string;
  initializing: string;
  loadingExtensions: string;
  initFailed: string;
  error: string;
  fullscreen: string;
  exitFullscreen: string;
  table: string;
  row: string;
  noField: string;
};

const LABELS: Record<string, SqlLabels> = {
  en: {
    edit: 'Edit',
    preview: 'Preview',
    run: 'Run',
    reset: 'Reset',
    running: 'Running…',
    initializing: 'Initializing DuckDB…',
    loadingExtensions: 'Loading extensions…',
    initFailed: 'DuckDB failed to start',
    error: 'Error',
    fullscreen: 'Fullscreen',
    exitFullscreen: 'Exit fullscreen',
    table: 'Table',
    row: 'Row',
    noField: 'this result has no markup column; set `field`',
  },
  'zh-hans': {
    edit: '编辑',
    preview: '预览',
    run: '执行',
    reset: '重置',
    running: '执行中…',
    initializing: '正在初始化 DuckDB…',
    loadingExtensions: '正在加载扩展…',
    initFailed: 'DuckDB 初始化失败',
    error: '错误',
    fullscreen: '全屏',
    exitFullscreen: '退出全屏',
    table: '表格',
    row: '行',
    noField: '该结果没有可展示的标记列，请设置 `field`',
  },
};

type Mode = 'static' | 'edit';

export class DfkSql extends HTMLElementBase {
  // Toolbar buttons are mutable so the Vaadin upgrade can swap the nodes.
  #editBtn: HTMLElement;
  #runBtn: HTMLElement;
  #resetBtn: HTMLElement;
  #fullscreenBtn: HTMLElement;
  readonly #editLabel = el('span');
  readonly #runLabel = el('span');
  readonly #resetLabel = el('span');
  readonly #fullscreenLabel = el('span');
  readonly #status = el('span', {class: 'dfk-sql-status', attrs: {'aria-live': 'polite'}});
  readonly #toolbar = el('div', {class: 'dfk-sql-toolbar'});

  #labels: SqlLabels = LABELS.en;
  #config: RunnableSqlConfig = {type: 'duckfn'};
  #originalSql = '';
  #currentSql = '';
  #mode: Mode = 'static';
  #running = false;
  #seeded = false;
  #upgraded = false;
  #expanded = false;
  /** Whether the document-level Esc handler is currently attached. */
  #escBound = false;
  readonly #onEsc = (event: KeyboardEvent): void => {
    if (event.key === 'Escape') {
      this.#setExpanded(false);
    }
  };

  #static: HTMLElement | null = null;
  #editorHost: HTMLElement | null = null;
  #resultHost: HTMLElement | null = null;
  #editor: SqlEditor | null = null;
  #disposeResult: (() => void) | null = null;

  constructor() {
    super();
    this.#editBtn = this.#makeButton('dfk-sql-button', this.#editLabel, () =>
      this.#onToggleEdit(),
    );
    this.#runBtn = this.#makeButton(
      'dfk-sql-button dfk-sql-button-primary',
      this.#runLabel,
      () => this.#onRun(),
    );
    this.#resetBtn = this.#makeButton('dfk-sql-button', this.#resetLabel, () =>
      this.#onReset(),
    );
    this.#fullscreenBtn = this.#makeButton('dfk-sql-button', this.#fullscreenLabel, () =>
      this.#onToggleFullscreen(),
    );
    this.#fullscreenBtn.setAttribute('disabled', '');
    this.#toolbar.append(
      this.#editBtn,
      this.#runBtn,
      this.#resetBtn,
      this.#fullscreenBtn,
      this.#status,
    );

    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [sqlStyles()];
    // Default slot: the slotted static CodeBlock. Named slots: the lazily
    // created editor / result hosts (they carry matching `slot` attributes),
    // so they never fall into the default slot.
    shadow.append(
      this.#toolbar,
      el('slot'),
      el('slot', {attrs: {name: 'dfk-editor'}}),
      el('slot', {attrs: {name: 'dfk-result'}}),
    );
    this.#applyLabels();
  }

  #makeButton(className: string, label: HTMLElement, onClick: () => void): HTMLElement {
    const button = el('button', {class: className, type: 'button'});
    button.appendChild(label);
    button.addEventListener('click', onClick);
    return button;
  }

  connectedCallback(): void {
    if (!this.#seeded) {
      this.#seed();
      this.#seeded = true;
    }
    // Vaadin upgrade is best-effort and browser-only; native buttons already
    // work, so a failed import (offline, older browser) is silently ignored.
    if (!this.#upgraded) {
      this.#upgraded = true;
      void this.#upgradeToVaadin();
    }
  }

  disconnectedCallback(): void {
    this.#setExpanded(false);
    this.#disposeResult?.();
    this.#disposeResult = null;
    this.#editor?.destroy();
    this.#editor = null;
  }

  /** Reads the `config` / `sql` attributes once and wires the static preview. */
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
    this.#static = this.firstElementChild as HTMLElement | null;
    this.#applyLabels();
  }

  #applyLabels(): void {
    this.#editLabel.textContent = this.#mode === 'edit' ? this.#labels.preview : this.#labels.edit;
    this.#runLabel.textContent = this.#labels.run;
    this.#resetLabel.textContent = this.#labels.reset;
    this.#fullscreenLabel.textContent = this.#expanded
      ? this.#labels.exitFullscreen
      : this.#labels.fullscreen;
  }

  async #upgradeToVaadin(): Promise<void> {
    try {
      await import('@vaadin/button');
    } catch {
      return; // Keep the native buttons.
    }
    const swap = (old: HTMLElement, className: string, label: HTMLElement, handler: () => void) => {
      const vb = document.createElement('vaadin-button');
      vb.className = className;
      // The upgrade can land mid-run (or before any result exists), so the
      // replacement starts in the state the button it replaces was in.
      if (old.hasAttribute('disabled')) {
        vb.setAttribute('disabled', '');
      }
      vb.appendChild(label);
      vb.addEventListener('click', handler);
      old.replaceWith(vb);
      return vb;
    };
    this.#editBtn = swap(this.#editBtn, 'dfk-sql-button', this.#editLabel, () =>
      this.#onToggleEdit(),
    );
    this.#runBtn = swap(
      this.#runBtn,
      'dfk-sql-button dfk-sql-button-primary',
      this.#runLabel,
      () => this.#onRun(),
    );
    this.#resetBtn = swap(this.#resetBtn, 'dfk-sql-button', this.#resetLabel, () =>
      this.#onReset(),
    );
    this.#fullscreenBtn = swap(
      this.#fullscreenBtn,
      'dfk-sql-button',
      this.#fullscreenLabel,
      () => this.#onToggleFullscreen(),
    );
    this.#toolbar.replaceChildren(
      this.#editBtn,
      this.#runBtn,
      this.#resetBtn,
      this.#fullscreenBtn,
      this.#status,
    );
  }

  // --- Edit / preview toggle -------------------------------------------------

  #onToggleEdit(): void {
    if (this.#mode === 'static') {
      void this.#enterEdit();
    } else {
      this.#exitEdit();
    }
  }

  async #enterEdit(): Promise<void> {
    this.#mode = 'edit';
    this.#applyLabels();
    if (this.#static) {
      this.#static.hidden = true;
    }
    if (!this.#editorHost) {
      this.#editorHost = el('div', {class: 'dfk-sql-editor'});
      this.#editorHost.slot = 'dfk-editor';
      this.appendChild(this.#editorHost);
    }
    this.#editorHost.hidden = false;
    if (!this.#editor) {
      this.#editor = await mountSqlEditor(this.#editorHost, this.#currentSql, (value) => {
        this.#currentSql = value;
      });
    }
  }

  #exitEdit(): void {
    this.#mode = 'static';
    this.#applyLabels();
    if (this.#editorHost) {
      this.#editorHost.hidden = true;
    }
    if (this.#static) {
      this.#static.hidden = false;
    }
  }

  // --- Reset -----------------------------------------------------------------

  #onReset(): void {
    this.#currentSql = this.#originalSql;
    this.#editor?.setValue(this.#currentSql);
    this.#clearResult();
  }

  // --- Fullscreen ------------------------------------------------------------

  #onToggleFullscreen(): void {
    if (!this.#resultHost) {
      return; // Nothing to expand yet; the button is disabled until then.
    }
    this.#setExpanded(!this.#expanded);
  }

  /**
   * The whole fullscreen state: the result panel becomes a fixed overlay, and
   * the shadow toolbar floats above it so the toggle (now labelled "Exit
   * fullscreen") stays reachable. Esc exits too, from anywhere on the page.
   */
  #setExpanded(value: boolean): void {
    this.#expanded = value;
    this.#resultHost?.classList.toggle('dfk-sql-result-expanded', value);
    this.#toolbar.classList.toggle('dfk-sql-toolbar-float', value);
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
    for (const button of [this.#editBtn, this.#runBtn, this.#resetBtn]) {
      if (busy) {
        button.setAttribute('disabled', '');
      } else {
        button.removeAttribute('disabled');
      }
    }
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
    // There is something to expand from here on.
    this.#fullscreenBtn.removeAttribute('disabled');
    return this.#resultHost;
  }

  #showResult(result: QueryResult): void {
    this.#disposeResult?.();
    this.#disposeResult = null;
    const host = this.#ensureResultHost();
    const context: RenderContext = {host, config: this.#config, labels: this.#labels};
    const renderer = rendererFor(this.#config, result);
    void renderer(context, result).then((dispose) => {
      this.#disposeResult = dispose ?? null;
    });
  }

  #renderInto(text: string): void {
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
    this.#fullscreenBtn.setAttribute('disabled', '');
    if (this.#resultHost) {
      this.#resultHost.replaceChildren();
      this.#resultHost.hidden = true;
    }
  }
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function errorText(labels: SqlLabels, detail: string): string {
  return `${labels.error}: ${detail}`;
}
