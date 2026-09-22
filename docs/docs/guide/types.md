---
title: Type mapping
sidebar_position: 9
description: How DuckDB types map to Rust types, including LIST, MAP, ARRAY and STRUCT, plus the nullability rules and the gaps.
---

# Type mapping

An argument or return type is written as the Rust type you want; the DuckDB type follows from it.

Nullability is expressed by that type and nowhere else: write `T` for `NOT NULL` and `Option<T>` for
nullable. `Option<T>` is a value type of its own — it maps to exactly the same DuckDB type as `T` —
so one rule covers everything: element types (`Vec<Option<i32>>`, `[Option<i32>; 3]`,
`IndexMap<String, Option<i32>>`), struct fields, function arguments and return types.

A second `Option` layer is allowed and changes nothing: `Option<Option<T>>` behaves exactly like
`Option<T>` — same DuckDB type, a `NULL` reads back as the outer `None`, and both `None` and
`Some(None)` write `NULL`. That is deliberate, because wrapping code often cannot strip the middle
type (what it wraps may already be an `Option`) and would otherwise need a branch just for that.

## Simple types

| DuckDB | Rust |
| --- | --- |
| `BOOLEAN` | `bool` |
| `TINYINT` / `SMALLINT` / `INTEGER` / `BIGINT` | `i8` / `i16` / `i32` / `i64` |
| `HUGEINT` | `i128` |
| `UTINYINT` / `USMALLINT` / `UINTEGER` / `UBIGINT` | `u8` / `u16` / `u32` / `u64` |
| `UHUGEINT` | `u128` |
| `FLOAT` / `DOUBLE` | `f32` / `f64` |
| `VARCHAR` | `String` |
| `NULL` | `Option<T>` |

```sql
SELECT dfn_echo_integer(42);                             -- 42
SELECT typeof(dfn_echo_integer(42));                     -- INTEGER
SELECT dfn_echo_uinteger(4294967295::UINTEGER);          -- 4294967295
SELECT dfn_echo_hugeint(9223372036854775808::HUGEINT);   -- 9223372036854775808
SELECT dfn_echo_varchar('你好 🦆');                       -- 你好 🦆
```

## Wrapper types

Types that share a physical representation but mean different things get a wrapper:

| DuckDB | Rust | Field |
| --- | --- | --- |
| `TIMESTAMP` | `DuckTimestamp` | `micros_since_epoch: i64` |
| `TIMESTAMPTZ` | `DuckTimestampTz` | `micros_since_epoch: i64` |
| `TIMESTAMP_S` / `TIMESTAMP_MS` / `TIMESTAMP_NS` | `DuckTimestampS` / `DuckTimestampMs` / `DuckTimestampNs` | `seconds_since_epoch` / `millis_since_epoch` / `nanos_since_epoch` |
| `TIME` | `DuckTime` | `micros_since_midnight: i64` |
| `TIME_NS` | `DuckTimeNs` | `nanos_since_midnight: i64` |
| `TIMETZ` | `DuckTimeTz` | `bits: u64` |
| `DATE` | `DuckDate` | `days_since_epoch: i32` |
| `DECIMAL(W, S)` | `DuckDecimal<W, S>` | `unscaled: i128` |
| `BLOB` | `DuckBlob` | `value: Vec<u8>` |
| `UUID` | `DuckUuid` | `value: u128` |
| `INTERVAL` | `DuckInterval` | from `quack-rs` |

`DuckDecimal` carries width and scale in the type, so it maps back to the right `DECIMAL`:

```rust
#[duck_scalar_function]
fn dfn_echo_decimal(i: DuckDecimal<18, 3>) -> DuckDecimal<18, 3> {
    i
}
```

