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

This site wires it up as its `npm test` (see `docs/package.json`), so running
`npm test` in `docs/` is the whole flow. It is deliberately **not** part of CI in
this repository: the test needs the wasm build of the extension, and that artefact
only comes out of duckfn's own CI (nine platform artifacts, around 20 minutes).
The repository's `AGENTS.md` records that trade-off.

## What it does

1. **Collect** — walk the content directories and take every fenced block whose
   info string is a runnable config (`sql/collect`). The metastring contract is
   the one `sql/remark` implements for the build, so the list is exactly what
   the site publishes — a block that is not runnable there is not collected
   here.
2. **Run** — execute each block in DuckDB-Wasm **in a headless browser**, with
   the site's extension preloaded (`sql/browserRunner` drives the browser, and
   the page it loads — `sql/harness` — reuses the very `sql/runtime` the site
   runs). Same architecture as the page: one instance per page, the blocks of a
   page sharing one connection, so a `CREATE` in one block is visible to the
   next — and pages stay isolated from each other.
3. **Report** — print what failed and exit non-zero if anything did
   (`sql/verify`).

## Blocks that fail on purpose

Half the guide ends on a statement that demonstrates an error, and such a block
has to declare it — the suite checks both directions, so a block that stops
failing is reported just like one that starts failing:

```sql {"type":"duckfn","expect":"error"}
SELECT CAST('abc' AS INTEGER);  -- error: not an integer: "abc"
```

`"expect"` defaults to `"ok"`; `"error"` means *this block must fail*. It is the
only thing the suite reads: the prose around a block, and a `-- error: …` comment
inside it, are there for readers and are not machine-checked.

## Options

| Option | Meaning |
| --- | --- |
| `--site <dir>` | Docs site root (default: the working directory). |
| `--content <dir>` | Content directory relative to the site root; repeatable. Defaults to `docs/` plus every `i18n/<locale>/docusaurus-plugin-content-docs/current/`. |
| `--extension <path\|url>` | The extension to preload: a `.duckdb_extension.wasm` path, or an absolute `http(s)` URL. Defaults to the single file under `static/duckdb-extensions/`. |
| `--platform <eh\|mvp>` | DuckDB-Wasm bundle, which has to match the extension build (default: `eh`, the one `selectBundle()` picks in a current browser). |
| `--engine <path>` | Engine wasm override, for pinning a specific DuckDB-Wasm build. |
| `--browser <path>` | The Chrome/Edge executable to drive. Defaults to a detected system Chrome/Edge, or the `DFK_BROWSER` environment variable. `playwright-core` launches it directly — no browser download. |
| `--timeout <ms>` | Per-block timeout (default: 30000) — a hang is reported as a failure instead of blocking CI. |
| `--report <file>` | Write the full per-block result list as JSON. |
| `--quiet` | Only report unexpected failures. |

## Why it runs the way it does

A block is executed in **DuckDB-Wasm inside a real browser** — the environment a
reader gets — not in Node. This is deliberate: the old Node-worker target could
not read remote `http(s)` data (every remote-data example failed with an
`IO Error`), so blocks that read real files could only ever be checked by hand
in a browser. Running the suite in a browser makes that the normal case.

The browser is driven with **Playwright** (`playwright-core`), which owns the protocol, navigation,
auto-waiting, timeouts and crash handling rather than a bespoke driver. It uses the
`playwright-core` package specifically because that one never downloads a browser — it launches your
system Chrome/Edge directly via `executablePath`. Everything else is served locally, so it runs
offline: the engine (`duckdb-*.wasm` and its worker script) from `node_modules`, the extension from
`static/duckdb-extensions/`, over a loopback http server the harness page fetches them from. Loading
the extension over http works exactly like it does on the site, with none of the port/staging-directory
constraints the Node worker imposed.

```sql
-- A remote read like this is what the Node worker could never do; a browser can.
-- (Not a runnable block here: fetching it needs the network, and the suite runs offline.)
SELECT count(*) AS n FROM read_csv_auto('https://example.com/data/smallest.csv');
```

### File-system examples are the exception

The browser's raw file system is not a faithful POSIX layer: DuckDB-Wasm opens a
file that was never written and hands back zero-filled bytes, so `dfn_file_exists`
reports `true` for things that are absent, and `COPY … TO` / `append` do not land
the bytes you would expect. DuckDB's C API exposes no existence primitive an
extension could use to correct this, so it is a platform limit rather than a
bug — and examples that depend on the file system stay **plain (non-runnable)
code blocks**, with the reason noted on the page. The native build (and the
native test suite `test/sql/functions/duck_vfs.test`) does behave correctly.
