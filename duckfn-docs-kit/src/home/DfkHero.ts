import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import {childTags, jsonAttr, textOf} from './attrs';
import {resolveHref} from './base';
import {homeStyles} from './styles';
import type {HeroAction, HeroBadge, HeroLink} from '../types';

const TAG = 'dfk-hero';

/**
 * `<dfk-hero>` — the landing hero: logo, title, tagline, the two call-to-action
 * buttons and the badge row. Ported from the Docusaurus home page's `Hero()`.
 *
 * Retained-mode: every node is held in a field, the structure is assembled once
 * in the constructor and the `set*` methods only mutate the nodes they own.
 *
 * Content arrives declaratively (see `attrs.ts` for the two spellings):
 * `logo-src` / `title` / `tagline` as text attributes, `primary` / `secondary`
 * / `badges` as JSON, and `<dfk-action>` / `<dfk-badge>` child tags read once on
 * connect. Whichever spelling is used, it ends in the setters below — no
 * generic `apply()` / `render()` entry point.
 *
 * The tree lives in a shadow root (adopting the shared `homeStyles()` sheet), so
 * the site's global CSS cannot reach it; theme colours cross the boundary
 * through the inherited `--duckfn-*` / `--ifm-*` custom properties.
 */
export class DfkHero extends HTMLElementBase {
  static get observedAttributes(): string[] {
    return ['logo-src', 'title', 'tagline', 'primary', 'secondary', 'badges'];
  }

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

  constructor() {
    super();
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
    // A custom element's constructor may call `attachShadow` (it only must not
    // add children or attributes to `this`), so the whole tree lives in the
    // shadow root from the first moment the element exists — the setters are
    // safe to call before the element is even connected.
    const shadow = this.attachShadow({mode: 'open'});
    shadow.adoptedStyleSheets = [homeStyles()];
    shadow.appendChild(this.#section);
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
      case 'logo-src':
        this.setLogo(value);
        break;
      case 'title':
        this.setTitle(value);
        // `title` is also a global HTML attribute: the browser would show the
        // same copy as a tooltip over the whole hero. The <h1> already says it,
        // so the attribute is spent and removed. React records it as applied,
        // so this cannot ping-pong.
        this.removeAttribute('title');
        break;
      case 'tagline':
        this.setTagline(value);
        break;
      case 'primary': {
        const link = jsonAttr<HeroLink>(TAG, name, value);
        if (link) {
          this.setPrimaryAction(link);
        }
        break;
      }
      case 'secondary': {
        const action = jsonAttr<HeroAction>(TAG, name, value);
        if (action) {
          this.setSecondaryAction(action);
        }
        break;
      }
      case 'badges': {
        const badges = jsonAttr<readonly HeroBadge[]>(TAG, name, value);
        if (badges) {
          this.setBadges(badges);
        }
        break;
      }
    }
  }

  connectedCallback(): void {
    // Read one task later: an element can be connected before its child tags are
    // parsed (the parser hits the start tag first) or before a framework
    // appends them (React connects the parent first). Nothing paints in
    // between, so the shadow tree is in place for the first frame anyway.
    queueMicrotask(() => {
      if (this.isConnected) {
        this.#seedFromChildren();
      }
    });
  }

  setLogo(src: string): void {
    this.#logo.src = resolveHref(src);
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

  /** The GitHub button: always external, optionally carrying a glyph. */
  setSecondaryAction(action: HeroAction): void {
    this.#fillLink(this.#secondary, this.#secondaryLabel, action, true);
    if (action.icon) {
      this.#secondaryIcon.setAttribute('icon', action.icon);
    } else {
      this.#secondaryIcon.removeAttribute('icon');
    }
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

  /** Child tags are the readable spelling of `primary` / `secondary` / `badges`. */
  #seedFromChildren(): void {
    for (const action of childTags(this, 'dfk-action')) {
      const link: HeroLink = {
        label: textOf(action),
        href: action.getAttribute('href') ?? '#',
      };
      const icon = action.getAttribute('icon');
      if (action.getAttribute('kind') === 'secondary') {
        this.setSecondaryAction(icon ? {...link, icon} : link);
      } else {
        this.setPrimaryAction(link);
      }
    }
    const badges = childTags(this, 'dfk-badge');
    if (badges.length > 0) {
      this.setBadges(
        badges.map((badge) => ({
          href: badge.getAttribute('href') ?? '#',
          src: badge.getAttribute('src') ?? '',
          alt: badge.getAttribute('alt') ?? '',
        })),
      );
    }
  }

  #fillLink(
    anchor: HTMLAnchorElement,
    label: HTMLElement,
    link: HeroLink,
    external: boolean,
  ): void {
    anchor.href = resolveHref(link.href);
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
