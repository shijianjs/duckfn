---
title: Attributes
sidebar_position: 1
description: Every duckfn attribute, the arguments they share, the items they generate, and how auto-registration works.
---

# Attributes

## The attributes

| Attribute | Registers | Accepted return shapes |
| --- | --- | --- |
| `#[duck_scalar_function]` | a scalar function | `T`, `Option<T>`, `DuckOptionResult<T>` |
| `#[duck_aggregate_function]` | an aggregate function | row handler returns `()` or `DuckResult<()>`; the output comes from the state |
| `#[duck_table_function]` | a table function | `impl Iterator<Item = Row>`, `DuckResult<impl Iterator<Item = Row>>`, `DuckFullIteratorResult<Row>` |
| `#[duck_cast_function]` | a type cast | `T`, `Option<T>`, `DuckOptionResult<T>` |
| `#[duck_replacement_scan]` | a replacement scan | `Option<String>`, `Option<&'static str>`, `DuckOptionResult<String>`, `DuckOptionResult<&'static str>` |
| `#[duck_sql_macro]` | a SQL macro | `SqlMacro`, `DuckResult<SqlMacro>`, `String`, `&'static str`, or `DuckResult` of those |
| `#[duck_custom_register]` | whatever the function registers itself | `fn(&Connection) -> DuckResult<()>` |
| `#[derive(DuckStruct)]` | — | maps a struct to a DuckDB `STRUCT` |
| `#[derive(DuckEnum)]` | — | maps a unit-variant enum to a DuckDB `ENUM` (optionally creates the type at load time) |
| `duckfn_entrypoint!("name")` | the extension entry point | — |
| `duck_sql_macro_files!("a.sql", …)` | macros defined in SQL files | — |

Anything outside the listed return shapes is a compile error, with a message naming the shapes that
are supported.

## Arguments

Every function attribute shares one argument list:

| Argument | Default | Meaning |
| --- | --- | --- |
| `auto_register` | `true` | `false` generates the builders but does not register the function. |
| `named_param_from` | — | For table functions: the argument from which on everything is a named parameter. |
| `special_null_handling` | `false` | Ask DuckDB to hand `NULL` arguments to the callback instead of folding them away. See [Scalar functions](./scalar-functions.md#null-handling). |
| `implicit_cost` | — | For casts: the implicit conversion cost. |
| `overloads_name` | — | Register as an overload of this function set instead of under the function's own name. |

`#[derive(DuckStruct)]` accepts the same arguments through its `#[duck(...)]` attribute, which is how
struct-based table functions declare where their named parameters start:

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "start")]
struct CountDownS {
    start: i64,
}
```

`#[derive(DuckEnum)]` maps a unit-variant-only enum onto a DuckDB `ENUM` (dictionary = declaration
order). It has one argument of its own — `rename_all` (`lowercase`, `UPPERCASE`, `snake_case`,
`SCREAMING_SNAKE_CASE`, `camelCase`, `PascalCase`, `kebab-case`, `SCREAMING-KEBAB-CASE`, default
`verbatim`) — plus `#[duck(rename = "...")]` on a single variant:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}
```

The enum is then usable anywhere a value type is expected — function arguments, return values,
`STRUCT` fields, container elements — and `Option<Priority>` makes it nullable. A **non-nullable**
argument additionally needs `Default` (the generated argument struct derives it), which is why the
example derives `Default` and marks a `#[default]` variant; with `Option<Priority>` it is not needed.

### Named types in the catalog

Both derives also accept:

| Argument | Default | Meaning |
| --- | --- | --- |
| `sql_name` | the type name in snake_case | The SQL-side type name (`Priority` → `priority`, `Ticket` → `ticket`). |
| `create_type` | `false` | `true` runs `CREATE TYPE IF NOT EXISTS <sql_name> AS <type>;` when the extension loads, `"print"` only prints that statement (to stderr), `false` does nothing. |

The statement is idempotent — loading the extension twice is fine — and leaves an existing type of
that name untouched. It goes through the same execution path as the SQL macros (`duckdb_query`). An
enum becomes `ENUM(...)`; a struct becomes `STRUCT(...)`, and its field types are rendered from
DuckDB's *own* logical types, so nested enums and structs, `LIST` / `ARRAY` / `MAP` and hand-written
custom field types come along automatically:

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = true)]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,
    pub labels: Vec<String>,
}
// CREATE TYPE IF NOT EXISTS "ticket" AS
//   STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'), "labels" VARCHAR[]);
```

With `create_type = "print"` the macro renders that very same statement but creates nothing. The DDL
is queued and printed **once**, after every registration has run, so an extension with a dozen
print-mode types still gets a single notice instead of a dozen hints. The block is framed by
`-- [duckfn]` comments saying it was *not* executed, which keeps it copy-pasteable as SQL and makes a
bare DDL line impossible to mistake for something that actually happened:

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = "print")]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,
}
```

```
-- [duckfn] create_type = "print": the statement below was NOT executed.
-- [duckfn] Copy it and run it yourself if you want the type created.
CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'));
-- [duckfn] end - nothing above was executed.
```

Keep `create_type = false` (the default) when the macro should stay out of the way entirely, and
print the DDL yourself — from `#[duck_custom_register]`, with wording of your own:

```rust
#[duck_custom_register]
fn show_the_create_type_ddl(_connection: &Connection) -> DuckResult<()> {
    // `create_type = false`: showing the DDL — and what it means — is up to you
    let ddl = duckfn::named_type_ddl("ticket", &Ticket::logical_type())?;
    duckfn::print_sql_preview(
        "my extension will NOT create this type",
        &ddl,
        "end - copy the statement above and run it yourself if you want it",
    );
    Ok(())
}
```

