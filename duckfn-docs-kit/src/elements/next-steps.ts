import type {IconifyIconHTMLElement} from 'iconify-icon';
import type {NextStepItem, NextStepsData} from '../types';
import {DfkElement} from './base';

/**
 * `<dfk-next-steps>` — the "Where to go next" row of link cards. Ported from
 * the home page's `NextSteps()`.
 *
 * Same persistent-DOM shape as `DfkFeatures`: the grid is built once and each
 * card holds its own nodes; `apply()` grows/shrinks the list and mutates in
 * place.
 */
export class DfkNextSteps extends DfkElement<NextStepsData> {
  readonly #heading = this.el('h2', {class: 'dfk-section-title'});
  readonly #grid = this.el('div', {class: 'dfk-next-grid'});
  #cards: DfkNextStepCard[] = [];

  protected override create(): void {
    const inner = this.el('div', {class: 'dfk-section-inner'});
    inner.append(this.#heading, this.#grid);
    const section = this.el('section', {class: 'dfk-section'});
    section.appendChild(inner);
    this.appendChild(section);
  }

  protected override apply(data: NextStepsData): void {
    this.#heading.textContent = data.sectionTitle;
    while (this.#cards.length > data.items.length) {
      this.#cards.pop()?.root.remove();
    }
    while (this.#cards.length < data.items.length) {
      const card = new DfkNextStepCard();
      this.#cards.push(card);
      this.#grid.appendChild(card.root);
    }
    data.items.forEach((item, i) => this.#cards[i].apply(item));
  }
}

/** One "next step" card: the whole card is the link. Built once. */
class DfkNextStepCard {
  readonly root: HTMLAnchorElement;
  readonly #title: HTMLElement;
  readonly #details: HTMLElement;
  readonly #arrow: IconifyIconHTMLElement;

  constructor() {
    this.root = document.createElement('a');
    this.root.className = 'dfk-next-card';

    this.#title = document.createElement('span');
    this.#title.className = 'dfk-next-card-title';
    this.#details = document.createElement('span');
    this.#details.className = 'dfk-next-card-details';
    const body = document.createElement('span');
    body.className = 'dfk-next-card-body';
    body.append(this.#title, this.#details);

    this.#arrow = document.createElement('iconify-icon');
    this.#arrow.className = 'dfk-next-card-arrow';
    this.#arrow.setAttribute('icon', 'lucide:arrow-right');
    this.#arrow.setAttribute('aria-hidden', 'true');

    this.root.append(body, this.#arrow);
  }

  apply(item: NextStepItem): void {
    this.root.setAttribute('href', item.href);
    this.#title.textContent = item.title;
    this.#details.textContent = item.details;
  }
}
