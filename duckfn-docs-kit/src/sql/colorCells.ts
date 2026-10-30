/**
 * Colour cells for the result table: a column named in the block's `colors`
 * option draws each value as a VTable custom layout made of VisActor `Tag`
 * pills, instead of painting the cell background.
 *
 * Why a tag and not a background fill:
 *
 * - A filled cell colours the *whole* row height, so a table of swatches reads
 *   as a block of stripes and the value itself (the hex code) has to fight the
 *   fill. A pill is only as wide as its text, keeps the cell's own background
 *   and can carry a readable foreground.
 * - An array value (a DuckDB `LIST`, e.g. a palette) becomes one pill per
 *   colour, which a background fill cannot express at all.
 *
 * Both facts are why this is a `customLayout` callback rather than a column
 * `style.bgColor`: VTable lays custom content out per cell, and the pill shapes
 * come from the same `@visactor/vrender-components` package VTable itself
 * renders its components with, so there is no second drawing stack.
 *
 * **What a value is.** A scalar is one colour, an array is several; entries are
 * parsed as CSS colours with `colord` (hex, `rgb()` / `hsl()`, and — because the
 * column is explicitly marked — CSS colour names). That is deliberately wider
 * than `color-swatch/rehype.ts`, which refuses names: there the scan cannot tell
 * `red` the colour from `red` the word, while here the author has said "this
 * column is colours". Values colord cannot parse are still shown, as a neutral
 * pill, so a bad row does not silently disappear; `colord` is not extended with
 * `lab` / `lch` / `oklch` — convert the value to hex or `rgb()` in SQL first.
 *
 * **Translucency** is composited over the table's own row background before the
 * pill is drawn: a canvas fill would otherwise blend with whatever the table
 * painted underneath, and text contrast has to be decided against a known
 * colour. The same composited colour picks the label's black or white ink by
 * WCAG contrast, so a pale swatch never gets white text.
 */
import {colord, extend} from 'colord';
import a11yPlugin from 'colord/plugins/a11y';
import namesPlugin from 'colord/plugins/names';
import type * as VTable from '@visactor/vtable';
import type * as VRenderComponents from '@visactor/vrender-components';

extend([a11yPlugin, namesPlugin]);

/** The two constructors the layout needs, both loaded lazily by the renderer. */
export interface ColorCellConstructors {
  Group: typeof VTable.Group;
  Tag: typeof VRenderComponents.Tag;
}

/** Theme-derived values a pill is drawn with. */
export interface ColorCellVisual {
  /** Row background translucent fills are composited over. */
  background: string;
  /** Body text colour, used by a pill whose value did not parse. */
  textColor: string;
  fontFamily: string;
  fontSize: number;
  /** Cell padding as `[top, right, bottom, left]`. */
  padding: readonly number[];
}

/** The shape VTable's `customLayout` callback has to return. */
export interface ColorCellLayout {
  rootContainer: InstanceType<ColorCellConstructors['Group']>;
  renderDefault: boolean;
}

/**
 * The part of VTable's `customLayout` argument this layout reads. `table` is
 * there for its stage: text measurement (and therefore a tag's size) does not
 * work without one.
 */
export interface ColorCellLayoutArgs {
  value: unknown;
  rect?: {width?: number; height?: number};
  table?: {scenegraph?: {stage?: unknown}};
}

/** One pill: its label and the two colours it is drawn with. */
export interface ColorTagSpec {
  text: string;
  /** Panel fill, already composited over the table background. */
  fill: string;
  /** Label ink, picked by contrast against {@link fill}. */
  color: string;
}

/**
 * The columns `config.colors` marks, in declaration order, de-duplicated.
 * A single string is the common case and is accepted as a one-item list.
 */
export function colorColumnsOf(colors: unknown): string[] {
  const list = typeof colors === 'string' ? [colors] : Array.isArray(colors) ? colors : [];
  const fields = list
    .filter((value): value is string => typeof value === 'string')
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
  return [...new Set(fields)];
}

/**
 * The pills for one cell. A scalar value is one pill; a `LIST` is one per
 * entry, whether it arrives as a JS array or as the Arrow vector DuckDB-Wasm
 * actually produces. `NULL`/`undefined`/empty entries are skipped (there is
 * nothing to paint), non-primitive entries are skipped (a `plt_color` STRUCT is
 * not a CSS colour, and `[object Object]` on a pill helps nobody — such a cell
 * keeps the default text).
 */
export function colorTagSpecs(
  value: unknown,
  background: string,
  textColor: string,
): ColorTagSpec[] {
  const items = listItems(value);
  const specs: ColorTagSpec[] = [];
  for (const item of items) {
    if (item === null || item === undefined || typeof item === 'object') {
      continue;
    }
    const text = String(item).trim();
    if (text.length === 0) {
      continue;
    }
    const parsed = colord(text);
    if (!parsed.isValid()) {
      specs.push({text, fill: NEUTRAL_FILL, color: textColor});
      continue;
    }
    const fill = compositeOver(parsed, background);
    specs.push({text, fill, color: inkOn(fill)});
  }
  return specs;
}

