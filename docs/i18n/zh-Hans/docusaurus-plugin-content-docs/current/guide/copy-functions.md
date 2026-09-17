---
title: COPY 函数
sidebar_position: 5
description: 为 COPY ... TO 与 COPY ... FROM 提供自定义文件格式，建立在运行时动态列之上。
---

# COPY 函数

COPY 函数让你用自己的文件格式完成 `COPY` —— 而且是双向的：

```sql
COPY (SELECT * FROM orders) TO 'orders.tsv' (FORMAT dfn_copy_tsv);
COPY orders FROM 'orders.tsv' (FORMAT dfn_copy_tsv_from);
```

两者都建立在**[运行时动态列](./table-functions.md#动态列)**之上：列不是编译期固定的。
`COPY ... TO` 在 bind 阶段由查询的输出列反推出 schema，再把每个数据块以 `DuckDynamicRow` 交给你的
函数；`COPY ... FROM` 读的是**目标表**的 schema，由你按批交出动态行。因此 `LIST` / `STRUCT` /
`MAP` / `DECIMAL` 与 `NULL` 在两个方向上都能工作。

:::note[DuckDB 1.5.0+]
COPY 函数来自 DuckDB 1.5.0 起的 C API，因此需要 `duckfn` 的 `duckdb-1-5` feature：

```toml
duckfn = { version = "0.0.4", features = ["duckdb-1-5"] }
```
:::

## `COPY ... TO`

格式名就是函数名，函数每个数据块调用一次：

```rust
#[duck_copy_function]
fn dfn_copy_tsv(writer: &mut TsvWriter, rows: &[DuckDynamicRow]) -> DuckResult<()> {
    /* 把 rows 写进 writer */
}
```

| 部分 | 含义 |
| --- | --- |
| `&mut MyWriter` | 格式的 writer 状态，实现 [`DuckCopyToWriter`](#duckcopytowriter)。 |
| `&[DuckDynamicRow]` | 本批的行：至多 `2048` 行，每行与 schema 的列一一对应，`None` 表示 SQL NULL。 |
| `-> DuckResult<()>` | `Err` 让整条 `COPY` 失败；panic 也会被转成查询错误。 |

两个参数顺序可以互换。

### `DuckCopyToWriter`

writer 状态只负责生命周期，逐批的写出逻辑留在函数里：

```rust
pub trait DuckCopyToWriter: Sized + 'static {
    /// 第一块之前调用一次：目标路径、动态 schema 与 COPY 选项。
    fn open(path: &str, schema: &DuckResultSchema, options: &DuckCopyOptions) -> DuckResult<Self>;

    /// 最后一块之后调用一次；在这里 flush / 关闭。
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}
```

`schema.columns()` 按查询的列顺序给出 `(列名, DuckTypeDesc)` —— 描述决定了这一列是 `LIST` 还是
`STRUCT`，也正是格式渲染时要依据的东西。`finish` 默认什么都不做；覆盖它的意义在于 flush 出错必须
能被报出来（`Drop` 没法返回错误，只靠 drop 时 flush 的 `BufWriter` 会把错误悄悄吞掉）。

### 选项

`COPY ... TO (...)` 的额外选项以一个 `STRUCT` 到达，被包装成 `DuckCopyOptions`（选项名 → 动态值，
查名字大小写不敏感）：

```rust
fn open(path: &str, schema: &DuckResultSchema, options: &DuckCopyOptions) -> DuckResult<Self> {
    let header = options.get_bool("header").unwrap_or(false);
    /* … */
}
```

```sql
COPY (SELECT 1 AS i) TO 'out.tsv' (FORMAT dfn_copy_tsv, HEADER true);
```

`get_bool` / `get_i64` / `get_str` / `get` 覆盖常见类型，`entries()` 能拿到全部。类型无法用
`DuckTypeDesc` 表达的选项（例如 `ENUM`）会被跳过，而不是让整条 `COPY` 失败。

### 四个阶段

DuckDB 通过四个回调驱动一次 `COPY TO`。`#[duck_copy_function]` 把四个都生成好，你只需要写行循环：

| 阶段 | 调用次数 | duckfn 做什么 | 你写什么 |
| --- | --- | --- | --- |
| bind | 一次 | 把输出列反推成 `DuckResultSchema`、读出选项，两者存成 bind data。 | — |
| global init | 一次 | 调用 `DuckCopyToWriter::open(path, schema, options)`。 | `open` |
| sink | 每块一次 | 把数据块读成 `Vec<DuckDynamicRow>` 并调用被标注的函数。 | 函数体 |
| finalize | 一次 | 调用 `DuckCopyToWriter::finish()`。 | `finish` |

每个阶段都跑在 `catch_unwind` 里，`Err` 与 panic 都会报给 DuckDB，因此失败会中止 `COPY`，而不是跨
FFI 边界展开。

## `COPY ... FROM`

`COPY ... FROM` 是把文件装载进**已存在的表**，所以 schema 不归你决定：DuckDB 把目标表的列给你，
你按批交出动态行：

```rust
#[duck_copy_from_function]
fn dfn_copy_tsv_from(reader: &mut TsvReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> {
    /* 返回至多 limit 行；空 Vec 表示读完了 */
}
```

| 部分 | 含义 |
| --- | --- |
| `&mut MyReader` | 读取器状态，实现 [`DuckCopyFromReader`](#duckcopyfromreader)。 |
| `limit: usize` | 批大小 —— 一个 DuckDB 向量的行数；少取几行也可以。 |
| `-> DuckResult<Vec<DuckDynamicRow>>` | 空 `Vec` 表示流结束。 |

### `DuckCopyFromReader`

同样是两阶段，而参数里同时带着路径与选项：

```rust
pub trait DuckCopyFromReader: Sized + Send + 'static {
    /// 字段 0 必须是文件路径（唯一的位置参数），其余字段是本读取器支持的命名 COPY 选项。
    type Args: DuckBindArgs;

    fn open(args: Self::Args, schema: &DuckResultSchema) -> DuckResult<Self>;

    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}
```

`Args` 就是普通的 `#[derive(DuckStruct)]` 结构体，于是
`COPY ... FROM 'f' (FORMAT fmt, SKIP_ROWS 1)` 会以字段的形式到达 `open`：

```rust
#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "skip_rows")]
pub struct MyFromArgs {
    pub path: String,
    pub skip_rows: Option<i64>,
}
```

三条规则来自 DuckDB：

- **位置参数恰好一个**（文件路径；`duckfn` 会校验并在不满足时报出实际个数）；
- reader **不能声明结果列** —— schema 来自目标表，由 `open` 收到；
- 你接受的每个选项都必须通过 `Args` **声明**；未声明的选项会在 bind **之前**被 binder 拒绝，而且
  那条报错信息指向**表函数名** —— 这正是 reader 表函数要带上格式名的原因。

### 三个回调

| 阶段 | 调用次数 | duckfn 做什么 | 你写什么 |
| --- | --- | --- | --- |
| bind | 一次 | 解析 `Args`、读目标表 schema、调用 `Reader::open`，并保存状态。 | `open` |
| scan | 直到 EOF | 调用被标注的函数，把返回的行写进输出数据块。 | 函数体 |
| finalize | — | 表函数没有 finalize 回调：查询结束、状态被释放时调用 `Reader::finish`。 | `finish` |

因为没有 finalize 回调，`finish` 里的错误**无法上报** —— 只会变成一行 `-- [duckfn]` 警告。任何必须
中止装载的错误都应该放在 `open` 或取批函数里。

行的校验是免费的：某一行的列数或某列的类型与目标表不符时，`DuckDynamicRow::write_batch` 里的逐列
校验会让整条 `COPY` 以可读的错误失败，而不是写进一行错位的数据。

## 编码由你决定

`DuckDynamicValue` 是「值」，不是文本格式。`to_text` 是**展示**用的渲染（字符串原样输出、容器里的
`NULL` 写作 `NULL`），一旦字符串本身就是 `NULL` 就会产生歧义 —— 所以格式应当按自己的转义约定递归
渲染。TSV 示例就是这么做的，其中转义部分值得照抄：

| 关注点 | 示例的做法 |
| --- | --- |
| 行 / 列分帧 | 一行一条记录、制表符分隔、NULL 写作 `\N` |
| 容器里的字符串 | 加引号（`['NULL', NULL]` 与 `['NULL', 'NULL']` 不会混） |
| 字符串里出现分隔符 | 转义（制表符 → `\t`），因此不会破坏分帧 |
| `BLOB` | 每字节写成 `\xHH` |

## 生成的东西与注册

与其它函数属性一样，宏保留原函数，并生成一个与函数同名的模块：

| 项 | 用途 |
| --- | --- |
| `CopyFunctionImpl` / `CopyFromFunctionImpl` | 适配层实现。 |
| `copy_function_builder()` | 配置好的 `CopyFunctionBuilder`（返回 `DuckResult`）。 |
| `copy_function_register(connection)` / `copy_from_register(connection)` | 在连接上注册该格式。 |

`auto_register = true`（默认）时格式随扩展加载自动注册，上面的 SQL 开箱可用；
`auto_register = false` 时只生成上述项：

```rust
#[duck_copy_from_function(auto_register = false)]
fn dfn_copy_manual(reader: &mut MyReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> {
    /* … */
}

#[duck_custom_register]
fn dfn_copy_manual_register(c: &Connection) -> DuckResult<()> {
    dfn_copy_manual::copy_from_register(c)
}
```

## 限制

- **`COPY ... FROM` 是串行扫描的。** 读取器状态只要求 `Send`（不要求 `Sync`），适配层用
  `set_max_threads(1)` 把扫描钉在单线程上，没有并行装载。
- **`ENUM` / `ARRAY` / `UNION` / `BIT` 列在两个方向上都被拒绝**并给出错误，因为它们的参数无法表达成
  `DuckTypeDesc`（见[类型映射](./types.md)）。
- **`COPY ... TO` 的列名是合成的**（`column_0`、`column_1`…）：DuckDB 只暴露输出列的类型，不暴露
  名字。需要真实列名的格式应该用自己的选项携带，而不是猜。
- **`MAP` 的值不能为 `NULL`** —— 动态值里不行，目标表里也不行。
- 用已存在的名字注册格式会报注册错误，所以不要与 `csv`、`parquet`、`json` 等撞名。

## 源码与测试

- [`src/extension/functions/copy_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/copy_function.rs) —— `dfn_copy_tsv` 写出示例
- [`src/extension/functions/copy_from_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/copy_from_function.rs) —— `dfn_copy_tsv_from` 读入示例
- [`src/extension/functions/tsv_format.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/tsv_format.rs) —— 共用的单元格编解码（转义与解析）
- [`test/sql/functions/copy_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/copy_function.test) 与 [`copy_from_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/copy_from_function.test) —— 期望结果
- [`duckfn/src/functions/copy_to_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/copy_to_adapter.rs) / [`copy_from_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/copy_from_adapter.rs) —— 运行时侧

## 接下来

- [表函数](./table-functions.md) —— 这些格式搬运的动态列
- [类型映射](./types.md)
- [属性参考](./attributes.md)
