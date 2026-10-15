---
title: 在扩展里执行 SQL
sidebar_position: 13
description: 通过 duckfn 捕获的自有连接在回调里跑 SQL，并把结果当作动态表处理 —— 运行期才知道的表名用标识符转义，值用参数绑定。
---

# 在扩展里执行 SQL

DuckDB 的 C API 只在部分回调里交出**客户端上下文**，而客户端上下文里**没有连接**：不存在
`duckdb_client_context_get_connection`。行处理函数、表函数的 `scan`、聚合的 `finalize` 都拿不到
连接，所以需要跑 SQL 的回调本来没有直达的路。

注册期是唯一的窗口，而 DuckDB 交给入口点的那条连接是**借来的**：注册一返回就被断开。`duckfn`
因此在注册期打开**自己的一条长连接** —— 就是[文件系统访问](./file-system.md)里那条 —— 而
`duckfn::sql` 是它的 SQL 那一面：跑语句、跑查询，并把行以运行期**动态值**的形式取回来。

::::note[只用稳定版 C API，不需要 `duckdb-1-5`]

`duckfn::sql` 建在注册期捕获的那条连接上，但它发出的每一个调用都落在 C API 的**稳定区** ——
`duckdb_connect`、预处理 / 绑定 / 取块、逻辑类型。所以它不需要 `duckdb-1-5`，只开这一个 feature
就够：

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["sql"] }
```

这件事影响的不只是构建，还有产物本身：保持 `USE_UNSTABLE_C_API=0` 并以 `v1.2.0` 作为 floor，一份
二进制就能服务 DuckDB 1.3 到 2.0（见[版本号与 ABI](../duckdb-versions/c-api-and-abi.md)）。还想顺带
要宿主文件系统就再加 `duck_vfs`：它建在同一条连接上，但既带来
[`duckfn::duck_vfs`](./file-system.md)，又要 `duckdb-1-5` —— 文件系统那一半**确实**在不稳定区，必须与
引擎小版本对齐。两个 feature 互不依赖，只共用那条连接与它的锁。

::::

无论开哪一个，这类 feature 都有副作用，动手前值得先读[代价与限制](#代价与限制)。

## 执行语句

`duckfn::sql::execute` 按顺序执行一条或多条 `;` 分隔的语句，返回影响行数；`CREATE` / `SET`
这类返回 0：

```rust
use duckfn::sql;

let created = sql::execute("CREATE OR REPLACE TABLE runs(id BIGINT, note VARCHAR)")?;  // 0
let inserted = sql::execute("INSERT INTO runs VALUES (1, 'a'), (2, 'b')")?;             // 2
let deleted = sql::execute("DELETE FROM runs WHERE id > 1")?;                           // 1
```

返回的是**产出结果的那一条**的影响行数 —— 第一条产出行的语句，若都不产出行则是最后一条，所以
`"SELECT 1; INSERT …"` 报的是 `INSERT`，而全是 `SELECT` 时报第一条。某条失败即整次调用失败，
但它之前的语句已经执行并提交（除非你开了显式事务）。

## 执行查询

`duckfn::sql::query` 跑一条查询并返回 `DuckQueryResult` —— 一份 schema 加若干行，两者都取自
结果本身：

```rust
use duckfn::{DuckDynamicValue, sql};

let result = sql::query("SELECT 42::BIGINT AS answer, [1, 2, 3] AS tags")?;
assert_eq!(result.len(), 1);
assert_eq!(
    result.cell(0, "answer")?,
    Some(&DuckDynamicValue::BigInt(42)),
);
```

**列名与列类型来自 SQL**，而不是你的 Rust 代码：它们从结果里读出来
（`duckdb_column_name` / `duckdb_column_logical_type`），再变成 `DuckTypeDesc` —— 也就是
[动态列表函数](./table-functions.md)用的那份运行期描述。于是表函数可以在 `bind` 阶段跑一句调用方
给的 SQL，把列原样交回：

```rust
use duckfn::{DuckDynamicTable, DuckResult, duck_table_function, sql};

