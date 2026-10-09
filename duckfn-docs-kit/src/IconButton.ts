import type {IconifyIconHTMLElement} from 'iconify-icon';
import {el} from './dom';

/**
 * A compact icon-only button with a hover/focus tooltip, built once and then
 * mutated. Shared by `<dfk-sql>`'s code-block cluster and `<dfk-mermaid>`'s
 * diagram cluster: both want the same "floating icon row over content" idiom,
 * and neither wants a second implementation of it.
 *
 * The tooltip is also the accessible name — an icon-only control has no text to
 * fall back on. The two class names are written by this module and styled in
 * *each* consumer's shadow-root CSS; that duplication is unavoidable (a shadow
 * boundary stops one sheet from reaching the other tree), so the rules carry the
 * same names and are kept in sync by hand.
 */
export class IconButton {
  readonly root = el('button', {class: 'dfk-icon-button', type: 'button'});
  readonly #icon: IconifyIconHTMLElement = el('iconify-icon', {
    class: 'dfk-icon',
    attrs: {'aria-hidden': 'true'},
  });

  constructor(icon: string, onClick: () => void) {
    this.root.appendChild(this.#icon);
    this.root.addEventListener('click', onClick);
    this.setIcon(icon);
  }

  setIcon(icon: string): void {
    this.#icon.setAttribute('icon', icon);
  }

  setLabel(text: string): void {
    this.root.setAttribute('data-tip', text);
    this.root.setAttribute('aria-label', text);
  }

  /** Marks a toggle as currently on (e.g. the SQL block's wrap toggle). */
  setOn(on: boolean): void {
    this.root.classList.toggle('dfk-icon-on', on);
  }

  /**
   * Hides the button without removing it (`[hidden]`, which every consumer's
   * stylesheet turns back into `display: none` — a shadow host cannot rely on the
   * UA rule alone). For controls that have nothing to do in the current state,
   * such as reset-zoom while zooming is off.
   */
  setHidden(hidden: boolean): void {
    this.root.hidden = hidden;
  }

  setDisabled(disabled: boolean): void {
    if (disabled) {
      this.root.setAttribute('disabled', '');
    } else {
      this.root.removeAttribute('disabled');
    }
  }
}