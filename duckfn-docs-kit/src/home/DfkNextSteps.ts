import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import {childTags, jsonAttr, textOf} from './attrs';
import {resolveHref} from './base';
import {homeStyles} from './styles';
import type {NextStepItem} from '../types';

const TAG = 'dfk-next-steps';

/**
 * `<dfk-next-steps>` — the "Where to go next" row of link cards. Ported from
 * the home page's `NextSteps()`.
 *
 * Same retained-mode shape as `DfkFeatures`: the grid is built once, each card
 * holds its own nodes, `setSteps()` grows/shrinks the list and mutates in
 * place. The tree lives in a shadow root like the other `dfk-*` elements.
 *
 * Content arrives declaratively: `section-title` as text, `steps` as a JSON
 * array, or one `<dfk-step href="…" title="…">description</dfk-step>` child per
 * card (read once on connect).
 */
export class DfkNextSteps extends HTMLElementBase {
  static get observedAttributes(): string[] {
    return ['section-title', 'steps'];
  }

  readonly #section = el('section', {class: 'dfk-section'});
  readonly #inner = el('div', {class: 'dfk-section-inner'});
  readonly #heading = el('h2', {class: 'dfk-section-title'});
  readonly #grid = el('div', {class: 'dfk-next-grid'});
  readonly #cards: DfkNextStepCard[] = [];

  constructor() {
    super();
    this.#inner.append(this.#heading, this.#grid);
    this.#section.appendChild(this.#inner);
    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [homeStyles()];
    shadow.appendChild(this.#section);
  }

  attributeChangedCallback(
    name: string,
    _old: string | null,
    value: string | null,
  ): void {
    if (value === null) {
      return;
    }
    switch (name) {
      case 'section-title':
        this.setSectionTitle(value);
        break;
      case 'steps': {
        const items = jsonAttr<readonly NextStepItem[]>(TAG, name, value);
        if (items) {
          this.setSteps(items);
        }
        break;
      }
    }
  }

  connectedCallback(): void {
    // See `DfkHero#connectedCallback` for why the read waits one task.
    queueMicrotask(() => {
      if (this.isConnected) {
        this.#seedFromChildren();
      }
    });
  }

  setSectionTitle(text: string): void {
    this.#heading.textContent = text;
  }

  setSteps(items: readonly NextStepItem[]): void {
    while (this.#cards.length > items.length) {
      this.#cards.pop()?.root.remove();
    }
    while (this.#cards.length < items.length) {
      const card = new DfkNextStepCard();
      this.#cards.push(card);
      this.#grid.appendChild(card.root);
    }
    items.forEach((item, index) => this.#cards[index].setStep(item));
  }

  /** Child tags are the readable spelling of the `steps` JSON attribute. */
  #seedFromChildren(): void {
    const items = childTags(this, 'dfk-step').map((node) => ({
      href: node.getAttribute('href') ?? '#',
      title: node.getAttribute('title') ?? '',
      details: textOf(node),
    }));
    if (items.length > 0) {
      this.setSteps(items);
    }
  }
}

/** One "next step" card: the whole card is the link. Built once. */
class DfkNextStepCard {
  readonly root = el('a', {class: 'dfk-next-card'});
  readonly #title = el('span', {class: 'dfk-next-card-title'});
  readonly #details = el('span', {class: 'dfk-next-card-details'});
  readonly #arrow: IconifyIconHTMLElement = el('iconify-icon', {
    class: 'dfk-next-card-arrow',
    attrs: {icon: 'lucide:arrow-right', 'aria-hidden': 'true'},
  });

  constructor() {
    // The body only groups the two text spans and is never touched again, so it
    // is described in place instead of being held in a field.
    this.root.append(
      el('span', {class: 'dfk-next-card-body'}, (body) =>
        body.append(this.#title, this.#details),
      ),
      this.#arrow,
    );
  }

  setStep(item: NextStepItem): void {
    this.root.href = resolveHref(item.href);
    this.#title.textContent = item.title;
    this.#details.textContent = item.details;
  }
}
