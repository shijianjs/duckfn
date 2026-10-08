/**
 * Regression tests for the runnable-SQL collector.
 *
 * The one that matters here is the **line ending**: the fence regex is
 * `/^(`{3,}|~{3,})(.*)$/`, and `.` never matches `\r` (it is a JavaScript line
 * terminator), so a fence line ending in `\r` did not match at all. Nothing
 * opened, and a CRLF-checked-out document silently produced **zero** blocks —
 * `npm test` / `duckfn-sql-verify` then ran none of its examples without
 * reporting anything. A repository with `core.autocrlf` (this one included) is
 * exactly the place that shows up.
 *
 * Runs against the built `dist/` (this package's tests are Node-side only), so
 * `npm run build` has to have happened first — `just release_kit_check` does
 * that before `npm test`.
 */
import assert from 'node:assert/strict';
import {mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {test} from 'node:test';

import {collectRunnableSql} from '../dist/sql/collect.js';

/** One runnable block plus the front matter a docs page would carry. */
const LINES = [
  '---',
  'title: Fixture',
  '---',
  '',
  '```sql {"type":"duckfn","show":"svg"}',
  'SELECT 1 AS a;',
  '```',
  '',
];

/** Collect a single document written with `eol`, then clean the directory up. */
function collectFixture(name, eol) {
  const dir = mkdtempSync(join(tmpdir(), 'duckfn-docs-kit-'));
  try {
    writeFileSync(join(dir, name), LINES.join(eol), 'utf8');
    return collectRunnableSql({siteDir: dir, contentDirs: ['.']});
  } finally {
    rmSync(dir, {recursive: true, force: true});
  }
}

function assertOneBlock(blocks) {
  assert.equal(blocks.length, 1);
  // The collected SQL is normalised to LF whatever the document used.
  assert.equal(blocks[0].sql, 'SELECT 1 AS a;');
  assert.equal(blocks[0].config.show, 'svg');
}

test('collects a block from an LF document', () => {
  assertOneBlock(collectFixture('lf.md', '\n'));
});

test('collects a block from a CRLF document', () => {
  assertOneBlock(collectFixture('crlf.md', '\r\n'));
});

test('collects a block from a CR-only document', () => {
  assertOneBlock(collectFixture('cr.md', '\r'));
});
