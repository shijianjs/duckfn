/**
 * `duckfn-docs-kit/sql/verify` — runs every runnable SQL block a docs site
 * publishes, so a broken example is caught by CI instead of by a reader
 * clicking **Run**.
 *
 * It is the same environment the site gives a block: DuckDB-Wasm in a browser,
 * the site's extension preloaded, one instance per page and the page's blocks
 * sharing a connection (see `sql/browserRunner.ts` for why this is a real
 * browser and not the Node worker the kit used to run).
 *
 * A block may fail *on purpose* — half the guide ends on a statement that
 * demonstrates an error. Such a block says so in its own metadata
 * (`{"type":"duckfn","expect":"error"}`) and is then **required** to fail: one
 * that starts succeeding is reported too, because an expectation the suite acts
 * on has to be data rather than a string match on a comment. Only blocks that
 * did not behave as declared make the command exit non-zero.
 */
import {writeFileSync} from 'node:fs';

import {collectRunnableSql, expectsError, type RunnableSqlBlock} from './collect';
import {BrowserSqlRunner, type WasmPlatform} from './browserRunner';
import {
  resolveDocsSiteConfig,
  resolveExtension,
  type DocsSiteConfig,
  type StaticAssetMount,
} from './site';

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
   * The browser executable that runs the blocks. Defaults to a detected
   * Chrome/Edge; the `DFK_BROWSER` environment variable is the same override.
   */
  browser?: string;
  /**
   * Local directories served over HTTP while the blocks run, so SQL can
   * `read_csv_auto('{{DFK_ORIGIN}}<prefix>/x.tsv')` the site's own data (see
   * `sql/site`).
   */
  assets?: readonly StaticAssetMount[];
  /** Write the full result list here as JSON. */
  reportFile?: string;
}

/** What happened to a block, judged against what it declared. */
export type BlockOutcome =
  /** Ran, and was expected to run. */
  | 'ok'
  /** Failed, and declared `"expect": "error"`. */
  | 'error-as-expected'
  /** Failed, but was expected to run. */
  | 'unexpected-error'
  /** Ran, but declared `"expect": "error"`. */
  | 'unexpected-success';

export interface BlockResult {
  file: string;
  line: number;
  outcome: BlockOutcome;
  detail: string;
}

export interface VerifyReport {
  blocks: BlockResult[];
  /** Blocks that behaved as declared: they ran, or they failed as declared. */
  asDeclared: BlockResult[];
  /** Blocks that did not behave as declared — the ones that should fail CI. */
  unexpected: BlockResult[];
}