#[duck_table_function(dynamic_columns = true)]
fn my_query(statement: String) -> DuckResult<DuckDynamicTable> {
    Ok(sql::query(&statement)?.into_table())
}
```

能往返的形状就是 `DuckTypeDesc` 能描述的那些：标量、`DECIMAL`、`LIST`、`STRUCT`、`MAP`，可任意
嵌套。`ENUM`、`ARRAY`、`UNION`、`BIT` 没有稳定的重建方式，遇到这类列就报错（消息里带列名），
而不是给出一份不完整的描述。

## 处理结果

`DuckQueryResult` 是**已物化**的：`query` 边读边把每个数据块读成 `DuckDynamicRow`，因为
`duckdb_result` 的句柄不能带出那条自有连接。于是整个结果就是一个可以随便索引的普通值：

| 调用 | 作用 |
| --- | --- |
| `schema()` / `column_count()` / `column_names()` | 输出列（顺序即列顺序） |
| `column_index(name)` | 按列名找下标（大小写敏感） |
| `len()` / `is_empty()` / `rows()` / `row(i)` | 行 |
| `cell(row, name)` | 按**列名**取一个单元格；`Ok(None)` 是 SQL NULL，列名不存在是 `Err` |
| `single_row()` | 唯一一行；行数不是 1 时返回错误 |
| `single_value()` | 唯一一个值（单行单列）；`Ok(None)` 是 SQL NULL |
| `single_value_text()` | 同一个值渲染成文本 |
| `into_table()` | 变成 `DuckDynamicTable`，可直接从表函数的 `bind` 返回 |
| `into_parts()` / `into_rows()` | 单独取 schema / 行 |

标量函数只能返回固定的 Rust 类型，所以「把 `SELECT` 的结果交回 SQL」通常就用
`single_value_text()`：

```rust
#[duck_scalar_function(volatile = true)]
fn dfn_sql_scalar(statement: String) -> DuckOptionResult<String> {
    sql::query(&statement)?.single_value_text()
}
```

:::note[它渲染的是物理值]

`single_value_text()` 走的是 `DuckDynamicValue::to_text`，所以 `DECIMAL` 输出的是**未缩放整数**
（`1.5::DECIMAL(4,1)` → `"15"`），`DATE` / `TIMESTAMP*` / `UUID` / `INTERVAL` 输出的也是各自的
物理整数 —— 这是一份渲染，不是无歧义的往返编码。要拿值本身请用 `single_value()` 并对
`DuckDynamicValue` 做匹配。

:::

## 用运行期才知道的内容拼 SQL

表名、路径、用户输入都只在运行期存在，而进来只有两条路：

**绑定值**（`query_with` / `execute_with`）：值走 `?` 占位，SQL 文本里永远不会出现它。

```rust
use duckfn::{DuckDynamicValue, sql};

let result = sql::query_with(
    "SELECT ?::BIGINT * 2",
    &[DuckDynamicValue::BigInt(21)],
)?;
assert_eq!(result.single_value()?, Some(DuckDynamicValue::BigInt(42)));
```

每一种标量 `DuckDynamicValue` 都能绑（值以 `duckdb_value` 交给 DuckDB，所以 `UUID` /
`TIMESTAMP_S|MS|NS` / `TIME WITH TIME ZONE` 也可以，`TIME` / `TIMESTAMP*` / `DECIMAL` 在调用前
会做范围校验）。`LIST` / `STRUCT` / `MAP` **不能**绑：它们的嵌套 NULL 没有 `duckdb_value` 形式。
请在 SQL 里就地构造（`list_value`、`struct_pack`、`map`），或者只绑它们的标量部分。

**转义标识符**（`quote_identifier`）：标识符不能是 `?` 占位 —— 它是语句结构的一部分 —— 所以运行期
的表名只能拼进文本，而这里就是那道转义边界：

```rust
let statement = format!("SELECT count(*) FROM {}", sql::quote_identifier(table_name));
let count = sql::query(&statement)?.single_value()?;
```

`quote_identifier` 把内部的 `"` 双写、首尾补引号，于是 `dfn_t` 变成 `"dfn_t"`，形如 `a"b` 的名字
也跳不出引号。`quote_literal` 对单引号字符串字面量做同样的事 —— 只在参数占位用不上的地方用它
（`SET` 的取值、`INSTALL '...'`），其余一律优先绑定。

## 一个完整例子

仓库的示例扩展把两半都接到了 SQL 上（`test/extension/functions/sql_exec.rs`）：

```sql
SELECT dfn_sql_exec('CREATE OR REPLACE TABLE dfn_sql_demo(i INTEGER, s VARCHAR)');
SELECT dfn_sql_exec('INSERT INTO dfn_sql_demo VALUES (1, ''a''), (2, ''b''), (3, ''c'')');  -- 3