```sql
SET TimeZone = 'UTC';  -- keep TIMESTAMPTZ output stable
SELECT dfn_echo_date(DATE '2024-01-02');                        -- 2024-01-02
SELECT typeof(dfn_echo_decimal(1.234::DECIMAL(18,3)));          -- DECIMAL(18,3)
SELECT CAST(dfn_echo_time_ns(TIME_NS '03:04:05.123456789') AS VARCHAR);  -- 03:04:05.123456789
SELECT CAST(dfn_echo_uuid('00000000-0000-0000-0000-000000000001'::UUID) AS VARCHAR);
```

`TIME_NS` was added in DuckDB 1.5, so it needs the
[`duckdb-1-5` feature](../getting-started/installation.md#cargo-features) enabled.

### Conversions with `chrono`

A wrapper type holds a raw scalar plus a unit and nothing else — plain `TIMESTAMP` is microseconds
since the epoch, `TIMESTAMP_MS` is milliseconds — so the epoch arithmetic needed to turn one into a
readable, computable date-time is on the caller. The `chrono` feature adds that as a pair of methods
per time type:

| Direction | Methods |
| --- | --- |
| `DuckDate` ↔ `chrono::NaiveDate` | `to_naive_date` / `from_naive_date` |
| `DuckTimestamp` / `DuckTimestampS` / `DuckTimestampMs` / `DuckTimestampNs` ↔ `chrono::NaiveDateTime` | `to_naive_datetime` / `from_naive_datetime` |
| `DuckTimestampTz` ↔ `chrono::DateTime<Utc>` | `to_datetime_utc` / `from_datetime_utc` |
| `DuckTime` / `DuckTimeNs` ↔ `chrono::NaiveTime` | `to_naive_time` / `from_naive_time` |

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["chrono"] }
# duckfn does not re-export chrono, so add it for the types you name. Core types plus `std` is
# enough; `clock` (system time) is only needed if you call `Utc::now()` yourself.
chrono = { version = "0.4", default-features = false, features = ["std"] }
```

```rust
use chrono::{Days, NaiveDate};

