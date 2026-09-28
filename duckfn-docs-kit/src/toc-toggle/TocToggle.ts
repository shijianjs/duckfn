/**
 * Adds a collapse/expand control to the desktop table of contents.
 *
 * Docusaurus only has `themeConfig.docs.sidebar.hideable` for the left sidebar;
 * the right-hand TOC has no such option (`themeConfig.tableOfContents` only
 * accepts `minHeadingLevel` / `maxHeadingLevel`). So the button is injected here
 * and the layout is switched by the `toc-collapsed` class on `<body>`. The
 * matching CSS lives in `TocToggle.css`, next to this file.
 *
 * This is the class-ified form of the original TOC glue: the button and TOC
 * references are held in fields instead of being looked up with
 * `document.querySelector` on every update, and all module-level mutable state
 * now lives inside {@link TocToggle}. `client.ts` next to this file is the thin
 * Docusaurus glue that the `toc-toggle/plugin` entry injects into a site.
 */

const DEFAULT_STORAGE_KEY = 'duckfn:toc-collapsed';
const STATE_CLASS = 'toc-collapsed';
const BUTTON_CLASS = 'toc-toggle';
const COLUMN_CLASS = 'toc-column';
const TOC_ID = 'doc-toc';
const TOC_SELECTOR = '.theme-doc-toc-desktop';
const DESKTOP_QUERY = '(min-width: 997px)';

export interface TocToggleLabels {
  hide: string;
  show: string;
}

export interface TocToggleOptions {
  /** Keyed by a lower-cased html-lang prefix; falls back to `en`. */
  labels?: Record<string, TocToggleLabels>;
  storageKey?: string;
}

const DEFAULT_LABELS: Record<string, TocToggleLabels> = {
  en: {hide: 'Collapse table of contents', show: 'Expand table of contents'},
  'zh-hans': {hide: '收起目录', show: '展开目录'},
};

export class TocToggle {
  readonly #labels: Record<string, TocToggleLabels>;
  readonly #storageKey: string;
  #collapsed = false;
  #button: HTMLButtonElement | null = null;

  constructor(options: TocToggleOptions = {}) {
    this.#labels = options.labels ?? DEFAULT_LABELS;
    this.#storageKey = options.storageKey ?? DEFAULT_STORAGE_KEY;
  }

  #currentLabels(): TocToggleLabels {
    const lang = (document.documentElement.getAttribute('lang') ?? 'en').toLowerCase();
    return this.#labels[lang] ?? this.#labels.en ?? DEFAULT_LABELS.en;
  }

  #readPreference(): boolean {
    try {
      return window.localStorage.getItem(this.#storageKey) === 'true';
    } catch {
      return false;
    }
  }

  #storePreference(value: boolean): void {
    try {
      window.localStorage.setItem(this.#storageKey, String(value));
    } catch {
      // Storage may be unavailable (private mode, blocked cookies). The toggle
      // still works for the current page; the choice is just not remembered.
    }
  }

  #updateLabel(): void {
    if (!this.#button) {
      return;
    }
    const {hide, show} = this.#currentLabels();
    const label = this.#collapsed ? show : hide;
    this.#button.setAttribute('aria-label', label);
    this.#button.setAttribute('title', label);
    this.#button.setAttribute('aria-expanded', String(!this.#collapsed));
  }

  #createButton(toc: Element): HTMLButtonElement {
    if (!toc.id) {
      toc.id = TOC_ID;
    }

    const button = document.createElement('button');
    button.type = 'button';
    button.className = `clean-btn ${BUTTON_CLASS}`;
    button.setAttribute('aria-controls', toc.id);
    button.addEventListener('click', () => {
      this.#collapsed = !this.#collapsed;
      this.#storePreference(this.#collapsed);
      this.#apply();
    });
    return button;
  }

  /**
   * The state class goes on `<body>`, not on `<html>`: Docusaurus rewrites the
   * whole `class` attribute of `<html>` on every route, which would drop the
   * class immediately after it is set. `document.body` is left alone.
   */
  #setStateClass(value: boolean): void {
    document.body?.classList.toggle(STATE_CLASS, value);
  }

  #apply(): void {
    this.#setStateClass(this.#collapsed);
    this.#updateLabel();
  }

  /**
   * Reconciles the button with the current page. The TOC is rendered by React,
   * so it only exists on pages with headings and only on wide viewports; the
   * button may also have been discarded by a re-render, so it is rebuilt when
   * missing.
   */
  refresh(): void {
    const toc = document.querySelector(TOC_SELECTOR);
    // The field may point at a button React has since removed from the DOM.
    if (this.#button && !this.#button.isConnected) {
      this.#button = null;
    }

    if (!toc) {
      // No desktop TOC on this page: drop the button and the layout class, so
      // the article always uses the full width where there is nothing to
      // collapse.
      this.#button?.remove();
      this.#button = null;
      this.#setStateClass(false);
      return;
    }

    toc.parentElement?.classList.add(COLUMN_CLASS);

    if (!this.#button) {
      this.#button = this.#createButton(toc);
      toc.parentElement?.insertBefore(this.#button, toc);
    }

    this.#apply();
  }

  /**
   * Reads the stored preference, applies it before React renders (so a collapsed
   * TOC never flashes open) and wires up the viewport listener.
   */
  init(): void {
    this.#collapsed = this.#readPreference();
    this.#setStateClass(this.#collapsed);

    // React drops the desktop TOC when the viewport shrinks; re-check once it
    // has re-rendered.
    window
      .matchMedia(DESKTOP_QUERY)
      .addEventListener('change', () => window.setTimeout(() => this.refresh(), 0));
  }
}

/**
 * Builds a {@link TocToggle}. `client.ts` — the glue the `toc-toggle/plugin`
 * entry injects — calls `init()` once (behind a `typeof window` guard) and
 * exports `onRouteDidUpdate` bound to `refresh()`.
 */
export function createTocToggle(options?: TocToggleOptions): TocToggle {
  return new TocToggle(options);
}
