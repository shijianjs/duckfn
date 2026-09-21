---
title: 文件系统访问
sidebar_position: 12
description: 在聚合函数与任意其它回调里，通过 DuckDB 的虚拟文件系统读写文件 —— 包括 s3:// 与 http(s)://。
---

# 文件系统访问

DuckDB 的 C API 只在部分回调里交出**客户端上下文**：标量函数在 `bind` / `init` 回调里能拿到，
表函数有 `duckdb_table_function_get_client_context`，`COPY TO` 的四个回调各有一个入口；
**聚合函数一个都没有** —— 既没有 bind 回调，也没有
`duckdb_aggregate_function_get_client_context`。行处理函数与 `finalize` 拿不到
`ClientContext`，没有它也就没有 `FileSystem`。

注册期是唯一的窗口，而 DuckDB 交给入口点的那条连接是**借来的**：注册一返回就被断开。
保存那个句柄 —— 或者由它派生的 `ClientContext` / `FileSystem` —— 只会得到一个悬空引用：
这三者在 C API 里都是引用语义，`duckdb_destroy_client_context` / `duckdb_destroy_file_system`
只是 `delete` 掉包装对象。

`duckfn` 因此在注册期打开**自己的一条长连接**，把其余细节藏在三个函数后面，于是任何回调都能
访问 DuckDB 的文件系统 —— `s3://` 与 `http(s)://`（走 `httpfs`）、内存文件系统与本地磁盘都是
同一条通路，而不是退化成只能看见本地磁盘的 `std::fs`。

:::note[DuckDB 1.5.0+]

文件系统访问来自 DuckDB 1.5.0 的 C API，因此需要 `duckfn` 的 `duckdb-1-5` feature：

```toml
duckfn = { version = "{{DUCKFN_VERSION}}", features = ["duckdb-1-5"] }
```
:::

## duckfn 做了什么

扩展加载时，`duckfn::register_all_duckfn`（`duckfn_entrypoint!` 指向的那个函数）先记下拿到的
database 句柄，并由它打开一条 `OwnedConnection`，把结果存进进程级静态变量。这条连接持有数据库
实例的 `shared_ptr`，所以能活过扩展加载；而静态变量永不析构 —— 这正是由它派生的句柄一直有效的
原因。

之后每次取用都会从这条连接现造一对 `ClientContext` → `FileSystem`，并在 guard 释放时一起收尾。
没有句柄逸出，也不需要为每个函数单独注册：宏写的聚合与手写适配器用的是同一个调用。

捕获是 best-effort 的：打开连接失败时注册照样成功，只是之后取用会报出记录下来的原因。

## 读一个文件

```rust
use std::ffi::CString;

use duckfn::{
    DuckAggregateState, DuckResult, ErrorData, FileOpenOptions, duck_aggregate_function, duck_error,
};

#[derive(Default, Debug, Clone)]
struct FileSizeState {
    total: i64,
}

impl DuckAggregateState for FileSizeState {
    type Output = i64;

    fn simple_combine(&mut self, other: &Self) {
        self.total += other.total;
    }

    fn simple_result(&self) -> Self::Output {
        self.total
    }
}

#[duck_aggregate_function]
fn dfn_agg_file_size(path: String, state: &mut FileSizeState) -> DuckResult<()> {
    let path = CString::new(path)
        .map_err(|_| duck_error("dfn_agg_file_size: path contains a NUL byte"))?;
    state.total += duckfn::with_file_system(|fs| {
        let handle = fs
            .open(&path, &FileOpenOptions::read_only())
            .map_err(file_error)?;
        let size = handle.size().map_err(file_error)?;
        i64::try_from(size).map_err(|_| duck_error("file is larger than BIGINT"))
    })?;
    Ok(())
}

fn file_error(error: ErrorData) -> quack_rs::error::ExtensionError {
    duck_error(error.message().unwrap_or_else(|| "file system error".to_string()))
}
```

```sql
SELECT dfn_agg_file_size(path) FROM (VALUES ('a.csv'), ('b.csv')) t(path);
```

一共三个入口：

| 入口 | 什么时候用 |
| --- | --- |
| `duckfn::with_file_system(\|fs\| …)` | 在一个闭包里读、什么都不逸出 —— 最不容易写错 |
| `duckfn::file_system()` | 你想自己持有 guard；它实现了 `Deref<Target = FileSystem>`，可以直接 `open()` |
| `duckfn::client_context()` | 需要连接级配置或连接 ID（注意 `catalog` 需要活跃事务，空闲连接上返回 `None`） |

`FileSystem::open` 收 `&CStr`，返回的 `FileHandle` 提供 `read` / `read_exact` / `read_to_end` /
`write` / `write_all` / `seek` / `tell` / `size` / `sync` / `close`，drop 时自动关闭。`duckfn` 已
再导出 `FileSystem` / `FileHandle` / `FileOpenOptions` / `FileFlag` / `ClientContext` / `ErrorData`，
因此下游不必直接依赖 `quack-rs`。

## 便捷读写（`duckfn::file`）

