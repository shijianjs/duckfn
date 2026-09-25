---
title: Same functions, two ways
sidebar_position: 2
description: rusty_echo, rusty_quack, word_count and first_word against the raw duckdb and quack-rs bindings, next to the duckfn spelling.
---

# Same functions, two ways

The four functions on this page are the demos the two upstream projects ship. `rusty_echo` and
`rusty_quack` come from DuckDB's official Rust template
([`src/lib.rs`](https://github.com/duckdb/extension-template-rs/blob/main/src/lib.rs)), and
`word_count` and `first_word` from the [`quack-rs`](https://quack-rs.com/getting-started/first-extension.html)
hello-ext example.

Each function appears twice: on the left against the raw binding — the `duckdb` crate for the first
two, `quack-rs` for the last two — and on the right as the `duckfn` implementation of exactly the
same function. Imports and the extension entry point are left out of the left column, because they
are the same for every function; the body is what differs, and that is what is worth comparing.

## Scalar function, against duckdb

Signature: `rusty_echo(varchar) -> varchar`

```sql
SELECT rusty_echo('Hello');  -- 🐤 Hello 🦀 Hello
```

<div className="code-compare">
<div>

**duckdb**

```rust
struct EchoScalar;

impl VScalar for EchoScalar {
    type State = ();

    fn invoke(
        _state: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input_vec = input.flat_vector(0);
        let values = unsafe { input_vec.as_slice_with_len::<duckdb_string_t>(input.len()) };
        let mut output = output.flat_vector();

        for (i, value) in values.iter().enumerate() {
            if input_vec.row_is_null(i as u64) {
                output.set_null(i);
                continue;
            }

            let mut value = *value;
            let s = DuckString::new(&mut value).as_str();
            output.insert(i, format!("🐤 {s} 🦀 {s}").as_str());
        }
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![LogicalTypeId::Varchar.into()],
            LogicalTypeId::Varchar.into(),
        )]
    }
}

// in the entry point
con.register_scalar_function::<EchoScalar>("rusty_echo")?;
```

</div>
<div>

**duckfn**

```rust
#[duck_scalar_function]
fn rusty_echo(s: String) -> String {
    format!("🐤 {s} 🦀 {s}")
}
```

</div>
</div>

## Table function, against duckdb

Signature: `rusty_quack(varchar) -> table(column0 varchar)`

```sql
SELECT * FROM rusty_quack('Sam');  -- Rusty Quack Sam 🐥
```

<div className="code-compare">
<div>

**duckdb**

```rust
#[repr(C)]
struct HelloBindData {
    name: String,
}

#[repr(C)]
struct HelloInitData {
    done: AtomicBool,
}

struct HelloVTab;

impl VTab for HelloVTab {
    type InitData = HelloInitData;
    type BindData = HelloBindData;

    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        bind.add_result_column("column0", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        let name = bind.get_parameter(0).to_string();
        Ok(HelloBindData { name })
    }

    fn init(_: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(HelloInitData {
            done: AtomicBool::new(false),
        })
    }

    fn func(
        func: &TableFunctionInfo<Self>,
        output: &mut DataChunkHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let init_data = func.get_init_data();
        let bind_data = func.get_bind_data();
        if init_data.done.swap(true, Ordering::Relaxed) {
            output.set_len(0);
        } else {
            let vector = output.flat_vector(0);
            let result = CString::new(format!("Rusty Quack {} 🐥", bind_data.name))?;
            vector.insert(0, result);
            output.set_len(1);
        }
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }
}

// in the entry point
con.register_table_function::<HelloVTab>("rusty_quack")?;
```

</div>
<div>

**duckfn**

```rust
#[derive(Clone, Debug, DuckStruct)]
pub struct RustyQuackResult {
    column0: String,
}

#[duck_table_function]
fn rusty_quack(name: String) -> impl Iterator<Item = RustyQuackResult> {
    vec![RustyQuackResult {
        column0: format!("Rusty Quack {} 🐥", name),
    }]
    .into_iter()
}
```

</div>
</div>

## Aggregate function, against quack-rs

Signature: `word_count(varchar) -> bigint`

```sql
SELECT word_count(s)
FROM (VALUES ('hello world'), ('one two three'), (NULL)) t(s);  -- 5
```

<div className="code-compare">
<div>

**quack-rs**

```rust
#[derive(Default, Debug)]
struct WordCountState {
    count: i64,
}

impl AggregateState for WordCountState {}

unsafe extern "C" fn wc_state_size(_info: duckdb_function_info) -> idx_t {
    FfiState::<WordCountState>::size_callback(_info)
}

unsafe extern "C" fn wc_state_init(info: duckdb_function_info, state: duckdb_aggregate_state) {
    unsafe { FfiState::<WordCountState>::init_callback(info, state) };
}

unsafe extern "C" fn wc_update(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    states: *mut duckdb_aggregate_state,
) {
    let reader = unsafe { VectorReader::new(input, 0) };
    let row_count = reader.row_count();

    for row in 0..row_count {
        if !unsafe { reader.is_valid(row) } {
            continue; // NULL input → skip (contributes 0 words)
        }
        let s = unsafe { reader.read_str(row) };
        let words = count_words(s);

        let state_ptr = unsafe { *states.add(row) };
        if let Some(st) = unsafe { FfiState::<WordCountState>::with_state_mut(state_ptr) } {
            st.count += words;
        }
    }
}

fn count_words(s: &str) -> i64 {
    s.split_whitespace().count() as i64
}

unsafe extern "C" fn wc_combine(
    _info: duckdb_function_info,
    source: *mut duckdb_aggregate_state,
    target: *mut duckdb_aggregate_state,
    count: idx_t,
) {
    for i in 0..count as usize {
        let src = unsafe { FfiState::<WordCountState>::with_state(*source.add(i)) };
        let tgt = unsafe { FfiState::<WordCountState>::with_state_mut(*target.add(i)) };
        if let (Some(s), Some(t)) = (src, tgt) {
            t.count += s.count;
            // If you add fields to WordCountState, combine them here too.
        }
    }
}

unsafe extern "C" fn wc_finalize(
    _info: duckdb_function_info,
    source: *mut duckdb_aggregate_state,
    result: duckdb_vector,
    count: idx_t,
    offset: idx_t,
) {
    let mut writer = unsafe { VectorWriter::new(result) };

    for i in 0..count as usize {
        let state_ptr = unsafe { *source.add(i) };
        match unsafe { FfiState::<WordCountState>::with_state(state_ptr) } {
            Some(st) => unsafe { writer.write_i64(offset as usize + i, st.count) },
            None => unsafe { writer.set_null(offset as usize + i) },
        }
    }
}

unsafe extern "C" fn wc_state_destroy(states: *mut duckdb_aggregate_state, count: idx_t) {
    unsafe { FfiState::<WordCountState>::destroy_callback(states, count) };
}

// registration
AggregateFunctionBuilder::new("word_count")
    .param(TypeId::Varchar)
    .returns(TypeId::BigInt)
    .state_size(wc_state_size)
    .init(wc_state_init)
    .update(wc_update)
    .combine(wc_combine)
    .finalize(wc_finalize)
    .destructor(wc_state_destroy)
    .register(con)?;
```

</div>
<div>

**duckfn**

```rust
#[duck_aggregate_function]
fn word_count(input: Option<String>, state: &mut WordCountState) {
    state.count += input.map(|s| count_words(&s)).unwrap_or(0);
}

fn count_words(s: &str) -> i64 {
    s.split_whitespace().count() as i64
}

#[derive(Default, Debug, Clone)]
struct WordCountState {
    count: i64,
}

impl DuckAggregateState for WordCountState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.count += other.count;
    }

    fn simple_result(&self) -> Self::Output {
        self.count
    }
}
```

</div>
</div>

## Scalar function, against quack-rs

Signature: `first_word(varchar) -> varchar`

```sql
SELECT first_word(s)
FROM (VALUES ('hello world'), ('  padded  '), (''), (NULL)) t(s);
-- hello
-- padded
-- (empty)
-- NULL
```

<div className="code-compare">
<div>

**quack-rs**

```rust
pub fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

unsafe extern "C" fn first_word_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    let reader = unsafe { VectorReader::new(input, 0) };
    let mut writer = unsafe { VectorWriter::new(output) };
    let row_count = reader.row_count();

    for row in 0..row_count {
        if !unsafe { reader.is_valid(row) } {
            unsafe { writer.set_null(row) }; // NULL in → NULL out
            continue;
        }
        let s = unsafe { reader.read_str(row) };
        unsafe { writer.write_varchar(row, first_word(s)) };
    }
}

// registration
ScalarFunctionBuilder::new("first_word")
    .param(TypeId::Varchar)
    .returns(TypeId::Varchar)
    .function(first_word_scalar)
    .register(con)?;
```

</div>
<div>

**duckfn**

```rust
#[duck_scalar_function]
fn first_word(input: Option<String>) -> Option<String> {
    input.map(|s| s.split_whitespace().next().unwrap_or("").to_string())
}
```

</div>
</div>

## What the comparison shows

The right column is not a shortcut for a subset of the left. The `duckfn` versions carry the same
NULL handling (`input.map(…)` and `Option<T>` instead of `row_is_null` / `set_null`), the same state
merge for parallel aggregation (`simple_combine` instead of `wc_combine`), and the same
registration — the attribute emits it, so there is no `register_scalar_function` or
`AggregateFunctionBuilder` chain to keep in sync.

What disappears is the per-row FFI plumbing: `DataChunkHandle`, `VectorReader` / `VectorWriter`,
the `unsafe extern "C"` callbacks, `FfiState` and the state lifetime callbacks. The parts that
cannot be generated — the function body and the aggregate state — stay.

## Next

- [The example extension](./duckfn-quack.md) — the same ideas covering every feature duckfn has.
- [Aggregate functions](../guide/aggregate-functions.md) — the state type and `combine` in detail.