#[duck_scalar_function]
fn dfn_date_add(d: DuckDate, days: i64) -> DuckOptionResult<DuckDate> {
    let date = d.to_naive_date()?;                                   // DATE -> NaiveDate
    let shifted = date.checked_add_days(Days::new(days.unsigned_abs()))
        .ok_or_else(|| duck_error("dfn_date_add: out of chrono's range"))?;
    Ok(Some(DuckDate::from_naive_date(shifted)?))                    // NaiveDate -> DATE
}
```

Two rules the conversions follow:

- **They never panic.** `DATE` is an `i32` day count and `TIMESTAMP` an `i64` microsecond count, both
  far wider than what chrono can represent, and DuckDB also carries the sentinels `infinity` /
  `-infinity` (`DATE` stores them as `±i32::MAX`, the `TIMESTAMP` family as `±i64::MAX`). Anything
  chrono cannot hold comes back as a `DuckResult` error rather than being saturated or wrapped.
- **They own the unit conversion**, so which precision a field name stands for never has to be
  guessed: `TIMESTAMP_S` / `TIMESTAMP_MS` / `TIMESTAMP_NS` are seconds / milliseconds / nanoseconds,
  and `TIMESTAMP` / `TIMESTAMPTZ` are both microseconds (they share the same `i64` storage). Going
  back into a coarser unit (`from_naive_datetime` into `TIMESTAMP_S`, or into `TIME`) truncates
  toward zero, the way DuckDB's own casts do.

`DuckDate` ↔ `NaiveDate` is the pair that comes up most often, because a date is what SQL hands you
and a calendar is what the calculation needs.

## Lists

`LIST(T)` is a `Vec<T>`. Element nullability is part of the Rust type:

| Rust | DuckDB | A `NULL` element means |
| --- | --- | --- |
| `Vec<T>` | `LIST(T) NOT NULL` | The whole row becomes `NULL`. |
| `Vec<Option<T>>` | `LIST(T)` | The element stays `NULL`. |

`DuckList<T>` is just an alias for `Vec<T>` — it exists only so that LIST / ARRAY / MAP share the
`Duck*` naming, and there is no dedicated wrapper type to look for. The element type carries the
nullability (write `Vec<Option<T>>` for nullable elements), which is why `Vec<T>` needs a single
implementation: a `NULL` element becomes `None` when the element type can hold it, and turns the
whole row `NULL` when it cannot.

```rust
#[duck_scalar_function]
fn dfn_echo_list_integer_n(i: Vec<Option<i32>>) -> Vec<Option<i32>> {
    i
}
```

```sql
SELECT CAST(dfn_echo_list_integer([1, 2, 3]) AS VARCHAR);     -- [1, 2, 3]
SELECT dfn_echo_list_integer([1, NULL, 3]);                   -- NULL
SELECT CAST(dfn_echo_list_integer_n([1, NULL, 3]) AS VARCHAR);-- [1, NULL, 3]
```

Lists nest to any depth: `Vec<Vec<i32>>`, `Vec<Option<Vec<Option<i32>>>>`.

## Maps

`MAP(K, V)` is an [`IndexMap`](https://docs.rs/indexmap), which keeps insertion order:

| Rust | Value may be `NULL` |
| --- | --- |
| `IndexMap<K, V>` | No — a `NULL` value is an error. |
| `IndexMap<K, Option<V>>` | Yes. |

`DuckMap<K, V>` is an alias for `IndexMap<K, V>`, again sharing the `Duck*` naming — a `NULL` value
is carried by the value type, so nullable values are written `IndexMap<K, Option<V>>`.

Keys may never be `NULL`, and a `MAP` argument is the way to pass key/value data into a table
function:

```sql
SELECT CAST(dfn_echo_map_varchar_integer(MAP {'a': 1, 'b': 2}) AS VARCHAR);  -- {a=1, b=2}
SELECT * FROM dfn_table_from_map(MAP {'a': 1, 'b': 2});                      -- a 1 / b 2
```

## Arrays

`ARRAY(T, N)` is a fixed-size array, and the length lives in the Rust type:

| Rust | DuckDB |
| --- | --- |
| `DuckArray<T, N>` (an alias for `[T; N]`) | `T[N]`, elements not nullable |
| `[Option<T>; N]` | `T[N]`, elements nullable |

```rust
#[duck_scalar_function]
fn dfn_echo_array_integer(i: DuckArray<i32, 3>) -> DuckArray<i32, 3> {
    i
}
```

```sql
SELECT typeof(dfn_echo_array_integer([1, 2, 3]));  -- INTEGER[3]
SELECT dfn_echo_array_integer([1, 2, 3]);          -- [1, 2, 3]
SELECT dfn_echo_array_integer([1, 2]);             -- error: No function matches
```

:::warning[Arrays are not bind parameters]
DuckDB cannot bind a `Value` to an `ARRAY` argument, so an array type cannot appear in a table
function's signature. Arrays are fine as scalar function arguments, as list elements, and as struct
fields.
:::

## Structs

`STRUCT` is a Rust struct with `#[derive(DuckStruct)]`; field names become column names and field
types become the column types:

```rust
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct DuckStructInner {
    key: String,
    value: i32,
}

#[derive(Debug, Clone, Default, DuckStruct)]
pub struct DuckStructNested {
    id: i32,
    inner: DuckStructInner,
    maybe: Option<DuckStructInner>,
}
```

```sql
SELECT (dfn_echo_struct_simple({'id': 1, 'name': 'a'})).id;                                 -- 1
SELECT (dfn_echo_struct_nested({'id': 1, 'inner': {'key': 'k', 'value': 2}, 'maybe': NULL})).inner.value;  -- 2
```

