import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import {childTags, jsonAttr, textOf} from './attrs';
import {resolveHref} from './base';
import {DfkSection} from './DfkSection';
import {baseStyles, nextStepsStyles, sectionStyles} from './styles';
import type {NextStepItem} from '../types';

const TAG = 'dfk-next-steps';

/**
 * `<dfk-next-steps>` — the "Where to go next" row of link cards. Ported from
 * the home page's `NextSteps()`.
 *
 * Same retained-mode shape as `DfkFeatures`: the grid is built once, each card
 * holds its own nodes, `setSteps()` grows/shrinks the list and mutates in
 * place. The grid and the cards' boxes are inline; the card surface, the arrow's
 * hover colour and the entrance motion live in `next-steps.css`, adopted as one
 * sheet by every instance.
 *
 * Content arrives declaratively: `section-title` as text, `steps` as a JSON
 * array, or one `<dfk-step href="…" title="…">description</dfk-step>` child per
 * card (read once on connect).
 */
export class DfkNextSteps extends HTMLElementBase {
  static get observedAttributes(): string[] {
    return ['section-title', 'steps'];
  }

  readonly #section = new DfkSection();
  readonly #grid = el('div', {class: 'dfk-next-grid'}, (grid) => {
    grid.style.display = 'grid';
    grid.style.gridTemplateColumns =
      'repeat(auto-fit, minmax(min(20rem, 100%), 1fr))';
    grid.style.gap = '1rem';
    grid.style.maxWidth = '54rem';
    grid.style.margin = '2.75rem auto 0';
  });
  readonly #cards: DfkNextStepCard[] = [];

  constructor() {
    super();
    this.#section.content.appendChild(this.#grid);
    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [
      baseStyles(),
      sectionStyles(),
      nextStepsStyles(),
    ];
    shadow.appendChild(this.#section.root);
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
    this.#section.heading.textContent = text;
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
  readonly root = el('a', {class: 'dfk-next-card'}, (card) => {
    // The border, the background and the `:hover` / `:focus-visible` lift are in
    // `next-steps.css`. `color: inherit` because the card is the link and must
    // not pick up the anchor colour.
    card.style.display = 'flex';
    card.style.alignItems = 'center';
    card.style.gap = '1rem';
    card.style.padding = '1.25rem 1.375rem';
    card.style.borderRadius = '0.875rem';
    card.style.color = 'inherit';
    card.style.textDecoration = 'none';
  });
  readonly #title = el('span', {class: 'dfk-next-card-title'}, (title) => {
    title.style.fontSize = '1rem';
    title.style.fontWeight = '600';
    title.style.color = 'var(--duckfn-blue)';
  });
  readonly #details = el('span', {class: 'dfk-next-card-details'}, (details) => {
    details.style.fontSize = '0.9rem';
    details.style.lineHeight = '1.6';
    details.style.color = 'var(--ifm-color-emphasis-700)';
  });
  readonly #arrow: IconifyIconHTMLElement = el(
    'iconify-icon',
    {class: 'dfk-next-card-arrow', attrs: {icon: 'lucide:arrow-right', 'aria-hidden': 'true'}},
    (arrow) => {
      // The colour is in `next-steps.css`: the card's state changes it.
      arrow.style.flexShrink = '0';
      arrow.style.fontSize = '1.25rem';
    },
  );

  constructor() {
    // The body only groups the two text spans and is never touched again, so it
    // is described in place instead of being held in a field.
    this.root.append(
      el(
        'span',
        {class: 'dfk-next-card-body'},
        (body) => {
          body.style.display = 'flex';
          body.style.flex = '1';
          body.style.flexDirection = 'column';
          body.style.gap = '0.25rem';
          body.style.minWidth = '0';
          body.append(this.#title, this.#details);
        },
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
