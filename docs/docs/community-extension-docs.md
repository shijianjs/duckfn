---
title: Community extension docs
sidebar_position: 10
description: Why a duckfn extension needs function_descriptions.csv, what the four columns mean, and how to generate and submit it.
---

# Community extension docs

Every community extension gets a page on <https://duckdb.org/community_extensions/extensions/…>
that is generated from the extension binary: DuckDB loads it, diffs `duckdb_functions()` before and
after, and renders what the extension added.

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

The attributes do not touch registration — they are collected into an `inventory` entry and only
used when you export the CSV.

## Generate the CSV

```bash
just docs_csv                      # -> docs/function_descriptions.csv
just docs_csv_check                # fail if the committed CSV is out of date
```

`just docs_csv` builds the extension, loads it in `duckdb` with
`DUCKFN_DUMP_FUNCTION_DESCRIPTIONS=<path>` set, and duckfn writes the file while it registers. The
`function` column comes from a real before/after diff of `duckdb_functions()`, so it is always the
name the community-extension generator will join on — including functions registered through
`#[duck_custom_register]` or `duck_sql_macro_files!`, none of which need a description of their own.
Functions without one still get a row, with empty text, so the skeleton is complete and you can see
what is left to write.

Without `just`, the same thing is one command:

```bash
DUCKFN_DUMP_FUNCTION_DESCRIPTIONS=docs/function_descriptions.csv \
  duckdb -unsigned -c "LOAD './target/debug/my_ext.duckdb_extension';"
```

## Submit it

The file is read from the **community-extensions repository**, not from your own: when you open the
pull request that adds `extensions/<name>/description.yml`, add
`extensions/<name>/docs/function_descriptions.csv` next to it. Keeping a copy in your own repo is
handy for regeneration — `just docs_csv_check` will then tell you when a new function showed up
without a description.
