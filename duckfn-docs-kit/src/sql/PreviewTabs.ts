/**
 * The tab strip the markup renderers share: one tab per preview row, plus a
 * `Table` tab that always comes **last**, holding the raw rows in VTable.
 *
 * `iframe`, `html` and `svg` differ only in how a panel gets filled, so the tab
 * bar, the trailing table tab, the lazy mounting and the teardown are written
 * once here rather than once per renderer.
 *
 * Retained mode: every button and panel is built in the constructor and held in
 * a field. Activating a tab mutates the nodes it owns (`hidden`, `classList`,
 * `aria-selected`, `tabIndex`) — there is no rebuild, and a panel is filled the
 * first time it is shown rather than up front.
 *
 * The table panel is mounted on first activation on purpose: VTable measures
 * its container when it is constructed, and a `hidden` panel measures to zero.
 */

import {el} from '../dom';

/** One preview row: its tab label and how to fill its panel. */
export interface PreviewTabItem {
  label: string;
  mount(panel: HTMLElement): void;
}

/** What `PreviewTabs` needs from the caller to own the trailing table tab. */
export interface PreviewTableHandle {
  dispose(): void;
}

/** Keeps per-instance element ids unique across every `<dfk-sql>` on a page. */
let sequence = 0;

export class PreviewTabs {
  readonly #buttons: HTMLButtonElement[] = [];
  readonly #panels: HTMLElement[] = [];
  /** `null` marks the trailing table tab. */
  readonly #items: (PreviewTabItem | null)[] = [];
  readonly #mounted: boolean[] = [];
  readonly #mountTable: (panel: HTMLElement) => Promise<PreviewTableHandle>;

  #tableHandle: PreviewTableHandle | null = null;
  #disposed = false;

  constructor(
    host: HTMLElement,
    items: readonly PreviewTabItem[],
    tableLabel: string,
    mountTable: (panel: HTMLElement) => Promise<PreviewTableHandle>,
  ) {
    this.#mountTable = mountTable;
    const uid = `dfk-sql-tabs-${(sequence += 1)}`;
    const bar = el('div', {class: 'dfk-sql-tabs', attrs: {role: 'tablist'}});
    const panels = el('div', {class: 'dfk-sql-panels'});

    const add = (label: string, item: PreviewTabItem | null): void => {
      const index = this.#buttons.length;
      const panel = el('div', {
        class: 'dfk-sql-panel',
        hidden: true,
        attrs: {
          role: 'tabpanel',
          id: `${uid}-panel-${index}`,
          'aria-labelledby': `${uid}-tab-${index}`,
        },
      });
      const button = el('button', {
        class: 'dfk-sql-tab',
        type: 'button',
        text: label,
        tabIndex: index === 0 ? 0 : -1,
        attrs: {
          role: 'tab',
          id: `${uid}-tab-${index}`,
          'aria-controls': `${uid}-panel-${index}`,
          'aria-selected': 'false',
        },
      });
      button.addEventListener('click', () => this.#select(index));
      button.addEventListener('keydown', (event) => this.#onKeydown(event, index));

      this.#buttons.push(button);
      this.#panels.push(panel);
      this.#items.push(item);
      this.#mounted.push(false);
      bar.appendChild(button);
      panels.appendChild(panel);
    };

    for (const item of items) {
      add(item.label, item);
    }
    add(tableLabel, null);

    // The one-time installation of this widget's own subtree.
    host.replaceChildren(bar, panels);
    this.#select(0);
  }

  /** Releases the table (if it was ever shown) and empties every panel. */
  dispose(): void {
    this.#disposed = true;
    this.#tableHandle?.dispose();
    this.#tableHandle = null;
    for (const panel of this.#panels) {
      panel.replaceChildren();
    }
  }

  /** Activation is pure mutation — no panel is rebuilt, none is discarded. */
  #select(index: number): void {
    for (let i = 0; i < this.#buttons.length; i += 1) {
      const active = i === index;
      const button = this.#buttons[i];
      button.classList.toggle('dfk-sql-tab-active', active);
      button.setAttribute('aria-selected', String(active));
      button.tabIndex = active ? 0 : -1;
      this.#panels[i].hidden = !active;
    }
    if (this.#mounted[index]) {
      return;
    }
    this.#mounted[index] = true;
    const item = this.#items[index];
    if (item) {
      item.mount(this.#panels[index]);
    } else {
      void this.#mountTableInto(index);
    }
  }

  async #mountTableInto(index: number): Promise<void> {
    const handle = await this.#mountTable(this.#panels[index]);
    if (this.#disposed) {
      // Disposed while the mount was in flight: release what just arrived.
      handle.dispose();
      return;
    }
    this.#tableHandle = handle;
  }

  #onKeydown(event: KeyboardEvent, index: number): void {
    const offset = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0;
    if (offset === 0) {
      return;
    }
    event.preventDefault();
    const next = (index + offset + this.#buttons.length) % this.#buttons.length;
    this.#select(next);
    this.#buttons[next].focus();
  }
}