/** The neutral pill a value that is not a colour is shown in. */
const NEUTRAL_FILL = 'rgba(128, 128, 128, 0.18)';

/**
 * The entries of a value, whichever shape the LIST arrived in.
 *
 * DuckDB-Wasm hands a `LIST` cell over as an Arrow `Vector`, not as a JS array:
 * a `Vector` is array-like and iterable, with `toArray()` being its documented
 * conversion. `Array.isArray` alone would therefore miss every array column and
 * leave it as plain text. A struct is not a list (its cell is a row proxy with
 * `toJSON`, not `toArray`), so it still falls through to the scalar branch and
 * ends up with nothing to paint.
 */
function listItems(value: unknown): unknown[] {
  if (Array.isArray(value)) {
    return value;
  }
  if (value !== null && typeof value === 'object' && typeof (value as {toArray?: unknown}).toArray === 'function') {
    return (value as {toArray(): unknown[]}).toArray();
  }
  return [value];
}

/**
 * Builds the `customLayout` result for one cell: a group of pills laid out in
 * one row and centred vertically. `renderDefault` is `true` when nothing could
 * be painted, which is what tells VTable to draw the value as ordinary text
 * (a STRUCT, an all-NULL list, an empty string).
 */
export function colorCellLayout(
  constructors: ColorCellConstructors,
  visual: ColorCellVisual,
  args: ColorCellLayoutArgs,
): ColorCellLayout {
  const group = new constructors.Group({
    x: 0,
    y: 0,
    width: 0,
    height: 0,
    fill: false,
    stroke: false,
    pickable: false,
  });

  const specs = colorTagSpecs(args.value, visual.background, visual.textColor);
  if (specs.length === 0) {
    return {rootContainer: group, renderDefault: true};
  }

  // A tag only knows its size once text can be measured, and that needs the
  // table's stage — without it `AABBBounds` is empty, whatever the text. VTable
  // hands the stage to the root container only *after* this callback returns,
  // which is too late for the widths the column is sized from, so the group
  // takes it from the table here. Children inherit it when appended.
  const stage = args.table?.scenegraph?.stage;
  if (stage) {
    group.setStage(stage as Parameters<InstanceType<ColorCellConstructors['Group']>['setStage']>[0]);
  }

  const [top = 0, right = 0, bottom = 0, left = 0] = visual.padding;
  const pills: {tag: InstanceType<ColorCellConstructors['Tag']>; height: number}[] = [];
  let x = left;
  for (const spec of specs) {
    const tag = new constructors.Tag({
      text: spec.text,
      textStyle: {
        fill: spec.color,
        fontFamily: visual.fontFamily,
        fontSize: visual.fontSize,
        textAlign: 'left',
        textBaseline: 'top',
      },
      panel: {
        visible: true,
        fill: spec.fill,
        stroke: false,
        cornerRadius: TAG_RADIUS,
      },
      shape: {visible: false},
      padding: [TAG_PAD_Y, TAG_PAD_X, TAG_PAD_Y, TAG_PAD_X],
    });
    // `Group` and `Tag` come from two entry points of the same render library,
    // whose `.d.ts` files describe the child type structurally; the runtime
    // objects compose fine, the cast only bridges the two declarations.
    group.appendChild(tag as unknown as Parameters<typeof group.appendChild>[0]);
    const width = tag.AABBBounds.width();
    const height = tag.AABBBounds.height();
    tag.setAttributes({x});
    pills.push({tag, height});
    x += width + TAG_GAP;
  }

  // The group spans the whole cell (`enableCellPadding` stays off), so vertical
  // centring is done here rather than by VTable.
  const tallest = Math.max(...pills.map((pill) => pill.height));
  const cellHeight = args.rect?.height ?? top + bottom + tallest;
  const y = (cellHeight - tallest) / 2;
  for (const {tag} of pills) {
    tag.setAttributes({y});
  }
  const contentWidth = Math.max(left, x - TAG_GAP) + right;
  group.setAttributes({width: contentWidth, height: cellHeight});
  return {rootContainer: group, renderDefault: false};
}

/** Space between two pills, and the pill's own metrics. */
const TAG_GAP = 6;
const TAG_RADIUS = 9;
const TAG_PAD_X = 9;
const TAG_PAD_Y = 3;

/** Black or white, whichever is more readable on `fill`. */
function inkOn(fill: string): string {
  const background = colord(fill);
  return background.contrast('#ffffff') >= background.contrast('#000000') ? '#ffffff' : '#000000';
}

/** `color` over `background`, so a translucent value has a definite fill. */
function compositeOver(color: ReturnType<typeof colord>, background: string): string {
  const {r, g, b, a} = color.rgba;
  if (a >= 1) {
    return color.toHex();
  }
  const under = colord(background);
  if (!under.isValid()) {
    return color.toHex();
  }
  const {r: ur, g: ug, b: ub} = under.rgba;
  const blend = (foreground: number, backdrop: number) =>
    Math.round(foreground * a + backdrop * (1 - a));
  return `rgb(${blend(r, ur)}, ${blend(g, ug)}, ${blend(b, ub)})`;
}
