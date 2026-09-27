import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import {homeStyles} from '../styles';
import type {FeatureItem} from '../types';

/**
 * `<dfk-features>` — the "Why duckfn" grid of feature cards. Ported from the
 * home page's `Features()`.
 *
 * Retained-mode: the grid is built once and each card (a {@link DfkFeatureCard})
 * holds its own nodes. `setFeatures()` grows or shrinks the list to the new
 * length and mutates the cards in place — the grid is never cleared and rebuilt.
 *
 * The tree lives in a shadow root (adopting the shared `homeStyles()` sheet);
 * see `DfkHero` for why the theme crosses the boundary through custom
 * properties.
 */
export class DfkFeatures extends HTMLElementBase {
  readonly #section = el('section', {class: 'dfk-section'});
  readonly #inner = el('div', {class: 'dfk-section-inner'});
  readonly #heading = el('h2', {class: 'dfk-section-title'});
  readonly #grid = el('div', {class: 'dfk-feature-grid'});
  readonly #cards: DfkFeatureCard[] = [];

  constructor() {
    super();
    this.#inner.append(this.#heading, this.#grid);
    this.#section.appendChild(this.#inner);
    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [homeStyles()];
    shadow.appendChild(this.#section);
  }

  setSectionTitle(text: string): void {
    this.#heading.textContent = text;
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
}

/** One feature card: an icon chip, a title and a description. Built once. */
class DfkFeatureCard {
  readonly root = el('article', {class: 'dfk-feature-card'});
  readonly #icon: IconifyIconHTMLElement = el('iconify-icon', {
    class: 'dfk-feature-icon',
    attrs: {'aria-hidden': 'true'},
  });
  readonly #title = el('h3', {class: 'dfk-feature-title'});
  readonly #details = el('p', {class: 'dfk-feature-details'});

  constructor() {
    // The chip only wraps the icon and is never touched again, so it is
    // described in place instead of being held in a field.
    this.root.append(
      el('span', {class: 'dfk-feature-icon-chip'}, (chip) =>
        chip.appendChild(this.#icon),
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