---
title: Type mapping
sidebar_position: 8
description: How DuckDB types map to Rust types, including LIST, MAP, ARRAY and STRUCT, plus the nullability rules and the gaps.
---

# Type mapping

An argument or return type is written as the Rust type you want; the DuckDB type follows from it.

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
| `DuckOptionArray<T, N>` (`[Option<T>; N]`) | `T[N]`, elements nullable |

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

The derive has two constraints: the struct must have **named fields**, and an optional field may only
be a single layer of `Option<T>` (`Option<Vec<Option<i32>>>` is fine as a field type, but
`Option<Option<T>>` is not).

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
`DuckOptionArray<DuckOptionArray<i32, 2>, 2>` all have DuckDB equivalents.

## Known gaps

| Gap | Detail |
| --- | --- |
| Behind a Cargo feature | `TIME_NS` is mapped by `DuckTimeNs`, but only with the [`duckdb-1-5` feature](../getting-started/installation.md#cargo-features) enabled. |
| Not mapped, but implementable | `ENUM`, `UNION`, `BIT`, `VARINT`, `GEOMETRY`, `VARIANT`: quack-rs has no read/write for them, but a [`DuckValueType` implementation of your own](./custom-types.md) can call the DuckDB C API through the raw vector handle. |
| Not storable types | `ANY`, `SQLNULL` and the integer/string literal types exist only in DuckDB's own signatures and literals; they cannot be an extension's argument or return type. |
| No `DuckList` / `DuckMap` | Lists and maps *are* `Vec` and `IndexMap`; there are no dedicated wrapper types. |
| `ARRAY` bind parameters | Not supported (see above). |
| `MAP` keys | Never nullable. |
| `Vec<T>` / `[T; N]` elements | Never nullable — use the `Option` variants. |
| `DECIMAL` | DuckDB requires `WIDTH < 39`. |

## Source and tests

- [`src/extension/types/`](https://github.com/shijianjs/duckfn/tree/main/src/extension/types) — the echo functions for every type
- [`test/sql/types/`](https://github.com/shijianjs/duckfn/tree/main/test/sql/types) — the expected results
- [`duckfn/src/value_types/`](https://github.com/shijianjs/duckfn/tree/main/duckfn/src/value_types) — the type implementations themselves

## Next

- [Custom types](./custom-types.md) — implementing your own type mapping.
- [Errors and panics](./errors-and-panics.md)
- [Table functions](./table-functions.md) — these types as output columns.
