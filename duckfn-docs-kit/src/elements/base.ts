import {el, HTMLElementBase} from '../dom';

/**
 * Base class for the `dfk-*` custom elements — persistent DOM, not a render loop.
 *
 * `create()` runs exactly once and builds the whole skeleton; every node the
 * component later touches is kept in a class field. `apply(data)` only mutates
 * those held nodes (`textContent`, `setAttribute`, …). There is no teardown,
 * no `replaceChildren()` of the subtree, nothing that re-creates nodes when the
 * data changes — the DOM is built once and then edited in place.
 *
 * Contract: a component receives all of its content through the `data`
 * property, never through attributes or light-DOM children. Setting `data`
 * before the element is connected is fine; `connectedCallback` picks it up.
 *
 * Light DOM on purpose: the components render into the document tree, not a
 * shadow root, so they keep seeing the site's Infima variables, the
 * `@layer docusaurus.theme-classic` tricks and the `[data-theme]` switch, and
 * their classes are the global `dfk-*` ones from `home.css`.
 */
export abstract class DfkElement<TData> extends HTMLElementBase {
  #data: TData | null = null;
  #created = false;

  set data(value: TData) {
    this.#data = value;
    if (this.#created) {
      this.apply(value);
    }
  }

  get data(): TData | null {
    return this.#data;
  }

  connectedCallback(): void {
    if (this.#created) {
      return;
    }
    this.#created = true;
    this.create();
    if (this.#data !== null) {
      this.apply(this.#data);
    }
  }

  /** Builds the skeleton once. Must only use `el()` / `createElement`. */
  protected abstract create(): void;

  /** Mutates the held nodes to reflect `data`. Must not create or remove nodes. */
  protected abstract apply(data: TData): void;

  /** `el()` re-exposed as a protected member so subclasses need no extra import. */
  protected el<K extends keyof HTMLElementTagNameMap>(
    tag: K,
    options?: Parameters<typeof el<K>>[1],
  ): HTMLElementTagNameMap[K] {
    return el(tag, options);
  }
}
