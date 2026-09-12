# duckfn

[English](README.md) | [简体中文](README.zh-CN.md)

**用纯 Rust 写 DuckDB 扩展。**

`duckfn` 是一个基于 DuckDB C Extension API 的 Rust 框架。借助一个属性宏，就能把普通的 Rust
函数变成 DuckDB 的**标量函数**、**聚合函数**、**表函数**、SQL 宏，或嵌套类型 —— 无需 C/C++
胶水代码，也无需在本地编译 DuckDB。

- 仓库地址：<https://github.com/shijianjs/duckfn>
- 已发布 crate：[`duckfn`](https://crates.io/crates/duckfn) · [`duckfn-macro`](https://crates.io/crates/duckfn-macro)
- 协议：[MIT](LICENSE)

> 状态：早期 / 实验性，`1.0` 之前 API 可能变化。

## 仓库结构

本仓库是一个 Cargo workspace：

| 路径 | 说明 | 是否发布到 crates.io |
| --- | --- | --- |
| [`duckfn/`](duckfn/) | 运行时框架：trait、类型适配、函数注册。 | 是 |
| [`duckfn-macro/`](duckfn-macro/) | 过程宏：`#[duck_scalar_function]`、`#[derive(DuckStruct)]` 等。 | 是 |
| `/`（`rusty_quack`） | 使用 `duckfn` 编写的示例扩展，放在根目录是为了复用官方多平台 CI。 | 否，仅作示例 |

## 快速开始

```rust
use duckfn::{duck_error, duck_scalar_function, duckfn_entrypoint, DuckOptionResult};

/// ```sql
/// SELECT double_it(21);    -- 42
/// SELECT double_it(NULL);  -- NULL
/// SELECT double_it(13);    -- error: unlucky input
/// ```
#[duck_scalar_function]
pub fn double_it(v: Option<i64>) -> DuckOptionResult<i64> {
    if v == Some(13) {
        return Err(duck_error("unlucky input"));
    }
    Ok(v.map(|x| x * 2))
}

// 生成 DuckDB 扩展入口（扩展名必须全小写、仅含下划线）
duckfn_entrypoint!("my_ext");
```

`#[duck_scalar_function]` 会自动生成 DuckDB 包装层、逻辑类型，并默认通过 `inventory` 完成注册。
`duckfn_entrypoint!` 负责导出 DuckDB 加载扩展时查找的 `*_init_c_api` 符号。

## 可用宏

| 宏 | 作用 |
| --- | --- |
| `#[duck_scalar_function]` | 注册标量函数。 |
| `#[duck_aggregate_function]` | 注册聚合函数。 |
| `#[duck_table_function]` | 注册表函数。 |
| `#[duck_sql_macro]` | 注册 SQL 宏。 |
| `#[duck_custom_register]` | 手动注册 builder，签名为 `fn(&Connection) -> DuckResult<()>`。 |
| `#[derive(DuckStruct)]` | 把结构体映射为 DuckDB `STRUCT`。 |
| `duckfn_entrypoint!("name")` | 生成扩展入口。 |

常用参数：

- `auto_register = false` —— 只生成 builder（`scalar_function_builder()`、
  `scalar_overload_builder()` 等）不自动注册，配合 `#[duck_custom_register]` 使用。
- `named_param_from = "field"` —— 表函数命名参数从哪个字段开始。

## 类型映射

| DuckDB | Rust |
| --- | --- |
| `BOOLEAN` | `bool` |
| `TINYINT` / `SMALLINT` / `INTEGER` / `BIGINT` | `i8` / `i16` / `i32` / `i64` |
| `UTINYINT` / `USMALLINT` / `UINTEGER` / `UBIGINT` | `u8` / `u16` / `u32` / `u64` |
| `HUGEINT` / `UHUGEINT` | `i128` / `u128` |
| `FLOAT` / `DOUBLE` | `f32` / `f64` |
| `VARCHAR` | `String` |
| `NULL` | `Option<T>` |
| `LIST(T)` | `Vec<T>`，支持嵌套（`Vec<Option<Vec<Option<T>>>>` 等） |
| `MAP(K, V)` | `IndexMap<K, V>` |
| `ARRAY(T, N)` | `[T; N]`（`DuckArray`）/ `[Option<T>; N]`（`DuckOptionArray`） |
| `STRUCT(...)` | `#[derive(DuckStruct)]`，支持嵌套 struct 和 list |

可空性由 Rust 签名决定：非 `Option` 参数遇到 `NULL` 输入时整行短路为 `NULL`（函数体不执行）；
`Option<T>` 参数则把 `NULL` 读成 `None` 交给函数自己决定语义。

## 错误处理与 panic

返回 `DuckOptionResult<T>`（即 `Result<Option<T>, ExtensionError>`）可以输出 `NULL`，或用
`duck_error("...")` 让整条查询失败。函数体内的 panic 会被捕获并转换成 DuckDB 错误，而不会跨
FFI 边界展开。

## 运行示例扩展

仓库根目录是一个功能完整的示例扩展（`rusty_quack`），覆盖了全部能力：

```shell
make configure
make debug
duckdb -unsigned -c "
LOAD './build/debug/extension/rusty_quack/rusty_quack.duckdb_extension';
SELECT rusty_echo('Jane');
"
```

大量可运行的 SQL 示例见 [`demo.sh`](demo.sh)，完整库文档见
[`duckfn/README.zh-CN.md`](duckfn/README.zh-CN.md)。

## 协议

基于 [MIT 协议](LICENSE) 开源。
