import type {IconifyIconHTMLElement} from 'iconify-icon';
import type {HeroData, HeroLink} from '../types';
import {DfkElement} from './base';

/**
 * `<dfk-hero>` — the landing hero: logo, title, tagline, the two call-to-action
 * buttons and the badge row. Ported from the Docusaurus home page's `Hero()`.
 *
 * The skeleton is built once in `create()`; `apply()` writes the data into the
 * held nodes. The badge row is the only variable-length list: its items are
 * appended/removed to match the data instead of the row being rebuilt.
 */
export class DfkHero extends DfkElement<HeroData> {
  readonly #logo = this.el('img', {
    class: 'dfk-logo',
    attrs: {alt: '', width: '480', height: '480'},
  });
  readonly #title = this.el('h1', {class: 'dfk-title'});
  readonly #tagline = this.el('p', {class: 'dfk-tagline'});
  readonly #primary: HTMLAnchorElement;
  readonly #primaryLabel = this.el('span');
  readonly #secondary: HTMLAnchorElement;
  readonly #secondaryLabel = this.el('span');
  readonly #secondaryIcon: IconifyIconHTMLElement;
  readonly #badges = this.el('div', {class: 'dfk-badges'});
  #badgeLinks: HTMLAnchorElement[] = [];

  constructor() {
    super();
    this.#primary = this.#buildLink('dfk-button-primary', this.#primaryLabel);
    this.#secondary = this.#buildLink('dfk-button-secondary', this.#secondaryLabel);
    // The <h1> spells out the name, so the logo is decorative: alt="".
    this.#secondaryIcon = this.el('iconify-icon', {
      class: 'dfk-button-icon',
      attrs: {'aria-hidden': 'true'},
    });
    this.#secondary.replaceChildren(
      this.#secondaryIcon,
      this.#secondaryLabel,
    );
  }

  protected override create(): void {
    const stage = this.el('span', {class: 'dfk-logo-stage'});
    stage.appendChild(this.#logo);

    const actions = this.el('div', {class: 'dfk-actions'});
    actions.append(this.#primary, this.#secondary);

    const inner = this.el('div', {class: 'dfk-hero-inner'});
    inner.append(stage, this.#title, this.#tagline, actions, this.#badges);

    const section = this.el('section', {class: 'dfk-hero'});
    section.appendChild(inner);
    this.appendChild(section);
  }

  protected override apply(data: HeroData): void {
    this.#logo.setAttribute('src', data.logoSrc);
    this.#title.textContent = data.title;
    this.#tagline.textContent = data.tagline;
    this.#applyLink(this.#primary, this.#primaryLabel, data.primary);
    this.#applyLink(this.#secondary, this.#secondaryLabel, {
      ...data.secondary,
      external: true,
    });
    this.#secondaryIcon.setAttribute('icon', data.secondary.icon);
    this.#applyBadges(data);
  }

  #buildLink(className: string, label: HTMLElement): HTMLAnchorElement {
    const anchor = this.el('a', {class: className});
    anchor.appendChild(label);
    return anchor;
  }

  #applyLink(
    anchor: HTMLAnchorElement,
    label: HTMLElement,
    link: HeroLink,
  ): void {
    anchor.setAttribute('href', link.href);
    label.textContent = link.label;
    if (link.external) {
      anchor.setAttribute('target', '_blank');
      anchor.setAttribute('rel', 'noopener noreferrer');
    } else {
      anchor.removeAttribute('target');
      anchor.removeAttribute('rel');
    }
  }

  #applyBadges(data: HeroData): void {
    // Grow/shrink the row to the data length; existing links are reused.
    while (this.#badgeLinks.length > data.badges.length) {
      this.#badgeLinks.pop()?.remove();
    }
    while (this.#badgeLinks.length < data.badges.length) {
      const link = this.el('a', {
        class: 'dfk-badge',
        attrs: {target: '_blank', rel: 'noopener noreferrer'},
      });
      link.appendChild(this.el('img', {class: 'dfk-badge-image'}));
      this.#badgeLinks.push(link);
      this.#badges.appendChild(link);
    }
    data.badges.forEach((badge, i) => {
      const link = this.#badgeLinks[i];
      link.setAttribute('href', badge.href);
      const image = link.firstElementChild as HTMLImageElement;
      image.setAttribute('src', badge.src);
      image.alt = badge.alt;
    });
  }
}