SELECT * FROM dfn_sql_query('SELECT 42::BIGINT AS answer, [1, 2] AS tags');
SELECT dfn_sql_scalar('SELECT 6 * 7');                                   -- 42
SELECT dfn_sql_scalar_arg('SELECT ?::BIGINT * 2', 21);                   -- 42
SELECT dfn_sql_table_count('dfn_sql_demo');                              -- 3
SELECT dfn_sql_has_table('dfn_sql_demo');                                -- true
```

| 函数 | 演示的是 |
| --- | --- |
| `dfn_sql_exec` | `execute` —— 跑语句、报影响行数 |
| `dfn_sql_scalar` | `query` + `single_value_text` —— 把单个值当文本回给 SQL |
| `dfn_sql_scalar_arg` | `query_with` —— 绑定参数（`?`），不拼文本 |
| `dfn_sql_table_count` | `quote_identifier` —— 运行期的表名安全拼接 |
| `dfn_sql_has_table` | 把字符串值绑到 `information_schema` 的查询上 |
| `dfn_sql_query` | `query` + `into_table` —— 列完全来自 SQL 自己 |

`test/sql/functions/sql_exec.test` 覆盖了全部函数，包括块边界（`range(5000)` 的结果经动态通路读
回）、NULL 单元格、空结果与各条错误路径。

## 代价与限制

- **语句跑在另一条连接上。** 这正是重点 —— 它是回调唯一能够到的连接 —— 也意味着外层查询保持自己
  的快照，而你的语句在它旁边执行。同理，连接级 `SET` 不共享：经 `duckfn::sql` 跑 `SET` 只影响
  这条自有连接，不影响调用方。
- **结果整份物化。** `query` 在返回前把所有块读进内存。结果大到放不下时，请改走「手写适配器 +
  自持连接」那条路。
- **两个 feature 都开时与 `duckfn::duck_vfs` 共用同一把锁。** 一次调用从头持锁到尾，所以长查询会
  挡住文件读写，而且**同一线程里嵌套取用会死锁** —— 开了 `duck_vfs` 时不要在
  `with_file_system` 闭包里调 `duckfn::sql`，反之亦然；其它线程的并发调用只是排队。
- **进程级只有一条连接，以首个完成注册的实例为准。** 同一进程里存在多个数据库实例时，用的是先注册
  那个实例的连接。
- **不能在「同一线程里同步执行 DuckDB」的运行时里用。** 注册期就在当前线程上打开连接并重入引擎，
  没有别的线程能推进的运行时因此死锁，表现为扩展 `LOAD` 卡住。已知一处是 DuckDB-Wasm 的 Node
  **blocking** 绑定；浏览器、原生 CLI 与 Node 的 worker 模式都正常。

## 错误

`duckfn::sql` 会给底层错误加上「哪一步」的前缀，便于断言与查日志：

| 前缀 | 来自 |
| --- | --- |
| `duckfn::sql::execute: …` | `execute` / `execute_with` |
| `duckfn::sql::query: …` | `query` / `query_with`，以及它们内部的数据块读取 |
| `duckfn::sql: parameter N: …` | 绑定第 `N` 个参数（从 1 开始） |
| `duckfn::sql: the statement has N parameter(s) but M value(s) were given` | 参数个数与语句不符 |
| `duckfn::sql: column I (\`name\`) cannot be turned into a dynamic column: …` | 动态通路无法表达的列类型 |

标量函数返回 `Err` 会让整条查询失败，所以内层查询的问题会被报出来，而不是被悄悄跳过。