An enum works the same way — `duckfn::named_type_ddl("priority", &Priority::logical_type())` (its
`logical_type()` already carries the dictionary), or `duckfn::register_enum_type` /
`duckfn::queue_enum_type_ddl` for a hand-written label list.

SQL can then use `ticket` as a type — a column type or a cast target — even though the functions are
registered with the equivalent structural type; the two are interchangeable. See
[Types → Enums](./types.md#enums) and [Types → Structs](./types.md#structs).

## What a macro generates

A function attribute expands to the original function plus a module **named after that function**:

```rust
#[duck_scalar_function]
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}
```

becomes approximately:

```rust
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}

mod dfn_scalar_reg_manual {
    use super::*;

    // One field per argument; the derive maps the struct to a STRUCT.
    #[derive(duckfn::DuckStruct, Debug, Clone, Default)]
    pub struct DuckArgsImpl {
        pub i: i32,
    }

    pub struct ScalarFunctionImpl;
    impl duckfn::ScalarFunctionAdapter for ScalarFunctionImpl {
        const NAME: &'static str = "dfn_scalar_reg_manual";
        type Args = DuckArgsImpl;
        type Output = i32;
        // …
    }

    pub fn scalar_function_builder() -> quack_rs::prelude::ScalarFunctionBuilder { /* … */ }
    pub fn scalar_overload_builder() -> quack_rs::prelude::ScalarOverloadBuilder { /* … */ }
}
```

The module inherits the function's visibility, so a `pub fn` gets a `pub mod`. Inside it, each macro
provides:

| Macro | Generated items |
| --- | --- |
| `#[duck_scalar_function]` | `ScalarFunctionImpl`, `scalar_function_builder()`, `scalar_overload_builder()` |
| `#[duck_aggregate_function]` | `AggregateFunctionImpl`, `aggregate_function_builder()`, `aggregate_overload_builder(builder)`, `aggregate_function_guard()` |
| `#[duck_table_function]` | `TableFunctionImpl`, `table_function_builder()` (returns a `DuckResult`) |
| `#[duck_cast_function]` | `CastFunctionImpl`, `cast_function_builder()`, `cast_function_register(connection)` |
| `#[duck_replacement_scan]` | `ReplacementScanImpl`, `replacement_scan_register(connection)` — no `DuckArgsImpl` |

`#[duck_custom_register]` and `#[duck_sql_macro]` generate no module at all: they keep the function
as written and submit it to the registry.

## Automatic vs. manual registration

With the default `auto_register = true` the macro submits itself to an `inventory` registry, and the
entry point generated by `duckfn_entrypoint!` registers everything it finds when DuckDB loads the
extension. There is no registration boilerplate to write.

With `auto_register = false` the function is generated but never registered, which makes it
invisible to SQL until you register it yourself:

```rust
#[duck_scalar_function(auto_register = false)]
fn dfn_scalar_reg_manual(i: i32) -> i32 {
    i + 1
}

#[duck_custom_register]
fn dfn_scalar_reg_manual_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_scalar(dfn_scalar_reg_manual::scalar_function_builder()) }
}
```

`#[duck_custom_register]` takes the function as-is, so its signature must be exactly
`fn(&Connection) -> DuckResult<()>`. Registering the other kinds follows the same pattern:

```rust
unsafe { c.register_aggregate(dfn_agg_reg_manual::aggregate_function_builder()) }  // aggregate
unsafe { c.register_table(dfn_table_reg_manual::table_function_builder()?) }      // table
dfn_cast_manual::cast_function_register(c)                                        // cast
dfn_scan_manual::replacement_scan_register(c)                                     // replacement scan
```

Note that the table builder returns a `DuckResult`, so it is `?`-ed; the cast and scan helpers take
the connection directly.

Manual registration is also how you assemble a function set by hand:

```rust
#[duck_custom_register]
fn dfn_scalar_reg_over_register(c: &Connection) -> DuckResult<()> {
    unsafe {
        c.register_scalar_set(
            ScalarFunctionSetBuilder::new("dfn_scalar_reg_overload")
                .overload(dfn_scalar_reg_over_int::scalar_overload_builder())
                .overload(dfn_scalar_reg_over_varchar::scalar_overload_builder()),
        )
    }
}
```

`ScalarFunctionSetBuilder`, `Connection` and the register methods come from `quack-rs`; see
[Installation](../getting-started/installation.md) for why you add that dependency yourself.

## The entry point

```rust
duckfn_entrypoint!("my_ext");
```

This generates the symbol `my_ext_init_c_api`, which is what DuckDB looks up when loading the
extension. The name must be non-empty and contain only lowercase ASCII letters, digits and
underscores; anything else is rejected at compile time, and a wrong symbol name makes the extension
fail to load.

## SQL macros from files

`duck_sql_macro_files!` registers macros kept in `.sql` files instead of Rust string literals:

```rust
duck_sql_macro_files!(
    "sql/macro_files_a.sql",
    "sql/macro_files_b.sql",
    "sql/macro_files_c.sql"
);
```

Paths are resolved relative to the `.rs` file that invokes the macro, inlined with `include_str!`
at compile time, and executed in the order written. A single file may define any number of macros;
see [SQL macros](./sql-macros.md).

## Source and tests

- [`duckfn-macro/src/attr_args.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/src/attr_args.rs) — the argument list every attribute shares
- [`duckfn-macro/src/duck_function.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-macro/src/duck_function.rs) — what each macro expands to
- [`src/extension/functions/scalar_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/scalar_function.rs) and [`test/sql/functions/scalar_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/scalar_function.test) — the manual-registration examples
