/**
 * Node-side collector for runnable SQL blocks: walks a docs site's content and
 * returns every fenced block whose info string is a runnable config.
 *
 * The metastring contract belongs to `sql/remark.ts` (one parser, exported as
 * `parseRunnableSqlMeta`), so the blocks collected here are exactly the ones
 * the build turns into `<dfk-sql>` — a CI check over this list therefore covers
 * what the site actually publishes.
 *
 * Fences follow CommonMark closely enough for a docs tree: a closing fence has
 * to use the same character, be at least as long, and carry no info string.
 * That is what keeps a ```sql example *inside* a ````md wrapper (as
 * `docs-kit/runnable-sql.md` shows the metastring) from being collected as a
 * block of its own.
 */
import {readFileSync, readdirSync, statSync} from 'node:fs';
import {join, relative, sep} from 'node:path';

import {parseRunnableSqlMeta, type RunnableSqlConfig} from './remark';

/** One runnable block, positioned so a failure can name it. */
export interface RunnableSqlBlock {
  /** Path relative to the site root, always with forward slashes. */
  file: string;
  /** 1-based line of the opening fence. */
  line: number;
  config: RunnableSqlConfig;
  sql: string;
}

export interface CollectRunnableSqlOptions {
  /** Site root the reported paths are relative to, and the base of a relative dir. */
  siteDir: string;
  /**
   * Directories to scan (absolute, or relative to `siteDir`). Missing ones are
   * skipped rather than reported: a site may have no translations.
   */
  contentDirs: readonly string[];
  /** File extensions to scan; both `.md` and `.mdx` are markdown to us. */
  extensions?: readonly string[];
}

const DEFAULT_EXTENSIONS = ['.md', '.mdx'] as const;

/** Every runnable block under `contentDirs`, in file order, then line order. */
export function collectRunnableSql(options: CollectRunnableSqlOptions): RunnableSqlBlock[] {
  const {siteDir, contentDirs, extensions = DEFAULT_EXTENSIONS} = options;
  const files: string[] = [];
  for (const dir of contentDirs) {
    collectFiles(join(siteDir, dir), extensions, files);
  }
  const blocks: RunnableSqlBlock[] = [];
  for (const file of files.sort()) {
    for (const block of blocksOf(readFileSync(file, 'utf8'))) {
      blocks.push({
        ...block,
        file: relative(siteDir, file).split(sep).join('/'),
      });
    }
  }
  return blocks;
}

function collectFiles(dir: string, extensions: readonly string[], out: string[]): void {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    // Nothing to scan here (typically a translation that does not exist yet).
    return;
  }
  for (const name of entries) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      collectFiles(path, extensions, out);
    } else if (extensions.some((extension) => name.endsWith(extension))) {
      out.push(path);
    }
  }
}

interface RawBlock {
  line: number;
  config: RunnableSqlConfig;
  sql: string;
}

/** The runnable blocks of one markdown document. */
function blocksOf(text: string): RawBlock[] {
  const lines = text.split('\n');
  const out: RawBlock[] = [];
  let open: {marker: string; info: string; line: number} | null = null;
  let body: string[] = [];

  for (let index = 0; index < lines.length; index++) {
    const line = lines[index] ?? '';
    const fence = /^(`{3,}|~{3,})(.*)$/.exec(line);
    if (!fence) {
      if (open) {
        body.push(line);
      }
      continue;
    }
    const [marker, info] = [fence[1] ?? '', (fence[2] ?? '').trim()];
    if (!open) {
      open = {marker, info, line: index + 1};
      body = [];
      continue;
    }
    const closes =
      marker.charAt(0) === open.marker.charAt(0) && marker.length >= open.marker.length && info === '';
    if (!closes) {
      // A shorter or differently marked fence is content of the open block.
      body.push(line);
      continue;
    }
    if (open.info.startsWith('sql')) {
      const config = parseRunnableSqlMeta(open.info.slice('sql'.length).trim());
      if (config) {
        out.push({line: open.line, config, sql: body.join('\n')});
      }
    }
    open = null;
    body = [];
  }
  return out;
}

/**
 * Whether a block is expected to fail — the block's own `"expect": "error"`
 * metadata, never its prose or its comments: an expectation that the SQL test
 * suite acts on has to be data, not a string match on a comment.
 */
export function expectsError(config: RunnableSqlConfig): boolean {
  return config.expect === 'error';
}