上面那套是底层形态：自己挑打开选项、自己持有句柄、自己推字节。日常用起来啰嗦，所以
`duckfn::file` 提供了一套 Hutool `FileUtil` 风格的一行式接口：

| 调用 | 作用 |
| --- | --- |
| `file::read(path)` | 整个文件读成 `Vec<u8>` |
| `file::read_string(path)` | 按 UTF-8 读成 `String`（非法字节报错） |
| `file::read_string_lossy(path)` | 同上，非法字节换成 `U+FFFD` |
| `file::read_lines(path)` | 按行读（`\n` 分行、行尾 `\r` 去掉、末尾换行不产生空行） |
| `file::write(path, bytes)` / `file::write_string(path, text)` | 用这些字节替换文件内容 |
| `file::write_with(path, bytes, mode)` / `file::write_string_with(path, text, mode)` | 同上，显式指定 `WriteMode` |
| `file::append(path, bytes)` / `file::append_string(path, text)` | 追加；文件不存在则创建 |
| `file::size(path)` / `file::exists(path)` | 字节数 / 是否存在 |

有意思的是 `WriteMode`：

| 模式 | 语义 |
| --- | --- |
| `Replace`（默认） | 文件内容**精确**等于这次写进去的字节 —— 旧文件更长也一样。C API 没有 truncate 这件事就藏在这里：旧文件更长时先用一条零行 `COPY ... TO` 把它清零，再写正文。 |
| `FailIfExists` | 文件已存在就报错，且不改动它。存在性由显式检查决定，另外叠上 `EXCLUSIVE_CREATE` 作为并发下的保险 —— DuckDB 只在 POSIX 本地文件系统上把它真正落成 `O_EXCL`，Windows 分支不处理它（缺文件时甚至会报「找不到文件」），所以光靠这个 flag 不够。 |
| `Append` | 追加到末尾；文件不存在则创建。 |

```rust
use duckfn::file::{self, WriteMode};

file::write_string("report.html", render())?;                        // 覆盖
file::append_string("report.log", "one more line\n")?;               // 追加
file::write_string_with("once.txt", "x", WriteMode::FailIfExists)?;  // 已存在就报错

let text = file::read_string("report.html")?;
let lines = file::read_lines("report.log")?;
let bytes = file::size("report.html")?;
```

这一层下面的一切 —— 共用连接、C 字符串转换、清零用的 `COPY` —— 都是实现细节；调用方依赖的是
**行为**：将来 DuckDB 的 C API 支持 truncate 了，也只需改 `duckfn::file`。

没有 `delete`：C API 既没有 remove 也没有 move，DuckDB 也没有 `remove_file` 函数 —— 想「清空」
就用空内容覆盖一次。每次调用都会自己取一次共用连接，所以不要在 `with_file_system` 的闭包里调
`file::*`（会死锁），并发调用之间也是串行的。

## 并发与开销

guard 持有那条自有连接上的互斥锁：

- **同一线程里不要嵌套取用** —— 在已经持有 `file_system()` 的闭包里再调 `with_file_system` 会
  死锁。要读多个文件就放进同一次 `with_file_system`。
- **取用之间是串行的。** 并行聚合时每个工作线程都经过这一条连接，所以要放在便宜的位置：在
  `finalize` 里（每组一次），或每组首行取一次并把结果缓存进自己的状态。逐行取用对
  `open` + `size` 这种小操作没问题，但一次很慢的 S3 读应该放到 `finalize`。
- 文件系统本身是 DuckDB 的实例级 VFS，多线程读是安全的；被串行化的只是「取用」这一步。
  `FileHandle` **不可**共享 —— 留在打开它的那个线程里，用完让它自己 drop。

## 错误

`FileSystem::open`、`FileHandle` 的各操作与 `FileSystem::error_data` 都返回 quack-rs 的
`ErrorData`。用 `duck_error(error.message()…)` 把它转成查询错误；聚合的行处理函数返回 `Err` 会
让整条查询失败（适配层通过 `AggregateFunctionInfo::set_error` 报出），而不是静默跳过这一行。

## 局限

- **进程级只有一份，首个实例胜出。** 同一进程存在多个数据库实例时，用的是先完成注册那个实例的
  VFS。需要按实例隔离时，手写适配器并把自有连接挂在函数的附加数据里
  （`duckfn::DuckExtraInfo` + `quack_rs::query::OwnedConnection`），框架不必参与。
- **自有连接的 `FileOpener` 与查询连接不是同一个。** 实例级配置与 secrets 生效，但连接级 `SET`
  不保证等价。
- **需要 DuckDB 1.5.0+ 与 `duckdb-1-5` feature**，否则这些函数不存在。

示例扩展在聚合函数里用了它（`src/extension/functions/file_system.rs` 的 `dfn_agg_file_size`），
`test/sql/functions/file_system.test` 用 DuckDB 自己的 `read_blob` 对照校验结果；`duckfn::file`
 这一层（往返读写、覆盖更长的旧文件、追加、已存在就报错、非法 UTF-8、按行读）由
`test/sql/functions/file_util.test` 覆盖。
