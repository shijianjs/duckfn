---
title: COPY 函数
sidebar_position: 5
description: 为 COPY ... TO 提供自定义文件格式，把 bind / global init / sink / finalize 四个阶段藏在一个函数后面。
---

# COPY 函数

COPY 函数让 `COPY ... TO` 用上你自己的文件格式：

```sql
COPY (SELECT * FROM orders) TO 'orders.tsv' (FORMAT dfn_copy_tsv);
COPY orders TO 'orders.tsv' (FORMAT dfn_copy_tsv);
```

格式名就是函数名，函数每个数据块被调用一次：

```rust
#[duck_copy_function]
fn dfn_copy_tsv(writer: &mut TsvWriter, chunk: &DataChunk) -> DuckResult<()> {
    /* 把 `chunk` 里的行写进 `writer` */
}
```

:::note[DuckDB 1.5.0+]
COPY 函数来自 DuckDB 1.5.0 起的 C API，因此需要 `duckfn` 的 `duckdb-1-5` feature：

```toml
duckfn = { version = "0.0.3", features = ["duckdb-1-5"] }
```
:::

## 签名

签名是固定的 —— 一个 writer 加一个 chunk，顺序可以互换：

| 部分 | 含义 |
| --- | --- |
| `&mut MyWriter` | 该格式的 writer 状态，类型需实现 [`DuckCopyWriter`](#duckcopywriter)。 |
| `&DataChunk` | 本次要写出的数据块：至多 `2048` 行、包含查询的全部列。 |
| `-> DuckResult<()>` | `Err` 会让整条 `COPY` 失败；panic 同样会被转成查询错误。 |

`chunk.size()` 是本块的行数（最后一块可能更少），`chunk.column_count()` 与
`DuckCopyWriter::open` 收到的列数一致。

## `DuckCopyWriter`

writer 状态就是一个普通结构体，实现两个方法：

```rust
pub trait DuckCopyWriter: Sized + 'static {
    /// 只调用一次，在第一个数据块之前：`path` 是 COPY 的目标路径，
    /// `columns` 是查询输出列的逻辑类型。
    fn open(path: &str, columns: &[LogicalType]) -> DuckResult<Self>;

    /// 只调用一次，在最后一个数据块之后；在这里 flush 并关闭。
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}
```

`finish` 默认什么都不做。重写它是有意义的：flush 失败必须能被上报，而 `Drop` 无法返回错误，
只在 drop 时 flush 的 `BufWriter` 会把错误悄悄咽掉。

```rust
use duckfn::{DuckCopyWriter, DuckResult, LogicalType, TypeId, duck_error};
use std::fs::File;
use std::io::{BufWriter, Write};

pub struct TsvWriter {
    file: BufWriter<File>,
    /// 输出列类型：bind 阶段记下，读每个单元格时用来分派。
    column_types: Vec<TypeId>,
}

impl DuckCopyWriter for TsvWriter {
    fn open(path: &str, columns: &[LogicalType]) -> DuckResult<Self> {
        let file = File::create(path)
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot create {path}: {e}")))?;
        let column_types = columns
            .iter()
            .map(|column| unsafe { column.get_type_id() })
            .collect();
        Ok(Self { file: BufWriter::new(file), column_types })
    }

    fn finish(&mut self) -> DuckResult<()> {
        self.file
            .flush()
            .map_err(|e| duck_error(format!("dfn_copy_tsv: cannot flush: {e}")))
    }
}
```

## 四个阶段

DuckDB 通过四个回调驱动一次 `COPY`；`#[duck_copy_function]` 把四个都生成好，你只需要写 sink：

| 阶段 | 调用次数 | duckfn 做的事 | 你要写的 |
| --- | --- | --- | --- |
| bind | 一次 | 从 bind info 读出输出列的 `LogicalType`，存成 bind data。 | — |
| global init | 一次 | 调用 `DuckCopyWriter::open(path, columns)`。 | `open` |
| sink | 每个数据块一次 | 调用被标注的函数。 | 函数体 |
| finalize | 一次 | 调用 `DuckCopyWriter::finish()`。 | `finish` |

每个阶段都包在 `catch_unwind` 里，`Err` 与 panic 都会上报给 DuckDB，因此失败是让 `COPY` 中止，
而不是跨 FFI 边界展开。

## 读取数据块

COPY 函数是「收数据」的一方，取值要自己来。`DataChunk::reader(col)` 给某一列的
`VectorReader`，`is_valid(row)` 判断该格是否为 `NULL`，再按类型调用
`read_i64` / `read_str` / … 读取；用 `TypeId`（来自 `LogicalType::get_type_id`）做分派：

```rust
fn format_cell(reader: &VectorReader, row: usize, type_id: TypeId) -> DuckResult<Option<String>> {
    if !unsafe { reader.is_valid(row) } {
        return Ok(None); // SQL NULL
    }
    let text = match type_id {
        TypeId::BigInt => unsafe { reader.read_i64(row) }.to_string(),
        TypeId::Double => unsafe { reader.read_f64(row) }.to_string(),
        TypeId::Varchar => escape(unsafe { reader.read_str(row) }),
        other => {
            return Err(duck_error(format!(
                "dfn_copy_tsv: unsupported column type: {}",
                other.sql_name()
            )));
        }
    };
    Ok(Some(text))
}
```

遇到不支持的类型就返回 `Err` 才诚实：写出一份读不回来的文件，比 `COPY` 直接失败更糟。

:::note[拿不到列名]
bind info 只暴露输出列的**类型**（`column_count` / `column_type`），不暴露列名，所以格式没法据此写表头。
需要表头的话就从查询里拿，或者干脆写成无表头格式。
:::

## 生成的 item 与注册

和其它函数属性一样，`#[duck_copy_function]` 保留原函数，并生成一个同名模块：

| item | 用途 |
| --- | --- |
| `CopyFunctionImpl` | 适配层实现。 |
| `copy_function_builder()` | 配置好的 `CopyFunctionBuilder`（返回 `DuckResult`）。 |
| `copy_function_register(connection)` | 在连接上注册该格式。 |

默认 `auto_register = true` 时，扩展加载即注册，上面的 SQL 开箱可用；`auto_register = false`
时只生成这些 item：

```rust
#[duck_copy_function(auto_register = false)]
fn dfn_copy_manual(writer: &mut TsvWriter, chunk: &DataChunk) -> DuckResult<()> {
    /* … */
}

#[duck_custom_register]
fn dfn_copy_manual_register(c: &Connection) -> DuckResult<()> {
    dfn_copy_manual::copy_function_register(c)
}
```

## 限制

- 目前只实现 **`COPY ... TO`**。`COPY ... FROM` 需要 DuckDB 的 `copy_from` 表函数挂载点，
  而 `quack-rs` 0.16 还没暴露它；现阶段读自定义格式请用普通
  [表函数](./table-functions.md)加[替换扫描](./replacement-scans.md)（`SELECT * FROM 'orders.tsv'`）。
- 格式**没有自己的 bind 选项**。`COPY ... (FORMAT dfn_copy_tsv, HEADER true)` 会在你的代码跑之前
  被 binder 拒绝，因为该选项没有被声明；格式相关的设置只能放在查询或路径里。
- 拿不到列名（见上）。
- 用已存在的格式名注册会报错，所以不要和 `csv`、`parquet`、`json` 等重名。

## 源码与测试

- [`src/extension/functions/copy_function.rs`](https://github.com/shijianjs/duckfn/blob/main/src/extension/functions/copy_function.rs) —— `dfn_copy_tsv` 示例格式
- [`test/sql/functions/copy_function.test`](https://github.com/shijianjs/duckfn/blob/main/test/sql/functions/copy_function.test) —— 期望结果（用 `read_csv` 做往返校验）
- [`duckfn/src/functions/copy_function_adapter.rs`](https://github.com/shijianjs/duckfn/blob/main/duckfn/src/functions/copy_function_adapter.rs) —— 运行时侧

## 下一步

- [表函数](./table-functions.md)
- [替换扫描](./replacement-scans.md)
- [属性一览](./attributes.md)
