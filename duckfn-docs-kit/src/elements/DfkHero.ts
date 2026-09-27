import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import type {HeroAction, HeroBadge, HeroLink} from '../types';

/**
 * `<dfk-hero>` — the landing hero: logo, title, tagline, the two call-to-action
 * buttons and the badge row. Ported from the Docusaurus home page's `Hero()`.
 *
 * Retained-mode: every node is held in a field, the structure is assembled once
 * in the constructor (into a detached `section`, because a custom element's
 * constructor must not add children to the element it is building), and the
 * `set*` methods only mutate the nodes they own.
 */
export class DfkHero extends HTMLElementBase {
  readonly #section = el('section', {class: 'dfk-hero'});
  readonly #inner = el('div', {class: 'dfk-hero-inner'});
  readonly #stage = el('span', {class: 'dfk-logo-stage'});
  // The <h1> spells out the name, so the logo is decorative: alt="".
  readonly #logo = el('img', {
    class: 'dfk-logo',
    alt: '',
    width: 480,
    height: 480,
  });
  readonly #title = el('h1', {class: 'dfk-title'});
  readonly #tagline = el('p', {class: 'dfk-tagline'});
  readonly #actions = el('div', {class: 'dfk-actions'});
  readonly #primary = el('a', {class: 'dfk-button-primary'});
  readonly #primaryLabel = el('span');
  readonly #secondary = el('a', {class: 'dfk-button-secondary'});
  readonly #secondaryIcon: IconifyIconHTMLElement = el('iconify-icon', {
    class: 'dfk-button-icon',
    attrs: {'aria-hidden': 'true'},
  });
  readonly #secondaryLabel = el('span');
  readonly #badges = el('div', {class: 'dfk-badges'});
  readonly #badgeLinks: HTMLAnchorElement[] = [];
  #attached = false;

  constructor() {
    super();
    // Assembled into the detached `section`, never into `this`.
    this.#stage.appendChild(this.#logo);
    this.#primary.appendChild(this.#primaryLabel);
    this.#secondary.append(this.#secondaryIcon, this.#secondaryLabel);
    this.#actions.append(this.#primary, this.#secondary);
    this.#inner.append(
      this.#stage,
      this.#title,
      this.#tagline,
      this.#actions,
      this.#badges,
    );
    this.#section.appendChild(this.#inner);
  }

  connectedCallback(): void {
    // React can detach and re-attach the same node; the structure is built once.
    if (this.#attached) {
      return;
    }
    this.#attached = true;
    this.append(this.#section);
  }

  setLogo(src: string): void {
    this.#logo.src = src;
  }

  setTitle(text: string): void {
    this.#title.textContent = text;
  }

  setTagline(text: string): void {
    this.#tagline.textContent = text;
  }

  /** The "Get started" button: an internal link, so it stays in the same tab. */
  setPrimaryAction(link: HeroLink): void {
    this.#fillLink(this.#primary, this.#primaryLabel, link, false);
  }

  /** The GitHub button: always external, always carries the glyph. */
  setSecondaryAction(action: HeroAction): void {
    this.#fillLink(this.#secondary, this.#secondaryLabel, action, true);
    this.#secondaryIcon.setAttribute('icon', action.icon);
  }

  setBadges(badges: readonly HeroBadge[]): void {
    // Grow/shrink the row to the new length and reuse the links already there.
    while (this.#badgeLinks.length > badges.length) {
      this.#badgeLinks.pop()?.remove();
    }
    while (this.#badgeLinks.length < badges.length) {
      const link = el('a', {
        class: 'dfk-badge',
        target: '_blank',
        rel: 'noopener noreferrer',
      });
      link.appendChild(el('img', {class: 'dfk-badge-image'}));
      this.#badgeLinks.push(link);
      this.#badges.appendChild(link);
    }
    badges.forEach((badge, index) => {
      const link = this.#badgeLinks[index];
      link.href = badge.href;
      const image = link.firstElementChild as HTMLImageElement;
      image.src = badge.src;
      image.alt = badge.alt;
    });
  }

  #fillLink(
    anchor: HTMLAnchorElement,
    label: HTMLElement,
    link: HeroLink,
    external: boolean,
  ): void {
    anchor.href = link.href;
    label.textContent = link.label;
    if (external) {
      anchor.target = '_blank';
      anchor.rel = 'noopener noreferrer';
    } else {
      anchor.removeAttribute('target');
      anchor.removeAttribute('rel');
    }
  }
}