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
// `Record<string, string>` index signature.
type SqlLabels = {
  edit: string;
  preview: string;
  run: string;
  reset: string;
  running: string;
  initializing: string;
  initFailed: string;
  error: string;
};

const LABELS: Record<string, SqlLabels> = {
  en: {
    edit: 'Edit',
    preview: 'Preview',
    run: 'Run',
    reset: 'Reset',
    running: 'Running…',
    initializing: 'Initializing DuckDB…',
    initFailed: 'DuckDB failed to start',
    error: 'Error',
  },
  'zh-hans': {
    edit: '编辑',
    preview: '预览',
    run: '执行',
    reset: '重置',
    running: '执行中…',
    initializing: '正在初始化 DuckDB…',
    initFailed: 'DuckDB 初始化失败',
    error: '错误',
  },
};

type Mode = 'static' | 'edit';

export class DfkSql extends HTMLElementBase {
  // Toolbar buttons are mutable so the Vaadin upgrade can swap the nodes.
  #editBtn: HTMLElement;
  #runBtn: HTMLElement;
  #resetBtn: HTMLElement;
  readonly #editLabel = el('span');
  readonly #runLabel = el('span');
  readonly #resetLabel = el('span');
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
    this.#toolbar.append(
      this.#editBtn,
      this.#runBtn,
      this.#resetBtn,
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
    this.#toolbar.replaceChildren(this.#editBtn, this.#runBtn, this.#resetBtn, this.#status);
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
        await runtime.init();
      } catch {
        this.#showInitError();
        return;
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
    return this.#resultHost;
  }

  #showResult(result: QueryResult): void {
    this.#disposeResult?.();
    this.#disposeResult = null;
    const host = this.#ensureResultHost();
    const context: RenderContext = {host, config: this.#config, labels: this.#labels};
    const renderer = rendererFor(this.#config, result);
    const outcome = renderer(context, result);
    if (outcome instanceof Promise) {
      void outcome.then((dispose) => {
        this.#disposeResult = dispose;
      });
    }
  }

  #renderInto(text: string): void {
    const host = this.#ensureResultHost();
    const pre = host.ownerDocument.createElement('pre');
    pre.className = 'dfk-sql-error';
    pre.textContent = text;
    host.replaceChildren(pre);
  }

  #clearResult(): void {
    this.#disposeResult?.();
    this.#disposeResult = null;
    if (this.#resultHost) {
      this.#resultHost.replaceChildren();
      this.#resultHost.hidden = true;
    }
  }
}
