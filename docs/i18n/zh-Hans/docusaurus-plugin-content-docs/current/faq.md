---
title: 常见问题
sidebar_position: 8
description: 为什么不需要编译 DuckDB、与 duckdb-rs 的区别，以及最常见报错的排查方法。
---

# 常见问题

### 为什么不需要本地编译 DuckDB？

因为什么都不链接。`libduckdb-sys` 以 `loadable-extension` feature 编译：只使用 DuckDB 的头文件，
所有 API 函数都通过一张指针表解析，而这张表由宿主 DuckDB 在加载扩展时填好。代价是扩展与编译时所用的
DuckDB 版本绑定，详见[架构](./internals/architecture.md#4-分发)。

### 为什么加载扩展必须加 `-unsigned`？

扩展没有经过 DuckDB 签名，并且被标记为使用 DuckDB 的 unstable C API，因此除非允许未签名扩展，DuckDB 会拒绝加载。
命令行上加 `-unsigned` 即可。

### 与 `duckdb-rs` 有什么区别？

两者的方向相反。`duckdb-rs`（`duckdb` crate）是**客户端**绑定：把 DuckDB 数据库嵌进 Rust 程序里调用。
`duckfn` 产出的是**扩展**：一个共享库，由已存在的 DuckDB 进程用 `LOAD` 加载，从而给那个进程增加函数。

`duckfn` 基于 [`quack-rs`](https://crates.io/crates/quack-rs) 与 `libduckdb-sys`，而不是 `duckdb-rs`。

### 函数在 SQL 里找不到，怎么办？

按顺序检查：

1. `LOAD` 是否成功 —— 脚本里加载失败很容易被忽略。
2. 该函数是否写成了 `auto_register = false`。这类函数只有在有人调用它们的 builder 之后才存在，
   见[属性参考](./guide/attributes.md#自动注册与手动注册)。
3. 入口符号是否与扩展名一致：`duckfn_entrypoint!("my_ext")` 导出 `my_ext_init_c_api`。
   不一致时扩展能加载，但什么都不会注册。
4. 用了 `overloads_name = "…"` 时，各分支函数**不会**以自身名字注册，只有函数集名存在。
5. 参数或返回类型是否是宏接受的形式。返回形态不对会编译报错，但**参数组合**不受支持时表现为
   `No function matches the given name and argument types`。

### 为什么常量 `NULL` 没有进入函数体？

DuckDB 会在 bind 阶段折叠常量表达式，所以 `NULL::INTEGER` 与 `NULL::INTEGER + 0` 根本到不了回调。
需要让函数体看到它们就设置 `special_null_handling = true`；列里的值本来就会以 `None` 传入。
详见[标量函数](./guide/scalar-functions.md#special_null_handling)。

### 为什么 `Vec<i32>` 里出现 `NULL`，标量函数和表函数表现不一样？

标量函数逐行读取参数，`NULL` 元素会让整行变成 `NULL` —— 想保留元素级 `NULL` 就声明 `Vec<Option<i32>>`。
表函数的参数是**bind** 参数，从 `Value` 读取，此时 `Vec<T>` 里的 `NULL` 元素会直接报错
（`Vec<T> value is None`）。同样，预期出现 `NULL` 元素时请用 `Option` 版本。

### 为什么 `ARRAY` 不能作为表函数参数？

DuckDB 无法把 `Value` 绑定到 `ARRAY` 类型上，会报 `Bind value to array type is not supported`。
数组作为标量函数参数、列表元素、结构体字段都没有问题。

### 函数可以返回 `Result<T, ExtensionError>` 吗？

不可以。标量类函数接受的形态是 `T`、`Option<T>`、`DuckOptionResult<T>`；成功时请返回
`Ok(Some(value))` 而不是 `Ok(value)`。

### 怎么返回 `STRUCT`？

在要返回的类型上派生 `DuckStruct`：

```rust
#[derive(Clone, Debug, DuckStruct)]
pub struct Point {
    x: i64,
    y: i64,
}
```

它的字段在任意层级都会成为结构体字段，包括位于列表、映射、数组内部时。

### duckfn 不支持的类型，我能自己加上吗？

可以。`DuckValueType` 是公开且未封闭的 trait，在 `duckfn` 之外为自己的类型实现它即可 —— 如果只是复用已有的物理表示，
三个方法就够了；quack-rs 没有访问器的类型，`DuckValueReader` / `DuckValueWriter` 里也暴露了裸的 DuckDB 向量。
见[自定义类型](./guide/custom-types.md)。

另外，有些 DuckDB 1.5 的类型只需要开启一个 feature：`TIME_NS` 就由 `duckdb-1-5` 提供。

### 为什么标量函数的命名参数不生效？

duckfn 按位置注册标量函数，因此 DuckDB 按书写顺序绑定值、忽略名字 —— `f(b := 2, a := 1)` 会把 `2` 传给第一个参数。
`named_param_from` 是表函数专用的键，其它属性宏会直接拒绝它。

### 怎么测试扩展？

用 `test/sql/` 下的 sqllogictest 文件，通过 `make test`（`just test`）运行。文件中用
`require duckfn`
声明依赖的扩展，然后成对给出语句与期望输出。见[贡献指南](./contributing.md#测试)。

### 扩展能加载，但调用时报版本错误

扩展针对特定的 DuckDB 版本编译（`TARGET_DUCKDB_VERSION`，当前为 v1.5.5），且使用 unstable C API，
因此只能与兼容版本配合使用。请用匹配的版本加载，或针对你手上的版本重新构建。

### 工具链层面的问题去哪看？

与目录结构有关的 —— 几个 crate root、嵌套模块后的 `error[E0583]`、IDE 对独立 wasm root 标红 ——
在[项目结构约定](./getting-started/project-structure.md)；官方 CI 的 WebAssembly 作业锁定 Rust 1.86，
以及几个值得知道的上游 bug，在[问题排查](./troubleshooting.md)。
