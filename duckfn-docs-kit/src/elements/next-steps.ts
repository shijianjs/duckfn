import {applyIconMask, iconArrowRight} from '../icons';
import type {NextStepItem, NextStepsData} from '../types';
import {DfkElement} from './base';

/**
 * `<dfk-next-steps>` — the "Where to go next" row of link cards. Ported from
 * the home page's `NextSteps()`; each card is an anchor holding a title, a
 * description and a trailing arrow.
 */
export class DfkNextSteps extends DfkElement<NextStepsData> {
  protected override build(data: NextStepsData): void {
    const section = this.el('section', {class: 'dfk-section'});
    const inner = this.el('div', {class: 'dfk-section-inner'});
    const heading = this.el('h2', {class: 'dfk-section-title', text: data.sectionTitle});

    const grid = this.el('div', {class: 'dfk-next-grid'});
    for (const item of data.items) {
      grid.appendChild(new DfkNextStepCard(item).render());
    }

    inner.append(heading, grid);
    section.appendChild(inner);
    this.appendChild(section);
  }
}

/** One "next step" card: the whole card is the link. */
class DfkNextStepCard {
  readonly #item: NextStepItem;
  readonly #root: HTMLAnchorElement;
  readonly #body: HTMLElement;
  readonly #title: HTMLElement;
  readonly #details: HTMLElement;
  readonly #arrow: HTMLElement;

  constructor(item: NextStepItem) {
    this.#item = item;
    this.#root = document.createElement('a');
    this.#root.className = 'dfk-next-card';
    this.#root.setAttribute('href', item.href);

    this.#body = document.createElement('span');
    this.#body.className = 'dfk-next-card-body';
    this.#title = document.createElement('span');
    this.#title.className = 'dfk-next-card-title';
    this.#details = document.createElement('span');
    this.#details.className = 'dfk-next-card-details';

    this.#arrow = document.createElement('span');
    this.#arrow.className = 'dfk-icon dfk-next-card-arrow';
    applyIconMask(this.#arrow, iconArrowRight);
  }

  render(): HTMLAnchorElement {
    this.#title.textContent = this.#item.title;
    this.#details.textContent = this.#item.details;
    this.#body.replaceChildren(this.#title, this.#details);
    this.#root.replaceChildren(this.#body, this.#arrow);
    return this.#root;
  }
}
