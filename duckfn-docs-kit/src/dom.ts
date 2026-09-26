/**
 * DOM helpers shared by the custom elements.
 *
 * `HTMLElementBase` is the whole reason this module exists: the element classes
 * `extend` it, but Docusaurus imports this package into Node during static
 * prerendering, where the global `HTMLElement` does not exist and evaluating
 * `class X extends HTMLElement` would throw. Falling back to an empty base
 * class keeps module evaluation safe on the server; the real `HTMLElement` is
 * picked up in the browser, where the elements are actually defined and run.
 */

export const HTMLElementBase: typeof HTMLElement =
  typeof HTMLElement !== 'undefined'
    ? HTMLElement
    : (class {} as unknown as typeof HTMLElement);

/**
 * The tag's props that hold a value `el()` can assign as-is — `href`, `src`,
 * `alt`, `width`, `hidden`, and so on.
 *
 * Methods and object-valued props are filtered out, so an options bag can never
 * carry `appendChild`, `style`, `dataset` or `classList`. Getter-only props
 * (`origin`, `clientWidth`) do pass the filter; assigning one throws in strict
 * mode, which is loud enough to be caught the first time it runs.
 */
type ValueProps<T> = {
  [P in keyof T as T[P] extends Function
    ? never
    : T[P] extends object
      ? never
      : P]?: T[P];
};

/**
 * Options for `el()`: the tag's own props, narrowed to that tag, plus two
 * shorthands and the attribute escape hatch.
 */
export type ElOptions<T extends Element> = ValueProps<T> & {
  /** Shorthand for `className`. */
  class?: string;
  /** Shorthand for `textContent`. */
  text?: string;
  /**
   * Attributes that have no matching prop: `aria-*`, `data-*`, and
   * custom-element attributes. Everything else goes through a property, so a
   * boolean or a number keeps its real type instead of being stringified.
   */
  attrs?: Record<string, string>;
};

/**
 * `document.createElement` with per-tag typed options and an optional `init`
 * callback.
 *
 * The one sanctioned way to build nodes in this package: it returns a live
 * element (held in a class field by the caller), never an HTML string.
 *
 * The options are narrowed to the tag, so a typo, a wrong value type, a method
 * name or an object-valued prop is a compile error:
 *
 * ```ts
 * el('img', {class: 'dfk-logo', alt: '', width: 480});
 * ```
 *
 * `init` describes a subtree in place, for structure that is never referenced
 * again and therefore needs no field:
 *
 * ```ts
 * this.root.append(
 *   el('span', {class: 'dfk-next-card-body'}, (body) => body.append(this.#title, this.#details)),
 *   this.#arrow,
 * );
 * ```
 *
 * Nodes touched later stay in fields. A field initializer must not read a
 * `#field` declared below it (initializers run in declaration order, so that is
 * a TDZ error) — pass `init` from the constructor, where every field is ready.
 */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  // A function is the `init` callback, so options can be skipped:
  // `el('span', (span) => (span.textContent = label))`.
  options?:
    | ElOptions<HTMLElementTagNameMap[K]>
    | ((node: HTMLElementTagNameMap[K]) => void),
  init?: (node: HTMLElementTagNameMap[K]) => void,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  const build = typeof options === 'function' ? options : init;
  if (typeof options !== 'function' && options) {
    const {class: className, text, attrs, ...props} = options;
    if (className !== undefined) {
      node.className = className;
    }
    if (text !== undefined) {
      node.textContent = text;
    }
    for (const [name, value] of Object.entries(props)) {
      if (value !== undefined) {
        (node as unknown as Record<string, unknown>)[name] = value;
      }
    }
    for (const [name, value] of Object.entries(attrs ?? {})) {
      node.setAttribute(name, value);
    }
  }
  build?.(node);
  return node;
}