---
title: Type mapping
sidebar_position: 8
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
| `TIMESTAMPTZ` | `DuckTimestampTz` | `millis_since_epoch: i64` |
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
SELECT (dfn_echo_struct_simple({'id': 1, 'name': 'a'})).id;                          -- 1
SELECT (dfn_echo_struct_nested({'id': 1, 'inner': {'key': 'k', 'value': 2}})).inner.value;  -- 2
```

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
#[duck_aggregate_function]
fn my_agg(cfg: DuckLazy<Config>, v: i64, state: &mut MyState) -> DuckResult<()> {
    // Parse once, on the first row; every later row reuses the parsed value.
    if state.cfg.is_none() {
        state.cfg = Some(cfg.get());
    }
    // ... use state.cfg
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
  `'high'::priority` and use `priority` as a column type;
- as with any argument, a **non-nullable** enum parameter needs `Default` (the generated argument
  struct derives it), hence the `#[derive(Default)]` + `#[default]` in the example.

Note that DuckDB only inserts the implicit `VARCHAR → ENUM` cast for *constant* strings; a column or an
expression has to be cast explicitly (`'low'::priority`). See [Attributes](./attributes.md) for the
`#[duck(...)]` arguments.

## Known gaps

| Gap | Detail |
| --- | --- |
| Behind a Cargo feature | `TIME_NS` is mapped by `DuckTimeNs`, but only with the [`duckdb-1-5` feature](../getting-started/installation.md#cargo-features) enabled. |
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

## Next

- [Custom types](./custom-types.md) — implementing your own type mapping.
- [Errors and panics](./errors-and-panics.md)
- [Table functions](./table-functions.md) — these types as output columns.
