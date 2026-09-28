#!/usr/bin/env node
/**
 * The `duckfn-sql-verify` executable.
 *
 * A hand-written wrapper rather than a built entry: Vite's library build emits
 * ESM and does not preserve a shebang, and npm only needs one file with one and
 * an executable bit. `dist/` is built by `prepack`, so the import below always
 * resolves in a published tarball.
 */
import {cliMain} from '../dist/sql/verify.js';

await cliMain(process.argv.slice(2));
