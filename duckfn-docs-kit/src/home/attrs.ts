/**
 * Declarative input parsing for the home components.
 *
 * The hero, the feature grid and the next-step row take their content from
 * markup, the way every other custom element does. Two spellings are accepted,
 * and both end in the same named setters, so the retained-mode rules are
 * unchanged — a setter still only mutates the nodes it owns, nothing is
 * re-rendered:
 *
 * - **Attributes** — scalars as text, structures as JSON: `title="duckfn"`,
 *   `badges='[{"href": …}]'`. `attributeChangedCallback` runs the setters, so
 *   a framework that diffs attributes (React included) drives the element
 *   without a ref. This is the form to use from JSX/TSX.
 * - **Child tags** — `<dfk-action>`, `<dfk-badge>`, `<dfk-feature>`,
 *   `<dfk-step>`. Read once on connect; the form to use in hand-written HTML
 *   and MDX, where JSON inside an attribute is unreadable (MDX does not even
 *   accept single-quoted attribute values, so JSON would need `&quot;`
 *   entities).
 */

/** Parses a JSON attribute; logs and returns `undefined` when it is malformed. */
export function jsonAttr<T>(
  tag: string,
  name: string,
  value: string | null,
): T | undefined {
  if (value === null || value.trim() === '') {
    return undefined;
  }
  try {
    return JSON.parse(value) as T;
  } catch (error) {
    console.error(
      `<${tag}>: the "${name}" attribute is not valid JSON and was ignored.`,
      error,
    );
    return undefined;
  }
}

/**
 * Element children whose tag is exactly `tag` (namespaced elements and
 * non-element nodes are skipped). Reading `children` rather than
 * `querySelectorAll` keeps this working on a detached element.
 */
export function childTags(element: Element, tag: string): HTMLElement[] {
  const name = tag.toUpperCase();
  return [...element.children].filter(
    (child): child is HTMLElement => child.tagName === name,
  );
}

/**
 * The child's text as one line: markup indentation and line breaks in the
 * source collapse to single spaces, so a description split over several lines
 * in Markdown reads as one sentence.
 */
export function textOf(element: Element): string {
  return (element.textContent ?? '').replace(/\s+/g, ' ').trim();
}
