---
title: Testing the examples
sidebar_position: 4
description: Run every runnable SQL block of a docs site in CI — the kit ships the collector, the DuckDB-Wasm runner and the duckfn-sql-verify command.
---

# Testing the examples

Runnable SQL blocks are executable documentation, so they rot the way tests do:
an extension function gets renamed, a default changes, and the block a reader
clicks **Run** on is the only place that notices. The kit ships the same runner
the site uses, so a docs project can check every block in CI:

```bash
duckfn-sql-verify --site .
```

This site wires it up as its `npm test` (see `docs/package.json`), which is what
CI and any local `npm test` in `docs/` runs.

## What it does

1. **Collect** — walk the content directories and take every fenced block whose
   info string is a runnable config (`sql/collect`). The metastring contract is
   the one `sql/remark` implements for the build, so the list is exactly what
   the site publishes — a block that is not runnable there is not collected
   here.
2. **Run** — execute each block in DuckDB-Wasm with the site's extension
   preloaded (`sql/nodeRunner`), on the architecture the page uses: one instance
   per page, and the blocks of a page sharing one connection, so a `CREATE` in
   one block is visible to the next — and pages stay isolated from each other.
3. **Report** — print what failed and exit non-zero if anything did
   (`sql/verify`).

## Blocks that fail on purpose

Half the guide ends on a statement that demonstrates an error, and the metadata
of a runnable block cannot say "this one is meant to fail". The convention is a
comment on that statement:

```sql
SELECT CAST('abc' AS INTEGER);  -- error: not an integer: "abc"
```

A block whose SQL carries `-- error:` or `-- 报错：` anywhere is *expected* to
fail; it is listed separately and does not fail the command. Only unexpected
failures do — which is why the docs pages that demonstrate a failure should say
so in a comment, not only in the prose around the block.

## Options

| Option | Meaning |
| --- | --- |
| `--site <dir>` | Docs site root (default: the working directory). |
| `--content <dir>` | Content directory relative to the site root; repeatable. Defaults to `docs/` plus every `i18n/<locale>/docusaurus-plugin-content-docs/current/`. |
| `--extension <path\|url>` | The extension to preload: a `.duckdb_extension.wasm` path, or an absolute `http(s)` URL. Defaults to the single file under `static/duckdb-extensions/`. |
| `--platform <eh\|mvp>` | DuckDB-Wasm bundle, which has to match the extension build (default: `eh`, the one `selectBundle()` picks in a current browser). |
| `--engine <path>` | Engine wasm override, for pinning a specific DuckDB-Wasm build. |
| `--timeout <ms>` | Per-block timeout (default: 30000) — a hang is reported as a failure instead of blocking CI. |
| `--report <file>` | Write the full per-block result list as JSON. |
| `--quiet` | Only report unexpected failures. |

## Why it runs the way it does

A block is executed in the **Node worker target** of DuckDB-Wasm
(`duckdb-node.cjs`), not the blocking one: an extension that opens its own
connection while registering — which is exactly what a file-system-capable
extension does — deadlocks a runtime that executes DuckDB synchronously on one
thread. The browser never sees this because an extension loads inside a worker
there.

The extension reaches the runner the same way it reaches a page: over an
**http URL**, with the signature check relaxed. Two consequences follow from how
DuckDB stages a fetched extension (`~/.duckdb/extensions/<host>/<first path
segment>/`): the served URL keeps a path segment, and the runner pre-creates
that directory (the loader's own `mkdir` is not recursive). On Windows the local
server takes port 80 so the URL carries no port — a colon is not a legal
character in a Windows path, and the staging directory is named after the URL.
On other platforms any free port is used.