A `STRUCT` **literal** is matched by its anonymous type, so its field list has to line up exactly —
same names, same types, same count. DuckDB inserts no implicit cast to add, drop or rename a field,
and the error it gives does not mention that: `{'id': 1}` against the struct above fails with
`No function matches the given name and argument types 'dfn_echo_struct_nested(STRUCT(id INTEGER))'.
You might need to add explicit type casts.` Write every field out (nullable ones included, as
`NULL`), or cast explicitly — `…::STRUCT(id INTEGER, "inner" STRUCT(…), maybe STRUCT(…))` (quote a
field name that clashes with a keyword, such as `inner`), or to a name created by `create_type`
(`…::ticket`, see below). A *column* of the right type needs none of this: it already has the type.

Structs nest, and they may be used inside the other containers — `Vec<DuckStructSimple>`,
`IndexMap<String, DuckStructSimple>`, `DuckArray<DuckStructSimple, 2>` and the `Option`-wrapped
variants are all supported.

The only constraint left is that the struct must have **named fields** — a field's type is used as
written. `Option<T>` marks a field nullable, and nesting is fine: `Option<Vec<Option<i32>>>`,
`Option<Option<i32>>` and `Vec<Option<Option<i32>>>` all compile. An extra `Option` layer is inert —
it maps to the same DuckDB type, a `NULL` reads back as the outer `None`, and both `None` and a
nested `Some(None)` write `NULL`.

A struct can also be given a **name in the catalog**: `#[duck(sql_name = "ticket", create_type = true)]`
makes the extension run `CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT(...)` when it loads, after which
SQL can use `ticket` as a type — a column type or a cast target:

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
#[duck(sql_name = "ticket", create_type = true)]
pub struct Ticket {
    pub id: i64,
    pub priority: Priority,      // #[derive(DuckEnum)] enum
    pub labels: Vec<String>,
}
// CREATE TYPE IF NOT EXISTS "ticket" AS
//   STRUCT("id" BIGINT, "priority" ENUM('low', 'medium', 'high'), "labels" VARCHAR[]);
```

```sql
CREATE TABLE tickets (v ticket);
INSERT INTO tickets VALUES ({'id': 1, 'priority': 'low', 'labels': ['a']});
SELECT dfn_echo_struct_ticket(v) FROM tickets;   -- 函数用的是等价的结构化类型
```

The field types are not spelled out by the macro: the `LogicalType` of each field is rendered
recursively (through DuckDB's own type introspection), so enums, nested structs, `LIST` / `ARRAY` /
`MAP`, `DECIMAL` and hand-written custom types all come along — and `Option<T>` does not change the
type text, because nullability is not part of a DuckDB type. The statement is idempotent, so loading
the extension twice is fine and a type of that name that already exists is left untouched.

Write `create_type = "print"` instead of `true` to render that same statement without creating
anything: the DDL is printed once, after every registration has run, inside a `-- [duckfn]` frame
that says it was *not* executed — easy to inspect, and to copy and run. See
[Attributes → Named types in the catalog](./attributes.md#named-types-in-the-catalog).

## Containers of containers

The containers compose. A struct field may be a list or a map, a list may hold structs, a map value
may be another map:

```rust
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct DuckStructWithList {
    id: i32,
    data: Vec<i32>,
    tags: Vec<String>,
}
```

`Vec<Option<Vec<Option<T>>>>`, `IndexMap<String, Option<IndexMap<String, Option<i32>>>>` and
`DuckArray<Option<DuckArray<Option<i32>, 2>>, 2>` all have DuckDB equivalents.

## Lazy arguments

`DuckLazy<T>` reads like `T` on the SQL side — same logical type, same `NULL` rules — but the parse is
deferred: reading a row only records *where* the value lives (O(1)), and `get()` / `try_get()` does the
real work. It is for arguments that stay constant across many rows yet cost far more to parse than the
values next to them, such as an aggregate's configuration argument:

```rust
#[derive(Default, Debug, Clone)]
struct MyState {
    cfg: DuckLazySlot<Config>,
    sum: f64,
}

