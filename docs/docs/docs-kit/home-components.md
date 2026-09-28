---
title: Home components
sidebar_position: 5
description: The dfk-hero / dfk-features / dfk-next-steps web components — the building blocks of the landing page.
---

# Home components

Three shadow-DOM web components make up the landing layout: `<dfk-hero>` (logo,
title, tagline, actions, badges), `<dfk-features>` (a section title and a card
grid) and `<dfk-next-steps>` (link cards). The live demo is this site's
[home page](/) — everything above the code showcase is these components.

## Content goes through setters

They are retained-mode elements, not attribute-driven ones: the structure is
built once in the constructor and each setter mutates only the nodes it owns.
From React, mount them through a callback ref — React 19 reconciles only
string props onto custom elements, so an object payload would never survive
hydration:

```tsx
import {createElement} from 'react';
import type {ReactNode} from 'react';
import {DfkNextSteps, type NextStepItem} from 'duckfn-docs-kit';

function mountSteps(node: HTMLElement): void {
  const steps = node as DfkNextSteps;
  steps.setSectionTitle('Where to go next');
  steps.setSteps([
    {href: '/docs/intro', title: 'Introduction', details: 'What duckfn is.'},
  ] satisfies NextStepItem[]);
}

const element: ReactNode = createElement('dfk-next-steps', {ref: mountSteps});
```

The content types come from the barrel — `HeroAction`, `HeroBadge` and
`HeroLink` for the hero, `FeatureItem` for the grid, `NextStepItem` for the
cards:

```tsx
import type {FeatureItem, HeroAction, HeroBadge, HeroLink, NextStepItem} from 'duckfn-docs-kit';
```

## Registering the elements

The `dfk-*` tags have to be defined once per app boot with
`registerDfkElements()` — it also registers the official `<iconify-icon>`
element used for every glyph. A site that configures the extension preloading
gets the registration injected on every page; a site that only uses the home
components calls it at module scope on its home page:

```tsx
import {registerDfkElements} from 'duckfn-docs-kit';

registerDfkElements();
```

## Styling and theming

- The components carry their own styles inside the JS bundle and inject them
  into their shadow roots (`adoptedStyleSheets`), so nothing leaks either way.
- The `--duckfn-*` brand tokens come from `src/theme/tokens.css`, part of
  `kit.css`; the site's Infima (`--ifm-*`) variables inherit into the shadow
  trees, so dark mode and the site palette apply automatically.
- Icons are Iconify names (`'lucide:sparkles'`), rendered by the official
  `<iconify-icon>` element; sizes follow `font-size`.

## Known trade-off

The components render client-side, so their prerendered HTML is an empty shell
until hydration. This is deliberate: it is what keeps the light DOM empty and
hydration mismatch-free, at the cost of one height jump on first paint.
