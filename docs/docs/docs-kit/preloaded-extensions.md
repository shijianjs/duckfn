---
title: Preloaded extensions
sidebar_position: 3
description: Load DuckDB-Wasm extensions on every page — one ordered preload list, fetched from GitHub releases at build time and cached by sha256.
---

# Preloaded extensions

Runnable SQL blocks can name the extensions they need
([Runnable SQL blocks](./runnable-sql.md)), but a docs site that documents one
extension should not make every example name it. The `dfkExtensions` plugin
takes an ordered **preload list** instead:

- at dev/build startup it fetches every GitHub-release source into the site's
  static directory — cached locally, re-downloaded only when the release asset
  changes;
- it injects the list into every page;
- the kit's runtime loads the extensions, in order, while DuckDB initialises —
  starting in the background as soon as a page with a runnable block opens, so
  the first **Run** click is fast.

A block can then simply call into the extension — nothing to declare:

```sql {"type":"duckfn"}
SELECT version() AS engine, double_it5(21) AS doubled;
```

## Configuring

```tsx
import {dfkExtensions} from 'duckfn-docs-kit/sql/extensions';

plugins: [
  dfkExtensions({
    // CI builds the release assets without DuckDB's signing keys — the same
    // reason local development runs `duckdb -unsigned`.
    allowUnsignedExtensions: true,
    preload: [
      // The extension this site documents, served same-origin from the
      // latest release of its GitHub repository.
      {
        url: 'duckdb-extensions/duckfn.duckdb_extension.wasm',
        release: {
          repository: 'shijianjs/duckfn',
          asset: 'duckfn-wasm_eh.duckdb_extension.wasm',
        },
      },
    ],
  }),
],
```

The plugin runs its fetch at startup for both `npm start` and the production
build, so the file exists in the dev server and in `static/`.

## The three source kinds

1. **A name** — `'json'`: the runtime runs `LOAD json` against the official
   repository.
2. **Name + repository** — `{name: 'h3', repository: 'community'}`: the
   repository may be `community`, `core` or a URL. The runtime records the
   source with `INSTALL … FROM` and then `LOAD`s the name. On WebAssembly
   `INSTALL` writes nothing to disk — there is no persistent storage to install
   into — it only records where `LOAD` should fetch from, which is also why the
   record stays attached to this one extension instead of polluting later
   loads.
3. **A file** — `{url: …, release?: …}`: the site serves the wasm file itself.
   With `release`, the build fetches the asset from that GitHub repository's
   **latest** release into `static/<url>` (the same path the runtime loads);
   without it, the file is placed by hand under `static/<url>` and the build
   only checks that it is there. `url` may also be an absolute `http(s)` URL.

## GitHub-release sources

For a `{url, release}` entry the plugin:

- asks the GitHub API for the repository's latest release and finds the asset
  by name (a miss lists the available names);
- compares the asset's sha256 — GitHub's own `digest` field — with the local
  cache and downloads **only when they differ**;
- verifies what it downloads against that digest, stores it under
  `<siteDir>/.cache/duckfn-docs-kit/` and copies it to `static/<url>`;
- falls back to the cached copy with a warning when the network is down, so
  offline development keeps working (CI always starts empty and fails loudly).

`GITHUB_TOKEN` (or the plugin's `token` option) lifts the anonymous API rate
limit; neither is needed for public repositories on a normal workstation.

## The file-name contract

The text before the **first dot** of the file name is the entry symbol DuckDB
looks up — `duckfn.duckdb_extension.wasm` loads through
`duckfn_init_c_api`. Release assets carry the platform suffix
(`duckfn-wasm_eh.duckdb_extension.wasm`), so they are renamed on the way in:
the destination in `url` decides the file name, and both the plugin and the
runtime reject names whose base is not a valid extension identifier.

## Versions and signing

- **Platform**: the preloaded file must match the runtime bundle's WebAssembly
  platform. Sites that let `selectBundle()` pick serve the `wasm_eh` asset;
  this is what the docs site does.
- **DuckDB version**: WebAssembly extensions are only loadable by a DuckDB-Wasm
  build with a compatible C API. The site pins `@duckdb/duckdb-wasm` to an
  exact version whose bundled engine matches the `duckdb_version` that CI
  builds the extension against; when either side moves, re-check these pages'
  live blocks.
- **Signing**: third-party release assets are not signed with DuckDB's keys, so
  a site that preloads them needs `allowUnsignedExtensions: true` (the
  WebAssembly equivalent of running the CLI with `-unsigned`). Community
  extensions are signed and load without it.

## Where it lands in the page

The plugin injects the resolved list as one JSON `<script>` tag into every
page:

```json
{"allowUnsignedExtensions":true,"preload":[{"url":"/duckdb-extensions/duckfn.duckdb_extension.wasm"}]}
```

The tag keeps its `id="dfk-sql-runtime"` and can be inspected in the page
source when debugging. A malformed list fails DuckDB initialisation with a
readable error in the first block that runs.
