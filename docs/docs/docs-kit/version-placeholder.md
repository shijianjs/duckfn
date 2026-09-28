---
title: Version placeholder
sidebar_position: 6
description: One version string for the whole docs tree — replaced at build time by a remark plugin.
---

# Version placeholder

A release should touch one file, not dozens: the docs write a placeholder
token anywhere, and `remarkVersionPlaceholder` substitutes the real version
while the site is built. The block below is live — the value comes from this
site's version file:

```toml
duckfn = "{{DUCKFN_VERSION}}"
duckfn-macro = "{{DUCKFN_VERSION}}"
```

## Configuring

```tsx
import {remarkVersionPlaceholder} from 'duckfn-docs-kit/remark';
import {DUCKFN_VERSION} from './duckfn-version';

// presets -> classic -> docs
remarkPlugins: [
  [remarkVersionPlaceholder, {version: DUCKFN_VERSION}],
],
```

- `version` — the string to substitute in, e.g. `0.0.13`.
- `placeholder` — override the token, if a site wants a different one
  (default: <code>&#123;&#123;DUCKFN_<wbr/>VERSION&#125;&#125;</code>).

## How it substitutes

- Pure text substitution in `text`, `inlineCode` and `code` nodes — code blocks
  included, which is why the demo above shows the real version.
- MDX expression nodes and ESM nodes are left alone, so JavaScript inside MDX
  is never rewritten.
- The substitution runs before anything is rendered, so the token itself never
  reaches a page.
