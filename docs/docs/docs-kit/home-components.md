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

## Content comes from markup

The components are retained-mode internally — the tree is built once in the
constructor and each setter mutates only the nodes it owns — but their input is
ordinary markup. That is the point: the same tags work in a plain `.md` page,
with no JavaScript of yours at all.

Scalars are attributes; the long copy is the child tag's text:

```html
<dfk-hero logo-src="img/duckfn-logo.svg" title="duckfn" tagline="A Rust framework…">
  <dfk-action kind="primary" href="/docs/intro">Get started</dfk-action>
  <dfk-action kind="secondary" href="https://github.com/shijianjs/duckfn"
              icon="simple-icons:github">GitHub</dfk-action>
  <dfk-badge href="https://crates.io/crates/duckfn" src="…" alt="duckfn on crates.io"></dfk-badge>
</dfk-hero>

<dfk-features section-title="Why duckfn">
  <dfk-feature icon="lucide:sparkles" title="No C/C++ glue code">
    DuckDB's C types never appear in your code…
  </dfk-feature>
</dfk-features>

<dfk-next-steps section-title="Where to go next">
  <dfk-step href="/docs/intro" title="Quick start">
    Write, build and load your first extension.
  </dfk-step>
</dfk-next-steps>
```

`href` and `src` are site-root relative (`/docs/intro`), exactly like a
Docusaurus link: the component prefixes the page's baseUrl itself (`/duckfn/`,
or `/duckfn/zh-Hans/` on a localised page), so the same markup works on every
locale and under any deploy prefix. Absolute URLs pass through untouched, and a
path that already carries the prefix is left as-is.

Structures can also arrive as JSON attributes — `primary`, `secondary`,
`badges`, `features`, `steps` — which `attributeChangedCallback` parses and
funnels into the same setters. That is the convenient spelling from JSX
(`badges={JSON.stringify(badges)}`) and an unreadable one from Markdown
(besides, MDX does not accept single-quoted attribute values, so JSON there
would need `&quot;` entities). Hand-written pages use child tags instead.

The child tags (`dfk-action`, `dfk-badge`, `dfk-feature`, `dfk-step`) are *not*
registered custom elements — just data carriers. Their parent reads them once
on connect and renders its shadow tree. `kit.css` hides them
(`dfk-*:defined > * { display: none }`) rather than removing them, because
React owns those nodes and expects its tree to match the DOM; before the
element upgrades they read as plain text instead of an empty box.

The tags are declared in `JSX.IntrinsicElements`, so the same markup works inside
a `.mdx` page — this site's home page is exactly that — with no `createElement`
and no ref:

```tsx
<dfk-next-steps section-title="Where to go next">
  <dfk-step href="/docs/getting-started/quick-start" title="Quick start">
    Write, build and load your first extension.
  </dfk-step>
</dfk-next-steps>
```

An attribute can only hold a string, so a React page that feeds translated copy
into one computes it first with `translate({id, message})`; child text can take
any React node. The site keeps one page per locale rather than routing the copy
through `code.json`.

The named setters (`setTitle`, `setBadges`, …) still exist and are still the
only code that touches the shadow tree — attributes and child tags all funnel
into them. The exported value types are what the JSON attributes parse into:
`HeroLink`, `HeroAction` and `HeroBadge` for the hero, `FeatureItem` for the
grid, `NextStepItem` for the cards.

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

The components render client-side, so the prerendered HTML is a semantic shell:
the text from the child tags is there and readable, but unstyled, and it is
replaced once the elements upgrade. Keeping the light DOM in place costs one
restyle on load and is what keeps hydration mismatch-free.
