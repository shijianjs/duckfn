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
 * `document.createElement` with optional class / text / attributes.
 *
 * The one sanctioned way to build nodes in this package: it returns a live
 * element (held in a class field by the caller), never an HTML string.
 */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  options?: {class?: string; text?: string; attrs?: Record<string, string>},
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (options?.class) {
    node.className = options.class;
  }
  if (options?.text !== undefined) {
    node.textContent = options.text;
  }
  if (options?.attrs) {
    for (const [name, value] of Object.entries(options.attrs)) {
      node.setAttribute(name, value);
    }
  }
  return node;
}
