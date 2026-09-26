import {applyIconMask} from '../icons';
import type {FeatureItem, FeaturesData} from '../types';
import {DfkElement} from './base';

/**
 * `<dfk-features>` — the "Why duckfn" grid of feature cards. Ported from the
 * home page's `Features()`; each card is built by {@link DfkFeatureCard}, which
 * holds its own nodes as fields.
 */
export class DfkFeatures extends DfkElement<FeaturesData> {
  protected override build(data: FeaturesData): void {
    const section = this.el('section', {class: 'dfk-section'});
    const inner = this.el('div', {class: 'dfk-section-inner'});
    const heading = this.el('h2', {class: 'dfk-section-title', text: data.sectionTitle});

    const grid = this.el('div', {class: 'dfk-feature-grid'});
    for (const item of data.items) {
      grid.appendChild(new DfkFeatureCard(item).render());
    }

    inner.append(heading, grid);
    section.appendChild(inner);
    this.appendChild(section);
  }
}

/** One feature card: an icon chip, a title and a description. */
class DfkFeatureCard {
  readonly #item: FeatureItem;
  readonly #root: HTMLElement;
  readonly #chip: HTMLElement;
  readonly #title: HTMLHeadingElement;
  readonly #details: HTMLParagraphElement;

  constructor(item: FeatureItem) {
    this.#item = item;
    this.#root = document.createElement('article');
    this.#root.className = 'dfk-feature-card';
    this.#chip = document.createElement('span');
    this.#chip.className = 'dfk-feature-icon-chip';
    this.#title = document.createElement('h3');
    this.#title.className = 'dfk-feature-title';
    this.#details = document.createElement('p');
    this.#details.className = 'dfk-feature-details';
  }

  render(): HTMLElement {
    const icon = document.createElement('span');
    icon.className = 'dfk-icon dfk-feature-icon';
    applyIconMask(icon, this.#item.icon);
    this.#chip.appendChild(icon);

    this.#title.textContent = this.#item.title;
    this.#details.textContent = this.#item.details;

    this.#root.replaceChildren(this.#chip, this.#title, this.#details);
    return this.#root;
  }
}
