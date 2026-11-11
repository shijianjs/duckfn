import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el, HTMLElementBase} from '../dom';
import {childTags, jsonAttr, textOf} from './attrs';
import {resolveHref} from './base';
import {baseStyles, heroStyles} from './styles';
import type {HeroAction, HeroBadge, HeroLink} from '../types';

const TAG = 'dfk-hero';

/**
 * `<dfk-hero>` — the landing hero: logo, title, tagline, the two call-to-action
 * buttons and the badge row. Ported from the Docusaurus home page's `Hero()`.
 *
 * Retained-mode: every node is held in a field, the structure is assembled once
 * in the constructor and the `set*` methods only mutate the nodes they own.
 *
 * Styles are written where the nodes are: each box's layout, spacing and type is
 * set inline here, and only what inline cannot say (`::before` glows, the
 * `@supports` gradient on the title, the button states, the entrance motion)
 * lives in `hero.css`, adopted as one sheet by every instance.
 *
 * Content arrives declaratively (see `attrs.ts` for the two spellings):
 * `logo-src` / `title` / `tagline` as text attributes, `primary` / `secondary`
 * / `badges` as JSON, and `<dfk-action>` / `<dfk-badge>` child tags read once on
 * connect. Whichever spelling is used, it ends in the setters below — no
 * generic `apply()` / `render()` entry point.
 *
 * The tree lives in a shadow root, so the site's global CSS cannot reach it;
 * theme colours cross the boundary through the inherited `--duckfn-*` /
 * `--ifm-*` custom properties.
 */
export class DfkHero extends HTMLElementBase {
  static get observedAttributes(): string[] {
    return ['logo-src', 'title', 'tagline', 'primary', 'secondary', 'badges'];
  }

  readonly #section = el('section', {class: 'dfk-hero'}, (section) => {
    // Contains the glow pseudo-elements without a z-index race. `clip`, not
    // `hidden`: both keep the page narrow, but `hidden` would also clip the warm
    // glow along this section's bottom edge instead of letting it fade out over
    // the next section. The padding is in `hero.css` (a breakpoint re-sets it).
    section.style.position = 'relative';
    section.style.isolation = 'isolate';
    section.style.overflowX = 'clip';
    section.style.textAlign = 'center';
  });
  readonly #inner = el('div', {class: 'dfk-hero-inner'}, (inner) => {
    inner.style.maxWidth = '46rem';
    inner.style.margin = '0 auto';
  });
  readonly #stage = el('span', {class: 'dfk-logo-stage'}, (stage) => {
    stage.style.display = 'inline-block';
    stage.style.lineHeight = '0';
  });
  // The <h1> spells out the name, so the logo is decorative: alt="".
  readonly #logo = el(
    'img',
    {class: 'dfk-logo', alt: '', width: 480, height: 480},
    (logo) => {
      // A vector, so this stays sharp at any density.
      logo.style.display = 'block';
      logo.style.width = 'clamp(7rem, 16vw, 10rem)';
      logo.style.height = 'auto';
      logo.style.filter = 'drop-shadow(0 14px 28px var(--duckfn-glow))';
    },
  );
  readonly #title = el('h1', {class: 'dfk-title'}, (title) => {
    title.style.margin = '1.5rem 0 0';
    title.style.fontSize = 'clamp(2.25rem, 5vw, 3.25rem)';
    title.style.fontWeight = '700';
    title.style.letterSpacing = '-0.02em';
    title.style.lineHeight = '1.1';
  });
  readonly #tagline = el('p', {class: 'dfk-tagline'}, (tagline) => {
    tagline.style.maxWidth = '34rem';
    tagline.style.margin = '1.25rem auto 0';
    tagline.style.fontSize = 'clamp(1.05rem, 2.2vw, 1.25rem)';
    tagline.style.lineHeight = '1.65';
    tagline.style.color = 'var(--ifm-color-emphasis-700)';
  });
  readonly #actions = el('div', {class: 'dfk-actions'}, (actions) => {
    // `flex-direction` is not set: the phone breakpoint in `hero.css` sets it, and
    // an inline value would win over that rule.
    actions.style.display = 'flex';
    actions.style.flexWrap = 'wrap';
    actions.style.justifyContent = 'center';
    actions.style.gap = '0.75rem 1rem';
    actions.style.marginTop = '2.25rem';
  });
  readonly #primary = el('a', {class: 'dfk-button-primary'}, styleButton);
  readonly #primaryLabel = el('span');
  readonly #secondary = el('a', {class: 'dfk-button-secondary'}, styleButton);
  readonly #secondaryIcon: IconifyIconHTMLElement = el(
    'iconify-icon',
    {class: 'dfk-button-icon', attrs: {'aria-hidden': 'true'}},
    (icon) => {
      // Size with `font-size`, not `width`/`height`: the component renders an
      // inner `<svg width="1em" height="1em">`, so the glyph follows the host's
      // font-size and a CSS width would only grow the empty host box.
      icon.style.fontSize = '1.125rem';
    },
  );
  readonly #secondaryLabel = el('span');
  readonly #badges = el('div', {class: 'dfk-badges'}, (badges) => {
    badges.style.display = 'flex';
    badges.style.flexWrap = 'wrap';
    badges.style.alignItems = 'center';
    badges.style.justifyContent = 'center';
    badges.style.gap = '0.5rem';
    badges.style.marginTop = '2rem';
  });
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
    shadow.adoptedStyleSheets = [baseStyles(), heroStyles()];
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
      const link = el(
        'a',
        {class: 'dfk-badge', target: '_blank', rel: 'noopener noreferrer'},
        (anchor) => {
          anchor.style.display = 'inline-flex';
        },
      );
      link.appendChild(
        el('img', {class: 'dfk-badge-image'}, (image) => {
          image.style.display = 'block';
          image.style.width = 'auto';
          image.style.height = '20px';
        }),
      );
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

/**
 * The pill's box, shared by both buttons: same size on purpose, only their
 * colours differ (and those are in `hero.css`, because `:hover` changes them).
 */
function styleButton(anchor: HTMLAnchorElement): void {
  anchor.style.display = 'inline-flex';
  anchor.style.alignItems = 'center';
  anchor.style.justifyContent = 'center';
  anchor.style.gap = '0.5rem';
  anchor.style.padding = '0.7rem 1.6rem';
  anchor.style.borderRadius = '999px';
  anchor.style.fontSize = '1rem';
  anchor.style.fontWeight = '600';
  anchor.style.lineHeight = '1.4';
  anchor.style.textDecoration = 'none';
}
