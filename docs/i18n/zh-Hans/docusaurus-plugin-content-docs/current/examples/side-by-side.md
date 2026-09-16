---
title: 同样功能的两种写法
sidebar_position: 2
description: rusty_echo、rusty_quack、word_count、first_word 分别用原始 duckdb / quack-rs 绑定与 duckfn 写一遍的对照。
---

# 同样功能的两种写法

这里的四个函数都是上游项目自带的示例。`rusty_echo` 与 `rusty_quack` 来自 DuckDB 官方的 Rust 模板
（[`src/lib.rs`](https://github.com/duckdb/extension-template-rs/blob/main/src/lib.rs)），
`word_count` 与 `first_word` 来自 [`quack-rs`](https://quack-rs.com/getting-started/first-extension.html)
的 hello-ext 示例。

每个函数出现两次：左边用原始绑定写 —— 前两个是 `duckdb` crate，后两个是 `quack-rs`；右边是用
`duckfn` 写出的同一个函数。左栏省去了 import 与扩展入口点，因为每个函数这部分都一样；真正的差别在
函数体，也正是值得对照的地方。

## 对比 duckdb 标量函数

函数签名：`rusty_echo(varchar) -> varchar`

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

// 在入口点里注册
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

```sql
SELECT rusty_echo('Hello');  -- 🐤 Hello 🦀 Hello
```

## 对比 duckdb 表函数

函数签名：`rusty_quack(varchar) -> table(column0 varchar)`

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

// 在入口点里注册
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

```sql
SELECT * FROM rusty_quack('Sam');  -- Rusty Quack Sam 🐥
```

## 对比 quack-rs 聚合函数

函数签名：`word_count(varchar) -> bigint`

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
            continue; // NULL 输入 → 跳过（贡献 0 个词）
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
            // WordCountState 若增加字段，这里也要一并合并。
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

// 注册
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

```sql
SELECT word_count(s)
FROM (VALUES ('hello world'), ('one two three'), (NULL)) t(s);  -- 5
```

## 对比 quack-rs 标量函数

函数签名：`first_word(varchar) -> varchar`

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
            unsafe { writer.set_null(row) }; // NULL 入 → NULL 出
            continue;
        }
        let s = unsafe { reader.read_str(row) };
        unsafe { writer.write_varchar(row, first_word(s)) };
    }
}

// 注册
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

```sql
SELECT first_word(s)
FROM (VALUES ('hello world'), ('  padded  '), (''), (NULL)) t(s);
-- hello
-- padded
-- （空串）
-- NULL
```

## 这组对比说明了什么

右栏并不是左栏的「精简子集」。`duckfn` 版本保留了同样的 NULL 处理（`input.map(…)` 与 `Option<T>`
对应 `row_is_null` / `set_null`）、同样的并行聚合状态合并（`simple_combine` 对应 `wc_combine`），
以及同样的注册 —— 属性宏自己把注册代码生成出来，没有 `register_scalar_function` 或
`AggregateFunctionBuilder` 链条需要跟着改。

真正消失的是逐行的 FFI 管道：`DataChunkHandle`、`VectorReader` / `VectorWriter`、
`unsafe extern "C"` 回调、`FfiState` 以及状态的生命周期回调。而无法被生成的仍留在原处 ——
函数体本身，以及聚合状态。

## 接下来

- [示例扩展](./rusty-quack.md) —— 用同样的思路覆盖 duckfn 的每一项功能。
- [聚合函数](../guide/aggregate-functions.md) —— 状态类型与 `combine` 的细节。
