/**
 * Tests for the ANSI parser behind the `terminal` result renderer.
 *
 * What is worth pinning here is not the colour table (that is `anser`'s, and it
 * has its own tests) but the two things this module is responsible for:
 *
 * 1. **Nothing escape-shaped reaches the DOM.** A terminal frame can carry OSC
 *    window titles, DCS payloads, cursor moves and stray control characters —
 *    including in text a reader controls (axis labels, category names). Those are
 *    either invisible or actively misleading (`]0;title` printed as literal
 *    text), so every run this module returns is printable-only. The invariant is
 *    asserted over a sample that contains all of them.
 * 2. **The colour plumbing reaches the runs in the shape `TerminalView` reads.**
 *    A frame drawn with truecolor (`ESC[38;2;r;g;bm`, what kuva's terminal
 *    backend emits for every cell) has to arrive as `fg: 'r, g, b'`, and reverse
 *    video has to arrive already swapped — the view applies nothing itself.
 *
 * Runs against the built `dist/` (`npm run build` first; `just release_kit_check`
 * does that, then `npm test`).
 */
import assert from 'node:assert/strict';
import {test} from 'node:test';

import {ansiRuns, plainAnsiText} from '../dist/sql/ansi.js';

/** ESC, spelled without a literal control character in the source. */
const ESC = String.fromCharCode(27);
const BEL = String.fromCharCode(7);
/** A sweep of the characters the sanitiser has to drop, NUL and DEL included. */
const CONTROLS = [0, 1, 8, 11, 12, 14, 31, 127, 155].map((code) => String.fromCharCode(code)).join('');

/** Every character a run must never contain, as a test-level predicate. */
function isPrintable(text) {
  return ![...text].some((character) => {
    const code = character.codePointAt(0);
    return (code < 0x20 && character !== '\n' && character !== '\t') || code === 0x7f || (code >= 0x80 && code <= 0x9f);
  });
}

const texts = (runs) => runs.map((run) => run.text).join('');
const colours = (runs) => runs.filter((run) => run.fg || run.bg);

test('keeps plain text as one unstyled run', () => {
  const runs = ansiRuns('no escapes at all');
  assert.deepEqual(runs, [{text: 'no escapes at all', fg: null, bg: null, decorations: []}]);
});

test('reads a truecolor foreground as an rgb() argument', () => {
  // The shape kuva's terminal backend emits: one 24-bit colour per run.
  const runs = ansiRuns(`${ESC}[38;2;20;69;155m███${ESC}[0m`);
  assert.equal(runs.length, 1);
  assert.equal(runs[0].text, '███');
  assert.equal(runs[0].fg, '20, 69, 155');
  assert.equal(runs[0].bg, null);
});

test('reads the basic, bright and background colours', () => {
  const runs = ansiRuns(`${ESC}[31mred ${ESC}[92mbright ${ESC}[44;97mon blue${ESC}[0m`);
  assert.deepEqual(
    runs.map((run) => [run.text, run.fg, run.bg]),
    [
      ['red ', '187, 0, 0', null],
      ['bright ', '0, 255, 0', null],
      ['on blue', '255, 255, 255', '0, 0, 187'],
    ],
  );
});

test('resolves a 256-colour palette index', () => {
  const runs = ansiRuns(`${ESC}[38;5;196mred${ESC}[48;5;226m on yellow${ESC}[0m`);
  assert.equal(runs[0].fg, '255, 0, 0');
  assert.equal(runs[1].bg, '255, 255, 0');
});

test('reports the decorations in effect, which accumulate like a terminal does', () => {
  const runs = ansiRuns(`${ESC}[1mb${ESC}[3mi${ESC}[4mu${ESC}[9ms${ESC}[8mh${ESC}[0m`);
  assert.deepEqual(
    runs.map((run) => run.decorations.join()),
    [
      'bold',
      'bold,italic',
      'bold,italic,underline',
      'bold,italic,underline,strikethrough',
      'bold,italic,underline,strikethrough,hidden',
    ],
  );
  // A reset drops them again.
  assert.deepEqual(ansiRuns(`${ESC}[1mb${ESC}[0ma`).map((run) => run.decorations.length), [1, 0]);
});

test('reverse video arrives already swapped, with no decoration to apply', () => {
  const runs = ansiRuns(`${ESC}[7mrev${ESC}[0m${ESC}[31;42mrev2${ESC}[0m`);
  // `anser` resolves `reverse` into the colours themselves; the view must not
  // swap them a second time.
  assert.equal(runs[0].fg, '0, 0, 0');
  assert.equal(runs[0].bg, '255,255,255');
  assert.ok(!runs[0].decorations.includes('reverse'));
  assert.equal(runs[1].fg, '187, 0, 0');
  assert.equal(runs[1].bg, '0, 187, 0');
});

test('drops terminal string sequences (OSC, DCS) instead of printing them', () => {
  const runs = ansiRuns(`${ESC}]0;window title${BEL}visible${ESC}Ppayload${ESC}\\after`);
  assert.equal(texts(runs), 'visibleafter');
});

test('drops cursor movement and erase sequences', () => {
  const runs = ansiRuns(`${ESC}[2J${ESC}[H${ESC}[3A${ESC}[Kframe`);
  assert.equal(texts(runs), 'frame');
});

test('keeps newlines and tabs, which a frame renders', () => {
  const runs = ansiRuns('a\nb\tc');
  assert.equal(texts(runs), 'a\nb\tc');
  assert.ok(runs.every((run) => isPrintable(run.text)));
});

test('never lets a control character through, whatever the input mixes', () => {
  const hostile = [
    `${ESC}[38;2;1;2;3mok`,
    `${ESC}]0;title${BEL}`,
    `${ESC}[2J${ESC}[H`,
    `bell${BEL}and${ESC}esc`,
    `${ESC}P1;2q${ESC}\\`,
    CONTROLS,
    `${ESC}[31mred${ESC}[0m`,
  ].join('');
  const runs = ansiRuns(hostile);
  assert.ok(runs.length > 0);
  for (const run of runs) {
    assert.ok(isPrintable(run.text), `not printable: ${JSON.stringify(run.text)}`);
  }
  // The colours still came through: sanitising drops sequences, not content.
  assert.ok(colours(runs).length > 0);
  assert.ok(texts(runs).includes('red'));
});

test('merges adjacent runs that end up with the same styling', () => {
  // A per-cell colour emitter resets and re-asserts the colour it already has.
  const runs = ansiRuns(`${ESC}[38;2;1;2;3ma${ESC}[0m${ESC}[38;2;1;2;3mb`);
  assert.deepEqual(runs, [{text: 'ab', fg: '1, 2, 3', bg: null, decorations: []}]);
});

test('keeps differently styled runs apart', () => {
  const runs = ansiRuns(`${ESC}[31ma${ESC}[32mb`);
  assert.equal(runs.length, 2);
});

test('plainAnsiText drops every sequence but keeps the words', () => {
  const text = plainAnsiText(`${ESC}[38;2;20;69;155mcoloured${ESC}[0m ${ESC}]0;t${BEL}text${BEL}\n`);
  assert.equal(text, 'coloured text\n');
  assert.ok(isPrintable(text));
});