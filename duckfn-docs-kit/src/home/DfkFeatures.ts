import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import {childTags, jsonAttr, textOf} from './attrs';
import {DfkSection} from './DfkSection';
import {baseStyles, featureStyles, sectionStyles} from './styles';
import type {FeatureItem} from '../types';

const TAG = 'dfk-features';

/**
 * `<dfk-features>` — the "Why duckfn" grid of feature cards. Ported from the
 * home page's `Features()`.
 *
 * Retained-mode: the grid is built once and each card (a {@link DfkFeatureCard})
 * holds its own nodes. `setFeatures()` grows or shrinks the list to the new
 * length and mutates the cards in place — the grid is never cleared and rebuilt.
 *
 * The grid's layout and the cards' boxes are inline; the card surface and its
 * `:hover` lift live in `features.css`, adopted as one sheet by every instance.
 * The band around them comes from {@link DfkSection}.
 *
 * Content arrives declaratively: `section-title` as text, `features` as a JSON
 * array, or one `<dfk-feature icon="…" title="…">description</dfk-feature>`
 * child per card (read once on connect). All of them end in `setSectionTitle()`
 * / `setFeatures()`.
 */
export class DfkFeatures extends HTMLElementBase {
  static get observedAttributes(): string[] {
    return ['section-title', 'features'];
  }

  readonly #section = new DfkSection();
  readonly #grid: HTMLDivElement = el('div', {class: 'dfk-feature-grid'}, (grid) => {
    grid.style.display = 'grid';
    // `min(17rem, 100%)` rather than a bare 17rem: a track can then never be
    // wider than the viewport, which is what keeps the narrowest phones from
    // scrolling sideways.
    grid.style.gridTemplateColumns =
      'repeat(auto-fit, minmax(min(17rem, 100%), 1fr))';
    grid.style.gap = '1.25rem';
    grid.style.marginTop = '2.75rem';
  });
  readonly #cards: DfkFeatureCard[] = [];

  constructor() {
    super();
    this.#section.content.appendChild(this.#grid);
    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [
      baseStyles(),
      sectionStyles(),
      featureStyles(),
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
      case 'features': {
        const items = jsonAttr<readonly FeatureItem[]>(TAG, name, value);
        if (items) {
          this.setFeatures(items);
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

  setFeatures(items: readonly FeatureItem[]): void {
    while (this.#cards.length > items.length) {
      this.#cards.pop()?.root.remove();
    }
    while (this.#cards.length < items.length) {
      const card = new DfkFeatureCard();
      this.#cards.push(card);
      this.#grid.appendChild(card.root);
    }
    items.forEach((item, index) => this.#cards[index].setFeature(item));
  }

  /** Child tags are the readable spelling of the `features` JSON attribute. */
  #seedFromChildren(): void {
    const items = childTags(this, 'dfk-feature').map((node) => ({
      icon: node.getAttribute('icon') ?? '',
      title: node.getAttribute('title') ?? '',
      details: textOf(node),
    }));
    if (items.length > 0) {
      this.setFeatures(items);
    }
  }
}

/** One feature card: an icon chip, a title and a description. Built once. */
class DfkFeatureCard {
  readonly root = el('article', {class: 'dfk-feature-card'}, (card) => {
    // The border, the background and the `:hover` lift are in `features.css`.
    card.style.display = 'flex';
    card.style.flexDirection = 'column';
    card.style.padding = '1.5rem';
    card.style.borderRadius = '1rem';
  });
  readonly #icon: IconifyIconHTMLElement = el(
    'iconify-icon',
    {class: 'dfk-feature-icon', attrs: {'aria-hidden': 'true'}},
    (icon) => {
      icon.style.fontSize = '1.375rem';
    },
  );
  readonly #title = el('h3', {class: 'dfk-feature-title'}, (title) => {
    title.style.marginBottom = '0.5rem';
    title.style.fontSize = '1.0625rem';
    title.style.fontWeight = '600';
  });
  readonly #details = el('p', {class: 'dfk-feature-details'}, (details) => {
    details.style.marginBottom = '0';
    details.style.fontSize = '0.9375rem';
    details.style.lineHeight = '1.7';
    details.style.color = 'var(--ifm-color-emphasis-700)';
  });

  constructor() {
    // The chip only wraps the icon and is never touched again, so it is
    // described in place instead of being held in a field.
    this.root.append(
      el(
        'span',
        {class: 'dfk-feature-icon-chip'},
        (chip) => {
          chip.style.display = 'inline-flex';
          chip.style.alignItems = 'center';
          chip.style.justifyContent = 'center';
          chip.style.width = '2.5rem';
          chip.style.height = '2.5rem';
          chip.style.marginBottom = '1rem';
          chip.style.borderRadius = '0.75rem';
          chip.style.backgroundColor = 'var(--duckfn-blue-soft)';
          chip.style.color = 'var(--duckfn-blue)';
          chip.appendChild(this.#icon);
        },
      ),
      this.#title,
      this.#details,
    );
  }

  setFeature(item: FeatureItem): void {
    this.#icon.setAttribute('icon', item.icon);
    this.#title.textContent = item.title;
    this.#details.textContent = item.details;
  }
}