#[duck_aggregate_function]
fn my_agg(cfg: DuckLazy<Config>, v: i64, state: &mut MyState) -> DuckResult<()> {
    // Parse once, on the first row; every later row only bumps a refcount.
    let cfg = state.cfg.resolve(&cfg)?;
    state.sum += cfg.weight(v);
    Ok(())
}
```

Rules worth knowing:

- The token is **only valid inside the callback that produced it**. Storing it in the aggregate state,
  or consuming it in a later chunk or on another thread, is a misuse — the runtime guard turns it into
  a `DuckLazy<T> is stale: ...` query error rather than undefined behaviour. Cache the *parsed value*,
  never the token.
- `DuckLazy<T>` is **read-only**: a return type or output column that uses it raises an error.
- The bind/`Value` path (table-function arguments) rejects it explicitly, because those values only
  live inside the bind callback.
- A `NULL` cell reads as `None` like everywhere else — write `Option<DuckLazy<T>>` when the argument
  may be `NULL`.
- `DuckLazySlot<T>` is where the parsed value goes when the consumer is an aggregate state: `resolve`
  parses once (and `resolve_optional` is the nullable flavour), `combine` carries the result over when
  DuckDB merges parallel states — without re-parsing it — and `get` reads it back from `result()`.

## Enums

DuckDB's `ENUM` is not one of the value types duckfn maps, but the mapping is completely mechanical, so
`#[derive(DuckEnum)]` generates it from an ordinary Rust enum:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
#[duck(rename_all = "lowercase", sql_name = "priority", create_type = true)]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}
```

- the **dictionary is the declaration order**, and `rename_all` / a variant's `#[duck(rename = "...")]`
  decide the labels (`['low', 'medium', 'high']` above);
- the logical type is that `ENUM(...)`, so the enum can be an argument, a return value, a `STRUCT`
  field or a container element, and `Option<Priority>` makes it nullable;
- `create_type = true` additionally runs `CREATE TYPE IF NOT EXISTS "priority" AS ENUM (...) ` when the
  extension loads — idempotent, and it leaves an existing type of that name alone — so SQL can write
  `'high'::priority` and use `priority` as a column type; `create_type = "print"` renders that same
  statement but only prints it to stderr, leaving the catalog untouched;
- as with any argument, a **non-nullable** enum parameter needs `Default` (the generated argument
  struct derives it), hence the `#[derive(Default)]` + `#[default]` in the example.

Note that DuckDB only inserts the implicit `VARCHAR → ENUM` cast for *constant* strings; a column or an
expression has to be cast explicitly (`'low'::priority`). See [Attributes](./attributes.md) for the
`#[duck(...)]` arguments.

## Two type systems: static and dynamic

Everything above is the **static** system: a Rust type is mapped to a DuckDB type once, at compile
time, by implementing `DuckValueType` (`#[derive(DuckStruct)]` and `#[derive(DuckEnum)]` generate
those implementations for you). It is what scalar functions, aggregate functions, casts, `COPY`, SQL
macros and the ordinary table functions use — and it is the default choice: one Rust type ↔ one
logical type, with no runtime dispatch.

The **dynamic** system exists because a compile-time mapping structurally cannot answer "what are
this query's columns?" when that is only known during bind (a file header, a dictionary table, a
remote schema). It is not a replacement, and it does not duplicate the mapping above — it sits on top
of it:

| | Static (`DuckValueType`) | Dynamic (`DuckTypeDesc` / `DuckDynamicValue`) |
| --- | --- | --- |
| Type decided | compile time | bind time, at run time |
| One Rust type ↔ | one logical type | any logical type, described per column |
| Used by | scalar / aggregate / cast / `COPY` / SQL macro / ordinary table functions | table functions with `dynamic_columns = true`, or a hand-written `DynamicTableFunctionAdapter` |
| Column names | the struct's field names | whatever the `DuckResultSchema` says |
| Cost | none | one enum dispatch per cell, one `Vec` per row |

