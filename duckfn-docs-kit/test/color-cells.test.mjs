/**
 * Tests for the value rules behind the table's `colors` option.
 *
 * The layout itself needs VisActor's `Group` / `Tag` constructors and a canvas,
 * so it is checked in the browser; everything that decides *what* a cell shows
 * — which entries become pills, which colours they get, and how a value that is
 * not a colour is treated — is pure and pinned here.
 *
 * Runs against the built `dist/` (`npm run build` first; `just release_kit_check`
 * does that, then `npm test`).
 */
import assert from 'node:assert/strict';
import {test} from 'node:test';

import {colorColumnsOf, colorTagSpecs} from '../dist/sql/colorCells.js';

const WHITE = '#ffffff';
const DARK = '#373b45';
const INK = '#000000';

test('a single column name is a one-item list', () => {
  assert.deepEqual(colorColumnsOf('hex'), ['hex']);
});

test('column names are trimmed, de-duplicated and order-preserving', () => {
  assert.deepEqual(colorColumnsOf([' palette ', 'hex', 'palette', '']), ['palette', 'hex']);
});

test('non-string entries and a missing option name no columns', () => {
  assert.deepEqual(colorColumnsOf(undefined), []);
  assert.deepEqual(colorColumnsOf(null), []);
  assert.deepEqual(colorColumnsOf([1, true, 'hex']), ['hex']);
});

test('a scalar colour is one pill, in the colour itself', () => {
  assert.deepEqual(colorTagSpecs('#ff8000', WHITE, INK), [
    {text: '#ff8000', fill: '#ff8000', color: '#000000'},
  ]);
});

test('CSS colour names parse, because the column is explicitly marked as colours', () => {
  const [spec] = colorTagSpecs('red', WHITE, INK);
  assert.equal(spec?.fill, '#ff0000');
});

test('a list is one pill per entry; NULL and empty entries are skipped', () => {
  const specs = colorTagSpecs(['#ff0000', null, '', 'red'], WHITE, INK);
  assert.deepEqual(
    specs.map((spec) => spec.text),
    ['#ff0000', 'red'],
  );
});

test('a LIST that arrives as an Arrow vector is read through toArray()', () => {
  // DuckDB-Wasm's LIST cells are Arrow vectors, not JS arrays.
  const vector = {toArray: () => ['#440154', '#fde725']};
  assert.deepEqual(
    colorTagSpecs(vector, WHITE, INK).map((spec) => spec.text),
    ['#440154', '#fde725'],
  );
});

test('a value that is not a colour keeps its text in a neutral pill', () => {
  const [spec] = colorTagSpecs('nope', WHITE, '#e5e7ea');
  assert.equal(spec?.text, 'nope');
  assert.equal(spec?.fill, 'rgba(128, 128, 128, 0.18)');
  assert.equal(spec?.color, '#e5e7ea');
});

test('nothing to paint yields no pills, so the cell keeps its default text', () => {
  assert.deepEqual(colorTagSpecs(null, WHITE, INK), []);
  assert.deepEqual(colorTagSpecs([], WHITE, INK), []);
  assert.deepEqual(colorTagSpecs({space: 'srgb'}, WHITE, INK), []);
  assert.deepEqual(colorTagSpecs([{a: 1}, null], WHITE, INK), []);
});

test('the label ink is black or white by contrast, not by a brightness guess', () => {
  assert.equal(colorTagSpecs('#ff8000', WHITE, INK)[0]?.color, '#000000');
  assert.equal(colorTagSpecs('#003366', WHITE, INK)[0]?.color, '#ffffff');
});

test('translucency is composited over the table background', () => {
  assert.equal(colorTagSpecs('#ff000080', WHITE, INK)[0]?.fill, 'rgb(255, 128, 128)');
  // The dark theme's row background, so the same value reads differently there.
  assert.equal(colorTagSpecs('#ff000080', DARK, '#e5e7ea')[0]?.fill, 'rgb(155, 30, 35)');
  // ...and the ink follows the composited colour, not the raw one.
  assert.equal(colorTagSpecs('#ff000080', DARK, '#e5e7ea')[0]?.color, '#ffffff');
});
