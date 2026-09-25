---
title: Custom types
sidebar_position: 11
description: Implement DuckValueType for your own types, and how to reach logical types duckfn does not map yet.
---

# Custom types

`DuckValueType` is a public, unsealed trait, so a type defined outside `duckfn` can be mapped to a
DuckDB type. The example extension does exactly that in
`duckfn-quack/src/extension/types/custom_type_echo.rs`, and this page walks through it.

## Why write one

- **One physical type, two meanings.** `DOUBLE` is a temperature in one place and a distance in
  another. A newtype keeps them apart — the same reason duckfn wraps `TIMESTAMP`, `TIMESTAMP_S`,
  `TIME` and friends instead of mapping them all to `i64`.
- **A logical type duckfn does not map.** `ENUM`, `BIT`, `VARINT`, `GEOMETRY`, `VARIANT` and the
  DuckDB 1.5 additions each need either a Cargo feature or direct C API calls; see
  [Type mapping](./types.md#known-gaps).

## A minimal implementation

```rust
use duckfn::DuckValueType;
use quack_rs::prelude::{TypeId, Value, VectorReader, VectorWriter};

/// A Celsius temperature: the physical representation is `DOUBLE`, and the newtype carries the
/// semantics — DuckDB has no temperature type.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Celsius(pub f64);

impl DuckValueType for Celsius {
    fn type_id() -> TypeId {
        TypeId::Double
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self(unsafe { reader.read_f64(row) })
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f64(idx, v.0) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self(value.as_f64())
    }
}
```

| Method | Needed | Purpose |
| --- | --- | --- |
| `type_id()` | always | The DuckDB logical type this maps to. |
| `read_valid_by_vector_reader()` | always | Read one valid value out of an input vector. |
| `write_valid_to_vector_writer()` | always | Write one valid value into an output vector. |
| `read_by_duck_value_valid_simple()` | for arguments | Read from a `duckdb_value` — the path used by table function arguments, struct fields and casts. |
| `logical_type()` | for parameterised types | Override when the type is not fully described by `type_id()` — `DECIMAL(18,3)`, `LIST(T)`, `ARRAY(T,N)`, `STRUCT(...)`. |
| `from_null()` | for nullable types | Whether the type can hold `NULL` *as a value*. `Option<T>` returns `Some(None)`; the default is `None`, meaning "this type cannot hold NULL, so a NULL slot invalidates the enclosing value". |
| `write_null()` | for containers | Override when `NULL` must also be propagated into child vectors. |
| `create_reader_from_vector()` / `create_writer_batch()` / `write_finish()` | for containers | Where child readers and writers are attached, and child lengths are finalised. |

Two bounds are worth knowing before you start:

- The trait itself requires `Clone + Debug + Send + Sync + 'static`.
- A type used as a **function argument** also needs `Default`, because the argument struct the macro
  generates derives it.

`read()` and `write()` are the NULL-aware entry points and are deliberately not meant to be
overridden; `read_slot()` sits on top of `read()` and adds the `from_null()` fallback, which is how
containers and struct fields read a single element or field.

## Using it

As a scalar function argument and result, nothing special is needed:

```rust
#[duck_scalar_function]
fn dfn_echo_celsius(t: Celsius) -> Celsius {
    t
}
```

```sql
SELECT dfn_echo_celsius(21.5::DOUBLE);         -- 21.5
SELECT typeof(dfn_echo_celsius(21.5::DOUBLE)); -- DOUBLE
```

It also works as a `STRUCT` field — `#[derive(DuckStruct)]` emits a compile-time
`assert_impl_duck_value_type::<T>()` for every field, so a field whose type does not implement the
trait is a compile error rather than a runtime surprise:

```rust
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct TemperatureReading {
    pub place: String,
    pub celsius: Celsius,
}
```

```sql
SELECT (dfn_echo_temperature_reading({'place': 'oslo', 'celsius': -3.5::DOUBLE})).place;  -- oslo
SELECT typeof((dfn_echo_temperature_reading({'place': 'oslo', 'celsius': -3.5::DOUBLE})).celsius);
-- DOUBLE
```

And as a table function argument and column:

```rust
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoCelsiusRow {
    pub v: Option<Celsius>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_celsius(
    v: Option<Celsius>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoCelsiusRow> {
    echo_rows(v, count, |v| TableEchoCelsiusRow { v })
}
```

```sql
SELECT CAST(v AS VARCHAR) FROM dfn_table_echo_celsius(1.5::DOUBLE, count => 3);
-- 1.5
-- NULL
-- 1.5
```

## Going further

**A logical type quack-rs does not map.** `DuckValueReader` and `DuckValueWriter` both expose the raw
`c_duckdb_vector` next to the high-level `vector_reader` / `vector_writer`, so an implementation can
drop to the DuckDB C API when it has to. That is the route for `ENUM` (read the index, then look the
label up in the enum dictionary), `BIT` and `VARINT`, all of which have no quack-rs accessor. The `Color`
enum in `duckfn-quack/src/extension/types/custom_type_echo.rs` is the worked example: it declares the dictionary with
`LogicalType::enum_type(&[...])`, overrides the raw-vector `read_valid` / `write_valid` to move the index
in and out of the vector, and reads the label from a bind-time `duckdb_value` with `Value::as_str()`.
For `ENUM` specifically you rarely need to write that by hand: `#[derive(DuckEnum)]` generates exactly
this implementation — plus, with `create_type = true`, a load-time `CREATE TYPE ... AS ENUM (...)` — so
the hand-written version is there to explain the mechanics, not as the day-to-day route.

**A container type.** Containers need child readers and writers, and NULL has to be propagated into
the children. `src/value_types/duck_list.rs`, `duck_map.rs`, `duck_array.rs` and
`duck_struct.rs` are the reference implementations.

**A deferred read.** `duck_lazy.rs` is the other extreme: `read_valid` only records the position plus a
liveness token and the real parse happens later in `get()`. Copy that pattern when your own type wants
to postpone work — the token included, since it is what turns "consumed after its chunk died" from
undefined behaviour into a query error.

**A DuckDB 1.5 logical type.** Some of them only need a Cargo feature to become available:
`TIME_NS` ships in duckfn behind `duckdb-1-5`, which forwards to the same feature on quack-rs — see
[Installation](../getting-started/installation.md#cargo-features).

## Source and tests

- [`duckfn-quack/src/extension/types/custom_type_echo.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/src/extension/types/custom_type_echo.rs) — the `Celsius` type, implemented outside duckfn
- [`duckfn-quack/test/sql/types/custom_type_echo.test`](https://github.com/shijianjs/duckfn/blob/main/duckfn-quack/test/sql/types/custom_type_echo.test) — the expected results
- [`src/value_types/duck_value_type.rs`](https://github.com/shijianjs/duckfn/blob/main/src/value_types/duck_value_type.rs) — the trait itself

## Next

- [Type mapping](./types.md) — what is already built in, and what is not.
- [Architecture](../internals/architecture.md#6-value-types) — how the readers and writers fit together.
