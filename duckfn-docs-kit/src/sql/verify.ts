/**
 * `duckfn-docs-kit/sql/verify` — runs every runnable SQL block a docs site
 * publishes, so a broken example is caught by CI instead of by a reader
 * clicking **Run**.
 *
 * It is the same environment the site gives a block: DuckDB-Wasm in a worker,
 * the site's extension preloaded, one instance per page and the page's blocks
 * sharing a connection (see `sql/nodeRunner.ts` for why it is this target and
 * not the blocking one).
 *
 * A block may fail *on purpose* — half the guide ends on a statement that
 * demonstrates an error. The metadata cannot say so, so the convention is a
 * comment on that statement (`-- error: …`, `-- 报错：…`); `expectsError()`
 * recognises it and those blocks are reported separately. Only unexpected
 * failures make the command fail.
 */
import {mkdirSync, mkdtempSync, readdirSync, rmSync, statSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';

import {collectRunnableSql, expectsError, type RunnableSqlBlock} from './collect';
import {WasmSqlRunner, type WasmPlatform} from './nodeRunner';

export interface VerifyOptions {
  /** Docs site root; defaults to the working directory. */
  siteDir?: string;
  /** Content directories relative to the site root; defaults to the site layout. */
  contentDirs?: readonly string[];
  /**
   * The extension to `LOAD`: a path, or an absolute `http(s)` URL. Defaults to
   * the single file under `<siteDir>/static/duckdb-extensions/`.
   */
  extension?: string;
  /** DuckDB-Wasm platform, which must match the extension build. */
  platform?: WasmPlatform;
  /** Engine wasm override, for pinning a specific DuckDB-Wasm build. */
  engine?: string;
  /** Per-block timeout in milliseconds; a hang is reported instead of blocking CI. */
  timeoutMs?: number;
  /**
   * Directory the blocks run in. Defaults to a fresh temporary directory that
   * is removed afterwards: a block may `COPY … TO 'a.csv'`, and on Node
   * DuckDB's file system is the real one, relative to the working directory.
   */
  workingDir?: string;
  /** Write the full result list here as JSON. */
  reportFile?: string;
}

export interface BlockResult {
  file: string;
  line: number;
  ok: boolean;
  /** The block failed, but documents itself as failing. */
  expected: boolean;
  detail: string;
}

export interface VerifyReport {
  blocks: BlockResult[];
  /** Every block that failed, expected or not. */
  failures: BlockResult[];
  /** Failures that are not documented as such — the ones that should fail CI. */
  unexpected: BlockResult[];
}

const DEFAULT_CONTENT = ['docs', 'i18n'] as const;
const DEFAULT_TIMEOUT_MS = 30_000;

/** Runs every runnable block of the site and returns the outcome of each. */
export async function verifySqlDocs(options: VerifyOptions = {}): Promise<VerifyReport> {
  // Everything is resolved to absolute paths first: the run changes the working
  // directory (see below).
  const siteDir = resolve(options.siteDir ?? process.cwd());
  const blocks = collectRunnableSql({
    siteDir,
    contentDirs: options.contentDirs ?? defaultContentDirs(siteDir),
  });
  const requested = options.extension;
  const extension = requested
    ? /^https?:\/\//i.test(requested)
      ? requested
      : resolve(requested)
    : defaultExtension(siteDir);
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;

  // A block may write files — `COPY (SELECT 1) TO 'a.csv'` — and on Node the
  // file system behind DuckDB is the real one, resolved against the working
  // directory. Giving the run a scratch directory of its own keeps the docs tree
  // clean instead of dropping test residue into it.
  const scratch = options.workingDir
    ? resolve(options.workingDir)
    : mkdtempSync(join(tmpdir(), 'duckfn-sql-verify-'));
  mkdirSync(scratch, {recursive: true});
  const previousCwd = process.cwd();
  process.chdir(scratch);

  let results: BlockResult[];
  try {
    results = await runPages(blocks, extension, options, timeoutMs);
  } finally {
    process.chdir(previousCwd);
    if (!options.workingDir) {
      rmSync(scratch, {recursive: true, force: true});
    }
  }

  const failures = results.filter((result) => !result.ok);
  const report: VerifyReport = {
    blocks: results,
    failures,
    unexpected: failures.filter((result) => !result.expected),
  };
  if (options.reportFile) {
    writeFileSync(options.reportFile, `${JSON.stringify(report, null, 2)}\n`);
  }
  return report;
}

/** Opens one page per content file and runs that file's blocks against it. */
async function runPages(
  blocks: readonly RunnableSqlBlock[],
  extension: string,
  options: VerifyOptions,
  timeoutMs: number,
): Promise<BlockResult[]> {
  const runner = await WasmSqlRunner.create({
    extension,
    platform: options.platform,
    engine: options.engine,
  });
  const results: BlockResult[] = [];
  try {
    let currentFile: string | null = null;
    for (const block of blocks) {
      if (block.file !== currentFile) {
        currentFile = block.file;
        await runner.newPage();
      }
      results.push(await runBlock(runner, block, timeoutMs));
    }
  } finally {
    await runner.close();
  }
  return results;
}

async function runBlock(
  runner: WasmSqlRunner,
  block: RunnableSqlBlock,
  timeoutMs: number,
): Promise<BlockResult> {
  try {
    const result = await withTimeout(runner.run(block.sql), timeoutMs);
    return {
      file: block.file,
      line: block.line,
      ok: true,
      expected: false,
      detail: `ok (${result.rows}×${result.columns})`,
    };
  } catch (error) {
    return {
      file: block.file,
      line: block.line,
      ok: false,
      expected: expectsError(block.sql),
      detail: String((error as Error)?.message ?? error).split('\n')[0] ?? '',
    };
  }
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<never>((_resolve, reject) => {
    timer = setTimeout(() => reject(new Error(`timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

/**
 * Where a Docusaurus site keeps its pages: the English sources in `docs/`, and
 * each translation under `i18n/<locale>/docusaurus-plugin-content-docs/current/`.
 */
function defaultContentDirs(siteDir: string): string[] {
  const dirs: string[] = [];
  if (isDirectory(join(siteDir, DEFAULT_CONTENT[0]))) {
    dirs.push(DEFAULT_CONTENT[0]);
  }
  const i18n = join(siteDir, DEFAULT_CONTENT[1]);
  if (isDirectory(i18n)) {
    for (const locale of readdirSync(i18n)) {
      const translated = join(i18n, locale, 'docusaurus-plugin-content-docs', 'current');
      if (isDirectory(translated)) {
        dirs.push(join('i18n', locale, 'docusaurus-plugin-content-docs', 'current'));
      }
    }
  }
  return dirs.length > 0 ? dirs : ['.'];
}

/** The site's preloaded extension, as `dfkExtensions` places it under `static/`. */
function defaultExtension(siteDir: string): string {
  const dir = join(siteDir, 'static', 'duckdb-extensions');
  const candidates = isDirectory(dir)
    ? readdirSync(dir).filter((name) => name.endsWith('.duckdb_extension.wasm'))
    : [];
  if (candidates.length === 0) {
    throw new Error(
      `sql/verify: no extension found in ${dir} — pass --extension <file|url>, or let the site's ` +
        `extension preload plugin fetch it first`,
    );
  }
  if (candidates.length > 1) {
    throw new Error(
      `sql/verify: several extensions found in ${dir} (${candidates.join(', ')}) — ` +
        `pass --extension to pick one`,
    );
  }
  return join(dir, candidates[0] as string);
}

function isDirectory(path: string): boolean {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

export interface CliOptions extends VerifyOptions {
  quiet?: boolean;
}

/**
 * `duckfn-sql-verify` — the command line around {@link verifySqlDocs}.
 *
 * Exit code 1 when a block that was not documented as failing fails, so a docs
 * site can wire it straight into `npm test`.
 */
export async function cliMain(argv: readonly string[]): Promise<void> {
  const options = parseArgs(argv);
  if (options.help) {
    process.stdout.write(USAGE);
    return;
  }
  const started = Date.now();
  const report = await verifySqlDocs(options);
  const seconds = ((Date.now() - started) / 1000).toFixed(1);
  if (!options.quiet) {
    process.stdout.write(`\n${report.blocks.length} block(s) in ${seconds}s\n`);
    process.stdout.write(
      `  ${report.failures.length} failing: ` +
        `${report.failures.length - report.unexpected.length} documented, ` +
        `${report.unexpected.length} unexpected\n`,
    );
  }
  if (report.unexpected.length > 0) {
    process.stdout.write('unexpected failures:\n');
    for (const failure of report.unexpected) {
      process.stdout.write(`- ${failure.file}:${failure.line} :: ${failure.detail}\n`);
    }
    process.exitCode = 1;
  }
  if (report.failures.length > report.unexpected.length && !options.quiet) {
    process.stdout.write('documented failures:\n');
    for (const failure of report.failures.filter((result) => result.expected)) {
      process.stdout.write(`- ${failure.file}:${failure.line} :: ${failure.detail}\n`);
    }
  }
}

const USAGE = `Usage: duckfn-sql-verify [options]

Runs every runnable SQL block of a duckfn docs site in DuckDB-Wasm.

  --site <dir>          Docs site root (default: the working directory)
  --content <dir>       Content directory, relative to the site root (repeatable;
                        default: docs/ plus every i18n/<locale>/… translation)
  --extension <path>    The extension to preload: a .duckdb_extension.wasm path,
                        or an absolute http(s) URL (default: the single file under
                        static/duckdb-extensions/)
  --platform <eh|mvp>   DuckDB-Wasm bundle, which must match the extension build
                        (default: eh)
  --engine <path>       Engine wasm override
  --timeout <ms>        Per-block timeout (default: 30000)
  --report <file>       Write the full result list as JSON
  --quiet               Only report unexpected failures
  --help                Show this help
`;

interface ParsedArgs extends CliOptions {
  help?: boolean;
}

function parseArgs(argv: readonly string[]): ParsedArgs {
  const options: ParsedArgs = {};
  const content: string[] = [];
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    const next = (): string => {
      const value = argv[++index];
      if (value === undefined) {
        throw new Error(`sql/verify: ${arg} needs a value`);
      }
      return value;
    };
    switch (arg) {
      case '--site':
        options.siteDir = next();
        break;
      case '--content':
        content.push(next());
        break;
      case '--extension':
        options.extension = next();
        break;
      case '--platform':
        options.platform = next() as WasmPlatform;
        break;
      case '--engine':
        options.engine = next();
        break;
      case '--timeout':
        options.timeoutMs = Number(next());
        break;
      case '--report':
        options.reportFile = next();
        break;
      case '--quiet':
        options.quiet = true;
        break;
      case '--help':
      case '-h':
        options.help = true;
        break;
      default:
        throw new Error(`sql/verify: unknown option ${arg}`);
    }
  }
  if (content.length > 0) {
    options.contentDirs = content;
  }
  return options;
}
