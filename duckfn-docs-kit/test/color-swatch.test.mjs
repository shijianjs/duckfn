/**
 * Tests for the colour-swatch rehype plugin.
 *
 * The plugin is a plain hast→hast transform, so these tests build the trees by
 * hand instead of parsing markdown: no parser, no MDX runtime, no Docusaurus —
 * just the node shapes the plugin is documented against, which keeps the test on
 * the contract that matters (which element becomes a swatch, with which style)
 * and off the site's own markdown pipeline.
 *
 * Runs against the built `dist/` (`npm run build` first; `just release_kit_check`
 * does that, then `npm test`).
 */
import assert from 'node:assert/strict';
import {test} from 'node:test';

import {
  COLOR_SWATCH_ATTRIBUTE,
  COLOR_SWATCH_CLASS,
  rehypeColorSwatch,
} from '../dist/color-swatch/rehype.js';

/** Runs the plugin over a paragraph's children and returns the new children. */
const run = (children, options) => {
  const tree = {type: 'root', children: [{type: 'element', tagName: 'p', children}]};
  rehypeColorSwatch(options)(tree);
  return tree.children[0].children;
};

const code = (value) => ({
  type: 'element',
  tagName: 'code',
  properties: {},
  children: [{type: 'text', value}],
});

/** An explicitly marked `<code>`; `color === null` writes the value-less form. */
const marked = (color, text) => ({
  type: 'element',
  tagName: 'code',
  properties:
    color === null
      ? {dataColorSwatch: ''}
      : {dataColorSwatch: color},
  children: [{type: 'text', value: text}],
});

test('paints an inline code that is a colour value', () => {
  const [node] = run([code('#E69F00')]);

  assert.deepEqual(node.properties.className, [COLOR_SWATCH_CLASS]);
  // The text stays exactly what the page wrote; only the style is derived.
  assert.deepEqual(node.children, [{type: 'text', value: '#E69F00'}]);
  // Mid-tone orange: black has the higher contrast of the two.
  assert.match(node.properties.style, /background-color: #e69f00/);
  assert.match(node.properties.style, /color: #000000/);
});

test('picks a white foreground for a dark colour', () => {
  const [node] = run([code('#0072B2')]);

  assert.match(node.properties.style, /background-color: #0072b2/);
  assert.match(node.properties.style, /color: #ffffff/);
});

test('accepts the functional CSS notations colord parses', () => {
  // Whatever `colord` parses out of the box — its extra notations (and CSS
  // names) need plugins, and the plugin adds none beyond `a11y`.
  for (const source of ['rgb(0, 114, 178)', 'rgba(0, 114, 178, 1)', 'hsl(210, 100%, 35%)']) {
    const [node] = run([code(source)]);
    assert.ok(node.properties.style, source);
    assert.match(node.properties.style, /background-color: #/, source);
  }
});

test('makes a translucent colour opaque', () => {
  const [node] = run([code('rgba(0, 0, 0, 0.4)')]);

  // Otherwise the tag's own background shows through and the swatch reads as
  // something other than the colour it names.
  assert.match(node.properties.style, /background-color: #000000/);
  assert.match(node.properties.style, /color: #ffffff/);
});

test('leaves anything that is not a colour value alone', () => {
  for (const source of ['x_axis.wrap', '#section', '10px', '']) {
    const [node] = run([code(source)]);
    assert.equal(node.properties.style, undefined, source);
    assert.deepEqual(node.children, [{type: 'text', value: source}]);
  }
});

test('does not paint CSS colour names while scanning', () => {
  // `red`, `white` and `transparent` are ordinary words in prose, so naming a
  // colour is what the attribute is for (see the test below).
  for (const source of ['red', 'white', 'transparent', 'steelblue']) {
    const [node] = run([code(source)]);
    assert.equal(node.properties.style, undefined, source);
  }
});

test('scan: false turns the scan off but keeps the explicit marker', () => {
  const [plain] = run([code('#E69F00')], {scan: false});
  assert.equal(plain.properties.style, undefined);

  const [explicit] = run([marked('#E69F00', 'any text')], {scan: false});
  assert.match(explicit.properties.style, /background-color: #e69f00/);
});

test('paints a marked element whose text is the colour', () => {
  const [node] = run([marked(null, '#E69F00')]);

  assert.deepEqual(node.children, [{type: 'text', value: '#E69F00'}]);
  assert.match(node.properties.style, /background-color: #e69f00/);
  // The marker has done its job and does not reach the page.
  assert.equal(node.properties.dataColorSwatch, undefined);
});

test('paints arbitrary text with an explicit colour', () => {
  const [node] = run([marked('rgb(0, 114, 178)', 'not a colour value')]);

  assert.deepEqual(node.children, [{type: 'text', value: 'not a colour value'}]);
  assert.match(node.properties.style, /background-color: #0072b2/);
  assert.match(node.properties.style, /color: #ffffff/);
});

test('accepts no CSS colour name, marked or not', () => {
  // Names need `colord/plugins/names`, which is deliberately not loaded: the
  // scan cannot tell `red` the colour from `red` the word, so names are out
  // everywhere and a page writes hex / rgb() / hsl() instead.
  for (const source of ['tomato', 'steelblue']) {
    const [scanned] = run([code(source)]);
    assert.equal(scanned.properties.style, undefined, source);

    const [explicit] = run([marked(source, 'text')]);
    assert.equal(explicit.properties.style, undefined, source);
  }
});

test('leaves a marked element with an unparsable colour untouched', () => {
  const [node] = run([marked('not-a-colour', 'text')]);

  assert.equal(node.properties.style, undefined);
  assert.equal(node.properties.dataColorSwatch, 'not-a-colour');
});

test('appends to the class and style the author wrote', () => {
  const [node] = run([
    {
      type: 'element',
      tagName: 'code',
      properties: {className: ['mine'], style: 'border: 0', dataColorSwatch: '#E69F00'},
      children: [{type: 'text', value: 'text'}],
    },
  ]);

  assert.deepEqual(node.properties.className, ['mine', COLOR_SWATCH_CLASS]);
  assert.match(node.properties.style, /^border: 0; background-color: #e69f00/);
});

test('leaves a fenced code block alone', () => {
  // A block is `<pre><code>`, so a colour inside an example is source, not a
  // sample.
  const pre = {
    type: 'element',
    tagName: 'pre',
    children: [code('#E69F00')],
  };
  const [node] = run([pre]);

  assert.equal(node.children[0].properties.style, undefined);
});

test('reads the raw-HTML spelling of the marker too', () => {
  const [node] = run([
    {
      type: 'element',
      tagName: 'code',
      properties: {'data-color-swatch': '#E69F00'},
      children: [{type: 'text', value: 'text'}],
    },
  ]);

  assert.match(node.properties.style, /background-color: #e69f00/);
  assert.equal(node.properties['data-color-swatch'], undefined);
});

test('exposes the attribute name it documents', () => {
  assert.equal(COLOR_SWATCH_ATTRIBUTE, 'data-color-swatch');
});
