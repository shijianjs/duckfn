import type {IconifyIconHTMLElement} from 'iconify-icon';
import type {FeatureItem, FeaturesData} from '../types';
import {DfkElement} from './base';

/**
 * `<dfk-features>` — the "Why duckfn" grid of feature cards. Ported from the
 * home page's `Features()`.
 *
 * Persistent DOM: the grid skeleton is built once, and each card is a
 * {@link DfkFeatureCard} that holds its own nodes. `apply()` grows or shrinks
 * the card list to the data length and mutates the cards in place — the grid is
 * never cleared and rebuilt.
 */
export class DfkFeatures extends DfkElement<FeaturesData> {
  readonly #heading = this.el('h2', {class: 'dfk-section-title'});
  readonly #grid = this.el('div', {class: 'dfk-feature-grid'});
  #cards: DfkFeatureCard[] = [];

  protected override create(): void {
    const inner = this.el('div', {class: 'dfk-section-inner'});
    inner.append(this.#heading, this.#grid);
    const section = this.el('section', {class: 'dfk-section'});
    section.appendChild(inner);
    this.appendChild(section);
  }

  protected override apply(data: FeaturesData): void {
    this.#heading.textContent = data.sectionTitle;
    while (this.#cards.length > data.items.length) {
      this.#cards.pop()?.root.remove();
    }
    while (this.#cards.length < data.items.length) {
      const card = new DfkFeatureCard();
      this.#cards.push(card);
      this.#grid.appendChild(card.root);
    }
    data.items.forEach((item, i) => this.#cards[i].apply(item));
  }
}

/** One feature card: an icon chip, a title and a description. Built once. */
class DfkFeatureCard {
  readonly root: HTMLElement;
  readonly #icon: IconifyIconHTMLElement;
  readonly #title: HTMLHeadingElement;
  readonly #details: HTMLParagraphElement;

  constructor() {
    this.root = document.createElement('article');
    this.root.className = 'dfk-feature-card';
    const chip = document.createElement('span');
    chip.className = 'dfk-feature-icon-chip';
    this.#icon = document.createElement('iconify-icon');
    this.#icon.className = 'dfk-feature-icon';
    this.#icon.setAttribute('aria-hidden', 'true');
    chip.appendChild(this.#icon);
    this.#title = document.createElement('h3');
    this.#title.className = 'dfk-feature-title';
    this.#details = document.createElement('p');
    this.#details.className = 'dfk-feature-details';
    this.root.append(chip, this.#title, this.#details);
  }

  apply(item: FeatureItem): void {
    this.#icon.setAttribute('icon', item.icon);
    this.#title.textContent = item.title;
    this.#details.textContent = item.details;
  }
}
