import {applyIconMask} from '../icons';
import type {HeroData} from '../types';
import {DfkElement} from './base';

/**
 * `<dfk-hero>` — the landing hero: logo, title, tagline, the two call-to-action
 * buttons and the badge row. Ported from the Docusaurus home page's `Hero()`.
 */
export class DfkHero extends DfkElement<HeroData> {
  protected override build(data: HeroData): void {
    const section = this.el('section', {class: 'dfk-hero'});
    const inner = this.el('div', {class: 'dfk-hero-inner'});

    // The <h1> spells out the name, so the logo is decorative: alt="".
    const stage = this.el('span', {class: 'dfk-logo-stage'});
    const logo = this.el('img', {
      class: 'dfk-logo',
      attrs: {src: data.logoSrc, alt: '', width: '480', height: '480'},
    });
    stage.appendChild(logo);

    const title = this.el('h1', {class: 'dfk-title', text: data.title});
    const tagline = this.el('p', {class: 'dfk-tagline', text: data.tagline});

    const actions = this.el('div', {class: 'dfk-actions'});
    actions.append(
      this.#buildLink(data.primary, 'dfk-button-primary'),
      this.#buildAction(data),
    );

    const badges = this.el('div', {class: 'dfk-badges'});
    for (const badge of data.badges) {
      const link = this.el('a', {
        class: 'dfk-badge',
        attrs: {href: badge.href, target: '_blank', rel: 'noopener noreferrer'},
      });
      link.appendChild(
        this.el('img', {class: 'dfk-badge-image', attrs: {src: badge.src, alt: badge.alt}}),
      );
      badges.appendChild(link);
    }

    inner.append(stage, title, tagline, actions, badges);
    section.appendChild(inner);
    this.appendChild(section);
  }

  #buildLink(
    link: {label: string; href: string; external?: boolean},
    className: string,
  ): HTMLAnchorElement {
    const anchor = this.el('a', {class: className, text: link.label, attrs: {href: link.href}});
    if (link.external) {
      anchor.setAttribute('target', '_blank');
      anchor.setAttribute('rel', 'noopener noreferrer');
    }
    return anchor;
  }

  #buildAction(data: HeroData): HTMLAnchorElement {
    const anchor = this.#buildLink({...data.secondary, external: true}, 'dfk-button-secondary');
    const icon = this.el('span', {class: 'dfk-icon dfk-button-icon'});
    applyIconMask(icon, data.secondary.icon);
    // The label becomes a <span> so the icon and text can be laid out with the
    // button's flex gap.
    anchor.replaceChildren(icon, this.el('span', {text: data.secondary.label}));
    return anchor;
  }
}
