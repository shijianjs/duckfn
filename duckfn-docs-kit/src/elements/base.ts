import {el, HTMLElementBase} from '../dom';

/**
 * Base class for the `dfk-*` custom elements.
 *
 * Contract: a component receives all of its content through the `data`
 * property, never through attributes or light-DOM children. Setting `data`
 * (before or after the element is connected) (re)builds the subtree; the build
 * only ever runs `document.createElement` via `el()` and keeps references to
 * the nodes it cares about in class fields — no `innerHTML`, no
 * `querySelector` round-trips.
 *
 * Light DOM on purpose: the components render into the document tree, not a
 * shadow root, so they keep seeing the site's Infima variables, the
 * `@layer docusaurus.theme-classic` tricks and the `[data-theme]` switch, and
 * their classes are the global `dfk-*` ones from `home.css`.
 */
export abstract class DfkElement<TData> extends HTMLElementBase {
  #data: TData | null = null;
  #connected = false;

  set data(value: TData) {
    this.#data = value;
    // Before connection there is nothing to build into yet; connectedCallback
    // picks the data up. After a client-side re-render the element may already
    // be connected with fresh data, so rebuild.
    if (this.#connected) {
      this.#rebuild();
    }
  }

  get data(): TData | null {
    return this.#data;
  }

  connectedCallback(): void {
    this.#connected = true;
    if (this.#data !== null && this.childElementCount === 0) {
      this.#rebuild();
    }
  }

  #rebuild(): void {
    // replaceChildren() detaches the previous subtree, which also drops its
    // event listeners together with the nodes.
    this.replaceChildren();
    if (this.#data !== null) {
      this.build(this.#data);
    }
  }

  /** Renders `data` into this element. Must only use `el()` / `createElement`. */
  protected abstract build(data: TData): void;

  /** `el()` re-exposed as a protected member so subclasses need no extra import. */
  protected el<K extends keyof HTMLElementTagNameMap>(
    tag: K,
    options?: Parameters<typeof el<K>>[1],
  ): HTMLElementTagNameMap[K] {
    return el(tag, options);
  }
}
