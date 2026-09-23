---
title: Community extension docs
sidebar_position: 10
description: Why a duckfn extension needs function_descriptions.csv, what the four columns mean, and how to generate and submit it.
---

# Community extension docs

Every community extension gets a page on
[duckdb.org/community_extensions](https://duckdb.org/community_extensions/list_of_extensions) that
is generated from the extension binary: DuckDB loads it, diffs `duckdb_functions()` before and after,
and renders what the extension added.

That last part is the catch — the page shows whatever the catalog reports, and **DuckDB's C
extension API has no way to set a function's description or examples**. It exposes
`duckdb_scalar_function_set_name`, `_set_return_type`, `_set_varargs`, `_set_volatile`,
`_set_special_handling`, `_set_extra_info`, `_set_bind` and `_set_function`, and that is it. There
is no `duckdb_scalar_function_set_description`, no `_add_example`. So a duckfn extension cannot
carry that text into the catalog, and without help the "Added Functions" table on its page is a
list of bare names.

The way out is a CSV. `duckdb/community-extensions`' `scripts/generate_md.sh` looks for
`extensions/<name>/docs/function_descriptions.csv` and, when it is there, LEFT JOINs it on
`function_name == other.function` to override the `description`, `comment` and `examples` columns of
both `functions` and `functions_overloads`. Rows that match nothing are ignored, so the file is
purely an override table.

## Write the text on the attribute

duckfn lets you keep that text next to the function it describes:

```rust
use duckfn::duck_scalar_function;

/// Doubles an INTEGER.
#[duck_scalar_function(
    description = "Doubles an INTEGER",
    comment = "NULL in, NULL out",
    examples = ["SELECT double_it(21)", "SELECT double_it(x) FROM t"]
)]
pub fn double_it(v: Option<i64>) -> Option<i64> {
    v.map(|x| x * 2)
}
```

Three keys, all optional:

| Key | CSV column | Notes |
| --- | --- | --- |
| `description` | `description` | One-line summary. This is what shows up in the table. |
| `comment` | `comment` | Extra remarks, shown alongside the description. |
| `example` / `examples` | `example` | One example (`example = "SELECT ..."`) or several (`examples = ["...", "..."]`). The two are mutually exclusive — writing both is a compile error. |

They work on every attribute that registers a function: `#[duck_scalar_function]`,
`#[duck_aggregate_function]`, `#[duck_table_function]`, `#[duck_cast_function]`,
`#[duck_copy_function]`, `#[duck_copy_from_function]` and `#[duck_sql_macro]`.
`#[duck_replacement_scan]` and `#[duck_custom_register]` do not take them, because nothing named
after the Rust function ends up in the catalog.

Two things worth knowing about the text itself:

- **Line breaks collapse to a single space** when the CSV is written. The generated page is a
  Markdown table, where a newline inside a cell ends the row, and DuckDB's `read_csv()` reads a bare
  newline inside a quoted field back as `\r\n` anyway. Commas, double quotes and non-ASCII text pass
  through unchanged; only newlines are flattened.
- **Curly braces are your problem to write and the generator's to escape.** `generate_md.sh` runs
  both columns through a `jekyll_format_function` macro that wraps every `{{` and `}}` in
  `{% raw %}…{% endraw %}`, so `example = "SELECT f('{{x}}')"` is fine as written.

The attributes do not touch registration — they are collected into an `inventory` entry and only
used when the CSV is exported.

## Generate the CSV

```bash
cargo run --bin duckfn -- function_descriptions          # -> target/function_descriptions.csv
cargo run --bin duckfn -- function_descriptions --all    # -> target/function_descriptions_all.csv
```

or, if you prefer `just`:

```bash
just docs_csv
```

The default export writes one row per function that has a description, comment or example. `--all`
writes every registered function instead, leaving the columns empty for the ones you have not
documented yet — handy as a checklist, and the file name gets an `_all` suffix so the two do not
collide. The path is always `<project>/target/`, fixed, so nothing downstream has to guess where to
find it.

This is a plain in-memory operation over what the macros recorded at compile time: no extension is
loaded, `duckdb_functions()` is never queried, and DuckDB is not involved at all.

### Setting it up in a new project

Two pieces, both copied from the [duckfn repository](https://github.com/shijianjs/duckfn):

1. `Cargo.toml` — turn on duckfn's `cli` feature:

   ```toml
   duckfn = { version = "x.y.z", features = ["duckdb-1-5", "cli"] }
   ```

2. `src/bin/duckfn.rs` — the entry point:

   ```rust
   //! duckfn 命令行工具入口：cargo run --bin duckfn -- function_descriptions
   #[path = "../extension/mod.rs"]
   mod extension;

   fn main() -> std::process::ExitCode {
       duckfn::cli::run(env!("CARGO_MANIFEST_DIR"))
   }
   ```

   The `#[path]` inclusion is deliberate: `#[duck_*]` metadata is collected by `inventory`'s static
   constructors, which only fire for object files that are really linked into the final binary.
   Merely depending on the library lets the linker drop those modules, and the CSV would come out
   empty without any error.

## Submit it

The file is read from the **community-extensions repository**, not from your own: when you open the
pull request that adds `extensions/<name>/description.yml`, add
`extensions/<name>/docs/function_descriptions.csv` next to it. Keeping a copy in your own repo is
handy for regeneration — re-run the command whenever you add a function.
