---
title: Architecture
sidebar_position: 1
description: How duckfn turns an annotated function into a registered DuckDB function — expansion, inventory registration, entry point, adapters and value types.
---

# Architecture

This page follows one annotated function from source to a registered DuckDB function.

:::info Diagrams and a deeper walkthrough
[Zread](https://zread.ai/shijianjs/duckfn) documents this repository with generated diagrams — module
layering, the registration flow and much more. When this page is not enough, start there.
:::

## The pieces

| Crate | Responsibility |
| --- | --- |
| `duckfn-macro` | Procedural macros. Reads the attributes, validates signatures, and emits the wrapper plus a registration item. |
| `duckfn` | Runtime: adapter traits, value types, `inventory` registries, entry point glue. |
| `quack-rs` | The DuckDB C API bindings: builders, `LogicalType`, `DataChunk`, `VectorReader`/`VectorWriter`, `SqlMacro`, `ExtensionError`. |
| `libduckdb-sys` | DuckDB's C headers, compiled with the `loadable-extension` feature. |

`duckfn`'s modules are all `pub(crate)`; the public surface is the set of `pub use` re-exports in
`duckfn/src/lib.rs`, which is why `duckfn::DuckOptionResult` exists but `duckfn::ExtensionError` does
not.

## 1. Expansion

An attribute macro runs `common_build`, which keeps the original function and adds a module named
after it:

```
#item_fn                     // the function, untouched
#vis mod #name {             // visibility inherited from the function
    use super::*;
    #[derive(duckfn::DuckStruct, Debug, Clone, Default)]
    #[duck(#attr)]           // the attribute arguments are forwarded
    pub struct DuckArgsImpl { /* one field per argument */ }
    // macro-specific items: the adapter impl and the builders
}
```

`DuckArgsImpl` is the bridge between the argument list and the two directions data flows:
`#[derive(DuckStruct)]` turns it into a `DuckColumns` type, so the same struct is used to read
arguments from a chunk and to write them back for struct arguments. The adapter impl then calls the
original function. See [Attributes](../guide/attributes.md) for the items each macro emits.

## 2. Registration

The macro submits a registration item to an `inventory` registry, which collects items at link time:

```rust
pub type DuckRegisterFn = fn(connection: &Connection) -> DuckResult<()>;

pub struct DuckFunctionItem {
    pub register_fn: DuckRegisterFn,
}

inventory::collect!(DuckFunctionItem);
```

There are three registries, one per registration strategy:

| Registry | Collected from | Registered by |
| --- | --- | --- |
| `DuckFunctionItem` | Most macros, and every `#[duck_custom_register]` function | `register_all_duckfn` |
| `DuckAggregateOverloadItem` | `overloads_name` on aggregates | `register_all_aggregate_overload` |
| `DuckScalarOverloadItem` | `overloads_name` on scalars | `register_all_scalar_overload` |

`register_all_duckfn` applies them in that order, and the overload passes group items by name with
`itertools::into_grouping_map_by` so that every signature sharing an `overloads_name` ends up in one
function set. Registry `name` fields are `&'static str` rather than `String` because
`inventory::submit!` expands into a `static` initializer, where a `String` cannot be constructed.

## 3. Entry point

```rust
duckfn_entrypoint!("my_ext");
```

expands to

```rust
quack_rs::entry_point_v2!(my_ext_init_c_api, duckfn::register_all_duckfn);
```

so the exported symbol is `{name}_init_c_api`, and the body is a call to `register_all_duckfn` with
the `Connection` DuckDB hands over. The name is validated at compile time: it must be non-empty and
contain only lowercase ASCII letters, digits and underscores.

## 4. Dispatch

With the `loadable-extension` feature, DuckDB API functions are not linked. Instead they are resolved
through an `AtomicPtr` table that DuckDB fills in when it loads the extension and calls the entry
point:

```
LOAD 'my_ext.duckdb_extension'
  -> DuckDB calls my_ext_init_c_api(connection)
     -> quack-rs installs the API table
        -> register_all_duckfn(connection) registers every collected item
```

This is why the extension does not need a DuckDB build, and why it is tied to the DuckDB version it
was compiled against.

## 5. The adapters

Each registration kind has an adapter trait that turns Rust values into DuckDB's vector-based callbacks.

### Scalar

Reads one row at a time out of the input chunk, calls `apply_with_null`, writes the results as a
batch. `apply_with_null` returns `Ok(None)` when any non-`Option` argument is `NULL`, which is what
short-circuits the row. `null_handling()` defaults to `DefaultNullHandling` and is overridden to
`SpecialNullHandling` by `special_null_handling = true`.

### Aggregate

DuckDB's six callbacks are implemented on the state type:

| Callback | Behaviour |
| --- | --- |
| `c_state_size` / `c_state_init` | Allocate and initialise the state (`Default`). |
| `c_update` | Read a row and call `handle_row_with_null`; `NULL` rows are skipped by default. |
| `c_combine` | Merge a partial state into another one, for parallel aggregation. |
| `c_finalize` | Call `result()` per state and write the vector. A non-zero `offset` is rejected. |
| `c_state_destroy` | Drop the states. |

Aggregate functions and sets are wrapped in RAII guards (`AggregateFunctionGuard`,
`AggregateFunctionSetGuard`) so the DuckDB objects are destroyed even when registration fails midway.
`duckfn::DuckfnAggregateFunctionSetBuilder` exists because `quack-rs`'s set builder can only set one
return type per set, while each duckfn overload has its own `Output`.

### Table

`with_state` is the bind step: it reads arguments, decides the result columns through
`config_result_columns`, and returns the row iterator. `scan` pulls from that iterator once per chunk.
Both are wrapped in `catch_unwind`, so a panic in either becomes a query error. The iterator type is
`DuckFullIterator<T> = Box<dyn Iterator<Item = DuckOptionResult<T>> + Send>`.

### Cast

The wrapper receives a `count`, an input vector and an output vector, and calls the function per row.
Errors are handled according to the cast mode: `CastMode::Normal` (`CAST`) fails the query, while
`CastMode::Try` (`TRY_CAST`) records a row error and writes `NULL`.

### Replacement scan

`scan_callback` receives the unresolved table name. `handle_info` calls the user's `handle_path`, and
on `Some(table_fn)` it sets the function to call and adds the path as the first VARCHAR parameter.
Non-UTF-8 names are skipped, and `Err` is reported through `duckdb_replacement_scan_set_error`.

### SQL macro

The simplest adapter: the returned `SqlMacro` is registered, or a returned string is executed with
`duckdb_query`. That is why a string may contain several statements.

## 6. Value types

`DuckValueType` is the trait that makes a Rust type usable as an argument or result:

| Direction | Methods |
| --- | --- |
| Type identity | `type_id()`, `logical_type()` |
| Reading | `create_reader`, `read_valid`, `read_by_duck_value*` |
| Writing | `write_batch`, `write_valid`, `write_null`, `write_finish` |

Implementors override the `*_valid` half; `read` and `write` are not meant to be overridden.
`DuckValueReader` and `DuckValueWriter` carry the vector plus child readers/writers, which is how
nested types recurse into LIST, MAP, ARRAY and STRUCT children. `#[derive(DuckStruct)]` additionally
generates `assert_impl_duck_value_type::<T>()` calls for every field, so an unsupported field type is
a compile error rather than a runtime surprise.

`DuckStructTrait` is the generated struct interface, and three blanket impls connect it to the rest of
the system: `DuckValueType` (usable as a value), `DuckColumns` (usable as a table function's output
row), and `DuckBindArgs` (usable as a table function's arguments).

## Where to look

| Question | File |
| --- | --- |
| Which arguments does an attribute accept? | `duckfn-macro/src/attr_args.rs` |
| What does a macro emit? | `duckfn-macro/src/duck_function.rs`, `duck_struct_derive.rs` |
| How is the entry point generated? | `duckfn-macro/src/entrypoint.rs` |
| How does registration work? | `duckfn/src/register.rs` |
| How is a callback implemented? | `duckfn/src/functions/*_adapter.rs` |
| How is a type converted? | `duckfn/src/value_types/*.rs` |

## Next

- [Attributes](../guide/attributes.md) — the user-facing view of expansion.
- [Contributing](../contributing.md) — working on these crates.
