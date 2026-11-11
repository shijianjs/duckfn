import {el} from '../dom';

/**
 * The band a card section sits in: a centred content column and the title above
 * the grid.
 *
 * `<dfk-features>` and `<dfk-next-steps>` are the same band with different cards
 * inside, and the band is one look that has to stay identical in both — that is
 * real sharing, so it is built once here. The cards belong to the elements that
 * render them.
 *
 * The static layout is set inline; the paddings live in `section.css` because the
 * breakpoints override them and a media query cannot beat an inline declaration.
 */
export class DfkSection {
  /** The `<h2>` the element fills with its `section-title`. */
  readonly heading = el('h2', {class: 'dfk-section-title'}, (heading) => {
    // Only the bottom margin: the UA's top margin is what separates the band's
    // top edge from the title.
    heading.style.marginBottom = '0';
    heading.style.textAlign = 'center';
    heading.style.fontSize = 'clamp(1.5rem, 3vw, 2rem)';
    heading.style.letterSpacing = '-0.01em';
  });

  /** Where the element appends its grid. */
  readonly content = el('div', {class: 'dfk-section-inner'}, (inner) => {
    inner.style.maxWidth = '72rem';
    inner.style.margin = '0 auto';
  });

  readonly root = el('section', {class: 'dfk-section'});

  constructor() {
    this.content.appendChild(this.heading);
    this.root.appendChild(this.content);
  }
}
