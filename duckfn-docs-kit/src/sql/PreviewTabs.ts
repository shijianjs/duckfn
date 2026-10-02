import {el} from '../dom';
import {saveDownload, type DownloadPayload} from '../download';
import {IconButton} from '../IconButton';

/**
 * Every renderer's result shell: a tab strip plus the panels behind it.
 *
 * The strip holds one tab per item — a preview row, a `Text` view, or the
 * trailing `Table` view — and, at its right end, the chrome that acts on the
 * result as a whole: the active tab's own {@link PreviewTabItem.actions}, a
 * **download** button, and whatever the caller parks last (the fullscreen toggle
 * `<dfk-sql>` owns). Every result therefore has the same chrome, and the panel
 * behind a tab is built the first time that tab is shown.
 *
 * Retained mode: every button, action container and panel is built in the
 * constructor and held in a field. Activating a tab mutates the nodes it owns
 * (`hidden`, `classList`, `aria-selected`, `tabIndex`) — there is no rebuild, and
 * a panel is filled the first time it is shown rather than up front.
 */

/** One tab: its label, how to fill its panel, and what it offers the strip. */
export interface PreviewTabItem {
  label: string;
  /**
   * Fills the panel. Called once, the first time the tab is shown; an async mount
   * may resolve with a disposer, which the strip runs when it is disposed (that is
   * how the table's handle gets released, including when the countdown lands after
   * {@link PreviewTabs.dispose}).
   */
  mount(panel: HTMLElement): void | (() => void) | Promise<void | (() => void)>;
  /**
   * Controls for the strip's right end, shown only while this tab is active — the
   * place for actions that belong to *this* result (a table's search and view
   * switches, a figure's zoom reset) rather than to every result.
   */
  actions?: HTMLElement;
  /** This tab's file, or `null` while there is nothing to save yet. */
  download?: () => DownloadPayload | null;
  /** Told the result area's fullscreen state, for figures that zoom inside it. */
  setFullscreen?: (value: boolean) => void;
}

/** Keeps per-instance element ids unique across every `<dfk-sql>` on a page. */
let sequence = 0;

export class PreviewTabs {
  readonly #buttons: HTMLButtonElement[] = [];
  readonly #panels: HTMLElement[] = [];
  readonly #items: PreviewTabItem[] = [];
  readonly #mounted: boolean[] = [];
  /** Disposers returned by mounts, run on {@link dispose}. */
  readonly #disposers: (() => void)[] = [];
  readonly #downloadBtn: IconButton;

  #active = 0;
  #fullscreen = false;
  #disposed = false;

  /**
   * @param corner Node parked at the very end of the strip, after the download
   * button. `<dfk-sql>` passes its fullscreen toggle: it owns that button's state,
   * so it owns the node and only lends it here.
   */
  constructor(
    host: HTMLElement,
    items: readonly PreviewTabItem[],
    downloadLabel: string,
    corner?: HTMLElement,
  ) {
    const uid = `dfk-sql-tabs-${(sequence += 1)}`;
    const bar = el('div', {class: 'dfk-sql-tabs'});
    // Only the tab buttons belong to the tablist; the corner must not be
    // scrollable with them, hence the nested list.
    const list = el('div', {
      class: 'dfk-sql-tab-list',
      attrs: {role: 'tablist'},
    });
    const cornerBox = el('div', {class: 'dfk-sql-tab-corner'});
    const panels = el('div', {class: 'dfk-sql-panels'});

    const add = (item: PreviewTabItem): void => {
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
        text: item.label,
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
      list.appendChild(button);
      panels.appendChild(panel);
      if (item.actions) {
        item.actions.hidden = true;
        cornerBox.appendChild(item.actions);
      }
    };

    for (const item of items) {
      add(item);
    }

    this.#downloadBtn = new IconButton('lucide:download', () => this.#download());
    this.#downloadBtn.setLabel(downloadLabel);
    this.#downloadBtn.root.hidden = true;
    cornerBox.appendChild(this.#downloadBtn.root);
    if (corner) {
      cornerBox.appendChild(corner);
    }

    bar.append(list, cornerBox);
    // The one-time installation of this widget's own subtree.
    host.replaceChildren(bar, panels);
    this.#select(0);
  }

  /** Releases every mounted item and empties the panels. */
  dispose(): void {
    this.#disposed = true;
    for (const dispose of this.#disposers) {
      dispose();
    }
    this.#disposers.length = 0;
    for (const panel of this.#panels) {
      panel.replaceChildren();
    }
  }

  /**
   * Records the result area's fullscreen state and passes it on to the tab on
   * screen: a figure zooms only where the result is expanded (see `PanZoomView`).
   */
  setFullscreen(value: boolean): void {
    this.#fullscreen = value;
    this.#items[this.#active]?.setFullscreen?.(value);
  }

  /** Activation is pure mutation — no panel is rebuilt, none is discarded. */
  #select(index: number): void {
    this.#active = index;
    for (let i = 0; i < this.#buttons.length; i += 1) {
      const active = i === index;
      const button = this.#buttons[i];
      button.classList.toggle('dfk-sql-tab-active', active);
      button.setAttribute('aria-selected', String(active));
      button.tabIndex = active ? 0 : -1;
      this.#panels[i].hidden = !active;
      const actions = this.#items[i].actions;
      if (actions) {
        actions.hidden = !active;
      }
    }
    const item = this.#items[index];
    // Only a tab that can save something gets a live download button; a figure
    // that has not rendered yet answers `null` to the click instead.
    this.#downloadBtn.root.hidden = item.download === undefined;
    item.setFullscreen?.(this.#fullscreen);
    if (this.#mounted[index]) {
      return;
    }
    this.#mounted[index] = true;
    void this.#mountInto(index);
  }

  async #mountInto(index: number): Promise<void> {
    const dispose = await this.#items[index].mount(this.#panels[index]);
    if (typeof dispose !== 'function') {
      return;
    }
    if (this.#disposed) {
      // Disposed while the mount was in flight: release what just arrived.
      dispose();
      return;
    }
    this.#disposers.push(dispose);
  }

  #download(): void {
    const payload = this.#items[this.#active]?.download?.();
    if (payload) {
      saveDownload(payload);
    }
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
