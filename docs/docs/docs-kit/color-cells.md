---
title: Colour cells
sidebar_position: 9
description: The block's `colors` option draws a table column's values as colour pills — one per value, one per list entry.
---

# Colour cells

A table of colours should read as colour. `colors` names the column — or a list of
columns — whose values are colours; the table then draws each value as a pill
filled with that colour, labelled with the value itself. A background fill would
colour the whole row height and could not show more than one colour per cell,
which is why this is a pill and not a cell style.

The option belongs to a runnable SQL block (see
[Runnable SQL blocks](./runnable-sql.md)) and needs nothing beyond the kit the
block already uses:

```sql {"type":"duckfn","show":"table","colors":"hex"}
SELECT '#ff8000' AS hex, 'orange' AS name
UNION ALL SELECT '#440154', 'viridis, dark end'
UNION ALL SELECT 'tomato', 'a CSS colour name'
UNION ALL SELECT 'not a colour', 'still shown, in grey';
```

Only `show: "table"` draws pills — the default for a multi-row or multi-column
result. The other renderers ignore `colors`.

## One pill per list entry

A `LIST` value becomes one pill per entry, the natural shape for a palette.
`NULL` and empty entries are skipped; an empty list keeps the empty cell.

```sql {"type":"duckfn","show":"table","colors":"palette"}
SELECT ['#440154', '#21918c', '#fde725'] AS palette, 'viridis' AS name;
```

## What counts as a colour

Values are read as **CSS colours**: hex (`#rgb`, `#rrggbb`, and the 4- and 8-digit
alpha forms), `rgb()` / `rgba()`, `hsl()` / `hsla()`, and CSS colour names
(`tomato`). Nothing else — `oklch()` and `lab()` are not parsed; convert them to
hex in SQL first.

- A value that is **not a colour** stays visible in a neutral grey pill, so a bad
  row does not silently disappear.
- A whole `STRUCT` value (such as a colour type that is not a string) has no text
  to show, and that cell falls back to the ordinary text. Convert it to hex in
  SQL.
- **Translucent values are composited over the row background**, light or dark,
  and the label's ink is black or white by WCAG contrast, so a pale swatch never
  gets white text.
- A column name the query does not return is a **typo**, not an empty result: the
  table shows an error naming the missing column.

## How the pills are drawn

The pills come from VTable's `customLayout` with VisActor's `Tag`, not from
`style.bgColor`. `@visactor/vrender-components` is imported only when a result
actually has a colour column, so a table without one pays nothing for the
feature. There is no per-site setup: the option is part of the block the site
already runs.
