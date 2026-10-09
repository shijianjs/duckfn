import Anser from 'anser';

/**
 * ANSI-coloured text → styled runs, with nothing escape-shaped left in the text.
 *
 * The parsing is [`anser`](https://www.npmjs.com/package/anser)'s: it is the
 * library every Node CLI ecosystem already uses for this (12M+ weekly
 * downloads, zero dependencies, TypeScript types included), it covers what a
 * terminal frame actually contains — SGR colours, bright, 256, truecolor,
 * decorations, reverse video — and it hands back **structured runs**
 * (`ansiToJson`) rather than an HTML string. That last part is why it is the
 * right one here: the kit builds DOM from nodes and never from markup
 * (CONVENTIONS rule 1), so a converter whose only output is `<span>` soup would
 * have to be pasted in through `innerHTML`.
 *
 * What this module adds on top of the library is the **sanitising**, which the
 * library does not do:
 *
 * - Terminal *string* sequences (OSC window titles, DCS/APC/PM payloads) go
 *   before parsing. `anser` only understands SGR, so an OSC title would
 *   otherwise survive into the output as literal `]0;title` text.
 * - Any CSI sequence left over afterwards goes too — the ones `anser` does not
 *   consume (a malformed SGR, a private parameter) would otherwise reach the DOM
 *   as `[38;2;1;2;3m` text.
 * - Every remaining control character is dropped. By that point the sequences
 *   have been consumed, so any ESC left is debris — and text that reaches the DOM
 *   must be printable (the same reason the `svg` renderer strips `script` / `on*`
 *   before insertion).
 *
 * `\n` and `\t` are the two control characters a frame legitimately renders
 * (CSS gives the tab its stop with `tab-size`).
 *
 * This module is free of DOM access on purpose: `terminal.ts` imports it for the
 * `terminal` renderer, and `test/ansi.test.mjs` imports the **built** copy in Node
 * to pin the behaviour there — see the `sql/ansi` entry in `vite.config.ts`.
 */

/** One styled run: its text plus the SGR state that was in effect for it. */
export interface AnsiRun {
  /** Printable text only: no escape sequences, no control characters. */
  text: string;
  /** `r, g, b` (a bare `rgb()` argument), or `null` for the panel's own colour. */
  fg: string | null;
  bg: string | null;
  /**
   * SGR attributes in effect: `bold`, `dim`, `italic`, `underline`, `blink`,
   * `strikethrough`, `hidden`. They **accumulate**, the way a terminal's do — a
   * run that turns on italics while bold is still on lists both. `reverse` never
   * appears: `anser` has already resolved it by swapping `fg` and `bg`.
   */
  decorations: readonly string[];
}

/**
 * Terminal *string* sequences: OSC (window title, hyperlinks), DCS, APC, PM and
 * the single-character `ESC X` forms. They carry an arbitrary payload up to BEL
 * or ST, so they are removed **before** parsing — once the ESC is gone there is
 * no way to tell payload from content. `anser` understands none of them.
 */
const STRING_SEQUENCE = /\u001b[\]P^_X][^\u0007\u001b]*(?:\u0007|\u001b\\|$)/g;

/**
 * Any CSI sequence: `ESC [` , parameter bytes, intermediate bytes, one final
 * byte. That is SGR (colour and attributes), cursor movement, erase — whatever
 * `anser` did not already consume.
 */
const CSI_SEQUENCE = /\u001b\[[0-9;:<=>?]*[ -\/]*[@-~]/g;

/**
 * The control characters that must never reach the DOM: everything below
 * U+0020 except the two a frame renders, DEL, and the C1 range. Note that ESC
 * (U+001B) is in here — this pass runs after the sequences have been consumed,
 * so a surviving ESC is debris rather than styling.
 */
const CONTROL_CHARACTERS = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/g;

/** The slice of `anser`'s JSON entry this module reads. */
interface AnsiEntry {
  content: string;
  /** Declared non-nullable upstream, but `null` whenever no colour is set. */
  fg: string | null;
  bg: string | null;
  decorations: readonly string[];
}

/**
 * Splits ANSI text into styled runs.
 *
 * Runs that end up with identical styling are merged: a frame that re-asserts a
 * colour it already has (a reset plus the same colour again, which is what a
 * per-cell colour emitter does) would otherwise produce two adjacent spans for
 * one visual run — and a plot drawn as a grid produces a lot of them.
 */
export function ansiRuns(text: string): AnsiRun[] {
  const entries: AnsiEntry[] = Anser.ansiToJson(text.replace(STRING_SEQUENCE, ''));
  const runs: AnsiRun[] = [];
  for (const entry of entries) {
    const content = printable(entry.content);
    if (content === '') {
      // `anser` emits an empty run at every style change, including the ones a
      // frame writes around a reset. Nothing to draw.
      continue;
    }
    const decorations = entry.decorations ?? [];
    const previous = runs.at(-1);
    if (
      previous &&
      previous.fg === entry.fg &&
      previous.bg === entry.bg &&
      sameDecorations(previous.decorations, decorations)
    ) {
      previous.text += content;
      continue;
    }
    runs.push({text: content, fg: entry.fg ?? null, bg: entry.bg ?? null, decorations});
  }
  return runs;
}

/**
 * The frame with every escape sequence and control character stripped, i.e. what
 * it looks like with the colour lost. What the view falls back to when the
 * parser's chunk never arrives (offline, blocked CDN), and what a reader wants
 * when they ask for the text of a frame.
 */
export function plainAnsiText(text: string): string {
  return printable(text.replace(STRING_SEQUENCE, ''));
}

/** The text a run may put in the DOM: printable, and free of any sequence. */
function printable(text: string): string {
  return text.replace(CSI_SEQUENCE, '').replace(CONTROL_CHARACTERS, '');
}

function sameDecorations(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((name, index) => name === b[index]);
}