/** Runs every runnable block of the site and returns the outcome of each. */
export async function verifySqlDocs(options: VerifyOptions = {}): Promise<VerifyReport> {
  const config = resolveDocsSiteConfig(options);
  const blocks = collectRunnableSql({
    siteDir: config.siteDir,
    contentDirs: config.contentDirs,
  });
  const extension = resolveExtension(config.siteDir, config.extension);

  // No working directory: in a browser DuckDB's file system is the instance's
  // own memory, so `COPY … TO` / `dfn_file_write_*` never touch the docs tree —
  // they land in the page and vanish on the next `newPage()`.
  const results = await runPages(blocks, extension, config, config.timeoutMs);

  const report: VerifyReport = {
    blocks: results,
    asDeclared: results.filter((result) => result.outcome === 'ok' || result.outcome === 'error-as-expected'),
    unexpected: results.filter(
      (result) => result.outcome === 'unexpected-error' || result.outcome === 'unexpected-success',
    ),
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
  config: DocsSiteConfig,
  timeoutMs: number,
): Promise<BlockResult[]> {
  const runner = await BrowserSqlRunner.create({
    extension,
    platform: config.platform,
    engine: config.engine,
    browser: config.browser,
    assets: config.assets,
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

/** Runs one block and judges the result against the block's own declaration. */
async function runBlock(
  runner: BrowserSqlRunner,
  block: RunnableSqlBlock,
  timeoutMs: number,
): Promise<BlockResult> {
  const declaredError = expectsError(block.config);
  try {
    const result = await withTimeout(runner.run(block.sql), timeoutMs);
    return {
      file: block.file,
      line: block.line,
      outcome: declaredError ? 'unexpected-success' : 'ok',
      detail: declaredError
        ? `declared "expect": "error" but succeeded (${result.rows}×${result.columns})`
        : `ok (${result.rows}×${result.columns})`,
    };
  } catch (error) {
    const message = String((error as Error)?.message ?? error).split('\n')[0] ?? '';
    return {
      file: block.file,
      line: block.line,
      outcome: declaredError ? 'error-as-expected' : 'unexpected-error',
      detail: message,
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


export interface CliOptions extends VerifyOptions {
  quiet?: boolean;
}

/**
 * `duckfn-sql-verify` — the command line around {@link verifySqlDocs}.
 *
 * Exit code 1 when a block did not behave as it declared, so a docs site can
 * wire it straight into `npm test`.
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
  const declaredFailures = report.blocks.filter(
    (result) => result.outcome === 'error-as-expected',
  ).length;
  if (!options.quiet) {
    process.stdout.write(`\n${report.blocks.length} block(s) in ${seconds}s\n`);
    process.stdout.write(
      `  ${report.asDeclared.length} as declared (${declaredFailures} erroring on purpose), ` +
        `${report.unexpected.length} unexpected\n`,
    );
  }
  if (report.unexpected.length > 0) {
    process.stdout.write('unexpected behaviour:\n');
    for (const block of report.unexpected) {
      process.stdout.write(`- ${block.file}:${block.line} [${block.outcome}] :: ${block.detail}\n`);
    }
    process.exitCode = 1;
  }
}

const USAGE = `Usage: duckfn-sql-verify [options]

Runs every runnable SQL block of a duckfn docs site in a headless browser
(DuckDB-Wasm), and checks that each one behaves as its own metadata declares
("expect": "error" for a block that demonstrates a failure).

  --site <dir>          Docs site root (default: the working directory)
  --content <dir>       Content directory, relative to the site root (repeatable;
                        default: docs/ plus every i18n/<locale>/… translation)
  --extension <path>    The extension to preload: a .duckdb_extension.wasm path,
                        or an absolute http(s) URL (default: the single file under
                        static/duckdb-extensions/)
  --asset <url=dir>     Serve a local directory over HTTP at a URL prefix while the
                        blocks run, so SQL can read_csv_auto the site's own data
                        (repeatable; e.g. --asset /my-site/data=static/data)
  --platform <eh|mvp>   DuckDB-Wasm bundle, which must match the extension build
                        (default: eh)
  --engine <path>       Engine wasm override
  --browser <path>      Browser executable (default: a detected Chrome/Edge,
                        or the DFK_BROWSER environment variable)
  --timeout <ms>        Per-block timeout (default: 30000)
  --report <file>       Write the full result list as JSON
  --quiet               Only report unexpected behaviour
  --help                Show this help
`;

interface ParsedArgs extends CliOptions {
  help?: boolean;
}

function parseArgs(argv: readonly string[]): ParsedArgs {
  const options: ParsedArgs = {};
  const content: string[] = [];
  const assets: StaticAssetMount[] = [];
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
      case '--asset':
        assets.push(parseAsset(next()));
        break;
      case '--platform':
        options.platform = next() as WasmPlatform;
        break;
      case '--engine':
        options.engine = next();
        break;
      case '--browser':
        options.browser = next();
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
  if (assets.length > 0) {
    options.assets = assets;
  }
  return options;
}

/** `--asset <url=dir>`: the URL prefix and the directory are both required. */
function parseAsset(value: string): StaticAssetMount {
  const equals = value.indexOf('=');
  if (equals <= 0 || equals === value.length - 1) {
    throw new Error(`sql/verify: --asset expects "url=dir" (got ${JSON.stringify(value)})`);
  }
  return {url: value.slice(0, equals), dir: value.slice(equals + 1)};
}