What the dynamic side actually reuses, rather than re-implementing:

- the scalar write path calls each primitive's `DuckValueType::write_valid_to_vector_writer`, so the
  physical write is identical to the static one;
- the `LIST` / `MAP` / `STRUCT` layout conventions (child vectors, entries, NULL rows, finishing) live
  in one shared module used by both paths;
- argument parsing is still `#[derive(DuckStruct)]` — a dynamic table function's `Args` is a
  `DuckBindArgs`;
- the scalar half of `DuckTypeDesc` is just a `TypeId` from the table at the top of this page.

Two things the dynamic side deliberately does **not** cover, so keep using the static system for
them: `ENUM` and `ARRAY` (as well as `UNION` / `BIT`) cannot be expressed as a `DuckTypeDesc`, because
their parameters are not part of `TypeId`; and a schema that *is* known at compile time is always
better served by a `#[derive(DuckStruct)]` row struct — the dynamic path pays an enum dispatch per
cell and a `Vec` per row.

### Reading a `Value`

Bind arguments — and the children of a `LIST` / `MAP` / `STRUCT` value — arrive as `Value`s, and there
is one rule worth knowing:

- test them with `duckfn::duck_value_is_null(&value)`, **not** `value.is_null()`. quack-rs'
  `is_null()` only looks at whether the handle pointer is null, so an explicit `arg = NULL` slips
  through and the typed read that follows aborts the process with `fatal runtime error: Rust cannot
  catch foreign exceptions`; only *omitting* the argument yields a null handle.
- a `NULL` reads as `None`, and `Some(None)` is how nested reads say "this element is NULL".

## Known gaps

| Gap | Detail |
| --- | --- |
| Behind a Cargo feature | `TIME_NS` is mapped by `DuckTimeNs`, but only with the [`duckdb-1-5` feature](../getting-started/installation.md#cargo-features) enabled; the [`chrono` conversions](#conversions-with-chrono) need the `chrono` feature. |
| Not mapped, but implementable | `UNION`, `BIT`, `VARINT`, `GEOMETRY`, `VARIANT`: quack-rs has no read/write for them, but a [`DuckValueType` implementation of your own](./custom-types.md) can call the DuckDB C API through the raw vector handle. `ENUM` is generated for you by [`#[derive(DuckEnum)]`](#enums). |
| Not storable types | `ANY`, `SQLNULL` and the integer/string literal types exist only in DuckDB's own signatures and literals; they cannot be an extension's argument or return type. |
| No dedicated `DuckList` / `DuckMap` wrappers | `DuckList<T>` / `DuckMap<K, V>` are only aliases for `Vec<T>` / `IndexMap<K, V>`; the real types are the standard library / `indexmap` ones. |
| `ARRAY` bind parameters | Not supported (see above). |
| `MAP` keys | Never nullable. |
| `DECIMAL` | DuckDB requires `WIDTH < 39`. |

## Source and tests

- [`src/extension/types/`](https://github.com/shijianjs/duckfn/tree/main/src/extension/types) — the echo functions for every type
- [`test/sql/types/`](https://github.com/shijianjs/duckfn/tree/main/test/sql/types) — the expected results
- [`duckfn/src/value_types/`](https://github.com/shijianjs/duckfn/tree/main/duckfn/src/value_types) — the type implementations themselves
- [`src/extension/functions/chrono_bridge.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/chrono_bridge.rs) — the `chrono` conversions in use
- [`test/sql/functions/chrono_bridge.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/chrono_bridge.test) — their expected results, including the `infinity` errors

## Next

- [Custom types](./custom-types.md) — implementing your own type mapping.
- [Errors and panics](./errors-and-panics.md)
- [Table functions](./table-functions.md) — these types as output columns.
