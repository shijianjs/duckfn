/**
 * Adds a collapse/expand control to the desktop table of contents.
 *
 * Docusaurus only has `themeConfig.docs.sidebar.hideable` for the left sidebar;
 * the right-hand TOC has no such option (`themeConfig.tableOfContents` only
 * accepts `minHeadingLevel` / `maxHeadingLevel`). So the button is injected here
 * and the layout is switched by the `toc-collapsed` class on <body>. The
 * matching CSS lives in src/css/custom.css.
 */

const STORAGE_KEY = 'duckfn:toc-collapsed';
const STATE_CLASS = 'toc-collapsed';
const BUTTON_CLASS = 'toc-toggle';
const COLUMN_CLASS = 'toc-column';
const TOC_ID = 'doc-toc';
const TOC_SELECTOR = '.theme-doc-toc-desktop';

const LABELS = {
  en: {
    hide: 'Collapse table of contents',
    show: 'Expand table of contents',
  },
  'zh-Hans': {
    hide: '收起目录',
    show: '展开目录',
  },
};

let collapsed = false;

function labels() {
  const lang = (document.documentElement.getAttribute('lang') ?? 'en').toLowerCase();
  return lang.startsWith('zh') ? LABELS['zh-Hans'] : LABELS.en;
}

function readPreference(): boolean {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
}

function storePreference(value: boolean): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, String(value));
  } catch {
    // Storage may be unavailable (private mode, blocked cookies). The toggle
    // still works for the current page; the choice is just not remembered.
  }
}

function updateLabel(button: HTMLButtonElement): void {
  const {hide, show} = labels();
  const label = collapsed ? show : hide;
  button.setAttribute('aria-label', label);
  button.setAttribute('title', label);
  button.setAttribute('aria-expanded', String(!collapsed));
}

function createButton(toc: Element): HTMLButtonElement {
  if (!toc.id) {
    toc.id = TOC_ID;
  }

  const button = document.createElement('button');
  button.type = 'button';
  button.className = `clean-btn ${BUTTON_CLASS}`;
  button.setAttribute('aria-controls', toc.id);
  button.addEventListener('click', () => {
    collapsed = !collapsed;
    storePreference(collapsed);
    apply();
  });
  updateLabel(button);
  return button;
}

/**
 * The state class goes on <body>, not on <html>: Docusaurus rewrites the whole
 * `class` attribute of <html> on every route (`docs-wrapper plugin-docs …`),
 * which would drop the class immediately after it is set. `document.body` is
 * left alone by the framework.
 */
function setStateClass(value: boolean): void {
  document.body?.classList.toggle(STATE_CLASS, value);
}

/** Reflects the current state on <body> and on the button. */
function apply(): void {
  setStateClass(collapsed);

  const button = document.querySelector<HTMLButtonElement>(`.${BUTTON_CLASS}`);
  if (button) {
    updateLabel(button);
  }
}

/**
 * Called after every route change: the TOC is rendered by React, so it only
 * exists on pages that have headings, and only on wide viewports.
 */
export function onRouteDidUpdate(): void {
  refresh();
}

function refresh(): void {
  const toc = document.querySelector(TOC_SELECTOR);
  const existing = document.querySelector<HTMLButtonElement>(`.${BUTTON_CLASS}`);

  if (!toc) {
    // No desktop TOC on this page: drop the button and the layout class, so the
    // article always uses the full width where there is nothing to collapse.
    existing?.remove();
    setStateClass(false);
    return;
  }

  toc.parentElement?.classList.add(COLUMN_CLASS);

  if (!existing) {
    const button = createButton(toc);
    toc.parentElement?.insertBefore(button, toc);
  }

  apply();
}

if (typeof window !== 'undefined') {
  collapsed = readPreference();

  // Applied before React renders, so a collapsed TOC never flashes open.
  setStateClass(collapsed);

  // React drops the desktop TOC when the viewport shrinks; re-check once it has
  // re-rendered.
  window.matchMedia('(min-width: 997px)').addEventListener('change', () => {
    window.setTimeout(refresh, 0);
  });
}
