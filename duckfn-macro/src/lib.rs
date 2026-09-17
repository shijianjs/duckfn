//! `duckfn-macro`：`duckfn` 的过程宏实现。
//!
//! 本 crate 提供属性宏、derive 宏和函数式宏，把一个普通的 Rust 函数/结构体包装成
//! 可注册到 DuckDB 的扩展对象；运行时依赖在 `duckfn` crate 中。
//!
//! `duckfn-macro`: the procedural macros behind `duckfn`. This crate provides the attribute,
//! derive and function-like macros that wrap an ordinary Rust function or struct into an object
//! registerable with DuckDB; the runtime lives in the `duckfn` crate.

/// `#[duck_scalar_function]` / `#[duck_aggregate_function]` / `#[duck_table_function]` 等
/// 属性宏共同使用的代码生成逻辑。
///
/// Shared code generation for the `#[duck_scalar_function]` / `#[duck_aggregate_function]` /
/// `#[duck_table_function]` and other attribute macros.
mod duck_function;
/// `#[derive(DuckStruct)]` 的实现：把具名结构体映射为 DuckDB `STRUCT`。
///
/// Implementation of `#[derive(DuckStruct)]`: maps a named struct onto a DuckDB `STRUCT`.
mod duck_struct_derive;
/// `#[derive(DuckEnum)]` 的实现：把只有单元变体的枚举映射为 DuckDB `ENUM`。
///
/// Implementation of `#[derive(DuckEnum)]`: maps a unit-variant-only enum onto a DuckDB `ENUM`.
mod duck_enum_derive;
/// 过程宏内部的 token/类型解析工具。
///
/// Token and type parsing helpers used inside the procedural macros.
pub(crate) mod macro_utils;
/// `duckfn_entrypoint!` 的实现：生成扩展入口符号。
///
/// Implementation of `duckfn_entrypoint!`: generates the extension entry-point symbol.
mod entrypoint;
/// 各属性宏共用的参数解析（`auto_register`、`named_param_from`、`overloads_name` 等）。
///
/// Argument parsing shared by the attribute macros (`auto_register`, `named_param_from`,
/// `overloads_name`, ...).
mod attr_args;
/// `duck_sql_macro_files!` 的实现：把若干 `.sql` 文件编译期内联并注册。
///
/// Implementation of `duck_sql_macro_files!`: inlines several `.sql` files at compile time and
/// registers them.
mod sql_macro_files;

use crate::macro_utils::handle_token_stream2_result;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};
use crate::attr_args::handle_duck_function;

/// 把一个具名结构体映射成 DuckDB `STRUCT`（嵌套 LIST / MAP / ARRAY / STRUCT 均支持）。
///
/// 生成的实现同时让该结构体可用于：
///
/// - 标量函数的参数（作为一行多列）；
/// - 表函数的输出行；
/// - 表函数的 bind 参数（用 `#[duck(named_param_from = "字段名")]` 指定命名参数起点）；
/// - 单独作为一个 STRUCT 列值读写。
///
/// 字段的可空性完全由字段类型表达：写 `Option<T>` 就是可空（NULL 取到 `None`），
/// 写 `T` 就是 NOT NULL（NULL 会让整行/整个结构变成 NULL，bind 参数处则报错）。
///
/// 还可以用 `#[duck(sql_name = "ticket", create_type = true)]` 在加载期把这个结构体建成 catalog 里的
/// 命名类型：`CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT(...)`。字段类型不是宏写死的 —— 注册时把整
/// 个结构体的 `LogicalType` 递归渲染成 SQL（走 DuckDB 的类型 introspection），因此枚举、嵌套结构体、
/// LIST / ARRAY / MAP、DECIMAL 与手写的自定义字段类型都能自动带上；语句幂等，`LOAD` 多次不会报错。
/// 把 `true` 换成 `"print"` 则不建类型：DDL 被收进队列，等全部注册项跑完由入口点一次性打印出来，
/// 方便先看看宏会生成什么。
///
/// Maps a named struct onto a DuckDB `STRUCT` (nested LIST / MAP / ARRAY / STRUCT are
/// supported). The generated implementations let the struct be used as scalar-function
/// arguments (a row of columns), as table-function output rows, as table-function bind
/// parameters (use `#[duck(named_param_from = "field")]` to mark where named parameters start)
/// and as a standalone STRUCT column value.
///
/// Field nullability is expressed purely by the field type: `Option<T>` is nullable (a NULL
/// yields `None`) while `T` is NOT NULL (a NULL turns the whole row / struct into NULL, or, for
/// a bind argument, into an error).
///
/// `#[duck(sql_name = "ticket", create_type = true)]` additionally creates a named type in the
/// catalog at load time (`CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT(...)`). The field types are
/// not hard-coded: the struct's `LogicalType` is rendered recursively through DuckDB's own type
/// introspection, so enums, nested structs, LIST / ARRAY / MAP, DECIMAL and hand-written custom
/// field types all come along. The statement is idempotent. Replacing `true` with `"print"` does not
/// create the type: the DDL is queued and the entry point prints the whole batch once every
/// registration has run, which is handy for previewing what the macro would run.
#[proc_macro_derive(DuckStruct, attributes(duck))]
pub fn duck_struct_derive(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = duck_struct_derive::duck_struct_derive(derive_input);
    handle_token_stream2_result(result)
}

/// 把「只有单元变体的 Rust enum」映射成 DuckDB `ENUM`。
///
/// 生成的实现让该枚举可直接用于函数参数/返回值、`STRUCT` 字段与容器元素，逻辑类型是带字典的
/// `ENUM('a', 'b', ...)`（字典就是变体的声明顺序）。ENUM 的规则很固定，所以这里生成完整实现，
/// 而不是像 `#[derive(DuckStruct)]` 那样只生成 trait 实现再靠 blanket impl 兜底 ——
/// `Option<T>` 与结构体已经各占一个 blanket impl，再加第三个会冲突（E0119）。
///
/// ```ignore
/// #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]
/// #[duck(rename_all = "lowercase", sql_name = "priority", create_type = true)]
/// pub enum Priority {
///     #[default]
///     Low,
///     Medium,
///     High,
/// }
/// // SQL 侧：ENUM('low', 'medium', 'high')，并在加载时建好 `priority` 类型
/// ```
///
/// `#[duck(...)]` 参数：
///
/// - `rename_all = "..."`：变体名 → SQL 标签的命名规则（`lowercase` / `UPPERCASE` /
///   `snake_case` / `SCREAMING_SNAKE_CASE` / `camelCase` / `PascalCase` / `kebab-case` /
///   `SCREAMING-KEBAB-CASE` / `verbatim`），默认原样使用变体名；
/// - `sql_name = "..."`：SQL 侧类型名，默认是类型名的小写蛇形（`Priority` -> `priority`）；
/// - `create_type`：`true` 在加载期执行 `CREATE TYPE IF NOT EXISTS <sql_name> AS ENUM (...)`，
///   `"print"` 则不建类型、只把同一条 DDL 收进队列，由入口点在全部注册跑完后一次性打印；
///   默认 `false` 什么都不做。语句幂等（重复 `LOAD` 不报错），也不会覆盖已存在的同名类型，
///   执行路径与 SQL 宏相同（`duckdb_query`）；
/// - 变体级 `#[duck(rename = "...")]`：单独覆盖某个变体的标签。
///
/// 约束：必须是 enum、不能带泛型、至少一个变体、变体不能带数据、标签不能重复。
/// 作为**非可空**函数参数时还需要 `Default`（宏生成的参数结构体会 `derive(Default)`），
/// 因此通常写成 `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, DuckEnum)]` 并用
/// `#[default]` 标一个变体；可空参数写 `Option<Priority>` 则不需要。
///
/// Maps a unit-variant-only Rust enum onto a DuckDB `ENUM`. The generated implementation lets the
/// enum be used as a function argument/return value, as a `STRUCT` field and as a container
/// element; its logical type is `ENUM('a', 'b', ...)` with the dictionary in declaration order.
/// The ENUM rules are fixed, so this generates the whole implementation instead of leaning on a
/// blanket impl — `Option<T>` and the struct derive already own one blanket impl each, and a third
/// would conflict (E0119).
///
/// `#[duck(...)]` arguments: `rename_all = "..."` for the variant-name-to-label rule,
/// `sql_name = "..."` for the SQL-side type name (defaults to the lowercase snake_case of the Rust
/// name), and `create_type` — `true` runs `CREATE TYPE IF NOT EXISTS ...` at load time while
/// `"print"` creates nothing: it queues that same DDL for the entry point to print as one batch
/// after every registration, and `false` (the default) does nothing (idempotent, leaves an existing
/// type untouched, same execution path as the SQL macros). A single variant can override its label
/// with `#[duck(rename = "...")]`. The input
/// must be a non-generic enum with at least one data-free variant and distinct labels; a
/// non-nullable function argument additionally needs `Default`.
#[proc_macro_derive(DuckEnum, attributes(duck))]
pub fn duck_enum_derive(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let result = duck_enum_derive::duck_enum_derive(derive_input);
    handle_token_stream2_result(result)
}

/// 把普通 Rust 函数注册成 DuckDB 标量函数。
///
/// 参数与返回值的映射规则（可空性完全由类型表达）：
///
/// - 每个参数对应一个 SQL 参数，参数类型决定 DuckDB 逻辑类型与可空性：写 `T` 就是 NOT NULL，
///   写 `Option<T>` 就是可空 —— `Option<T>` 自己也实现了 `DuckValueType`，可空性由
///   `DuckValueType::from_null` 承载；
/// - 参数写成 `T` 时，输入为 NULL 会直接短路输出 NULL（函数体不执行）；写成 `Option<T>`
///   时以 `None` 进入函数体，语义由函数自己决定；
/// - 返回类型可以是 `T`、`Option<T>`（`None` -> SQL NULL）或 `DuckOptionResult<T>`
///   （可失败、可为 NULL）；`T` 与 `Option<T>` 都直接作为该列的值类型，只有
///   `DuckOptionResult` 额外表示「可能失败」；
/// - 函数体里的 panic 会被捕获并转成查询错误；
/// - `volatile = true` 把函数标记为 volatile：注册时调用
///   `duckdb_scalar_function_set_volatile`，DuckDB 不缓存也不复用相同参数的调用结果，每一行
///   都重新求值（`random()` 这类函数需要它）。需要 duckfn 打开 `duckdb-1-5` feature
///   （DuckDB 1.5.0+ 的 C API），且不能与 `overloads_name` 同用；
/// - `varargs = true` 开启可变参数：函数签名的最后一个参数必须是 `Vec<T>`（可变参数集合），
///   `T` 的逻辑类型会交给 DuckDB 的 `duckdb_scalar_function_set_varargs`，调用时固定参数之后的
///   每一列都按 `T` 读出来、组成 `Vec<T>` 传给函数体。比如
///   `fn my_sum(values: Vec<i64>) -> i64`。同样需要 `duckdb-1-5`，也不能与 `overloads_name` 同用。
///
/// 宏会生成一个同名模块，导出 `scalar_function_builder()` / `scalar_overload_builder()`，
/// 便于手动注册重载或函数集。
///
/// Registers an ordinary Rust function as a DuckDB scalar function. Each parameter maps to one
/// SQL parameter whose type determines both the DuckDB logical type and nullability: `T` means
/// NOT NULL while `Option<T>` means nullable — `Option<T>` implements `DuckValueType` itself,
/// carrying nullability through `DuckValueType::from_null`. A `T` parameter short-circuits NULL
/// input to NULL output without running the body, whereas an `Option<T>` parameter receives
/// `None` and decides the semantics itself. The return type may be `T`, `Option<T>` (`None` maps
/// to SQL NULL) or `DuckOptionResult<T>` (fallible and nullable): `T` and `Option<T>` are both
/// used as the column's value type directly, and only `DuckOptionResult` adds a failure channel.
/// Panics in the body are caught and turned into query errors. `volatile = true` marks the
/// function volatile: registration then calls `duckdb_scalar_function_set_volatile`, so DuckDB
/// neither caches nor reuses the result of a call with the same arguments and every row is
/// re-evaluated (which is what functions like `random()` need). `varargs = true` enables variadic
/// arguments: the last parameter must then be `Vec<T>` (the variadic collection) and `T`'s logical
/// type goes to DuckDB's `duckdb_scalar_function_set_varargs`, so at call time every column after
/// the fixed ones is read as a `T` and collected into the `Vec<T>` passed to the body — e.g.
/// `fn my_sum(values: Vec<i64>) -> i64`. Both switches require duckfn's `duckdb-1-5` feature (the
/// DuckDB 1.5.0+ C API) and cannot be combined with `overloads_name`. A module named after the
/// function is generated, exporting `scalar_function_builder()` and `scalar_overload_builder()`
/// for manual overload / function-set registration.
#[proc_macro_attribute]
pub fn duck_scalar_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_scalar_function())
}

/// 把普通 Rust 函数注册成 DuckDB 聚合函数。
///
/// 函数需带一个 `&mut XxxState` 参数用于跨行累积状态（`XxxState` 需实现
/// `duckfn::DuckAggregateState`，其 `Output` 即聚合的返回类型）；其余参数是每行的输入。
/// 返回值规则与标量函数一致，通常直接返回 `()`。
///
/// 宏生成同名模块并导出 `aggregate_function_builder()` / `aggregate_overload_builder()` /
/// `aggregate_function_guard()`。
///
/// Registers an ordinary Rust function as a DuckDB aggregate function. The function takes one
/// `&mut XxxState` parameter that accumulates state across rows (`XxxState` must implement
/// `duckfn::DuckAggregateState`, whose `Output` is the aggregate's return type); the remaining
/// parameters are the per-row inputs. Return-type rules match scalar functions, though in
/// practice `()` is returned. A module named after the function is generated, exporting
/// `aggregate_function_builder()`, `aggregate_overload_builder()` and
/// `aggregate_function_guard()`.
#[proc_macro_attribute]
pub fn duck_aggregate_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_aggregate_function())
}

/// 把返回迭代器的 Rust 函数注册成 DuckDB 表函数。
///
/// 函数参数即表函数的 bind 参数；返回类型支持三种形式：
///
/// - `impl Iterator<Item = Row>`：最简单，不支持出错；
/// - `DuckResult<impl Iterator<Item = Row>>`：支持构建迭代器时出错；
/// - `DuckFullIteratorResult<Row>`（即 `DuckResult<Box<dyn Iterator<Item = DuckOptionResult<Row>>>>`）：
///   构建与逐行产出都可出错、行也可为 NULL。
///
/// `Row` 需实现 `DuckColumns`（一般用 `#[derive(DuckStruct)]`），其列即结果列。
///
/// Registers a Rust function returning an iterator as a DuckDB table function. The parameters
/// are the bind parameters and three return shapes are supported: `impl Iterator<Item = Row>`
/// (simplest, no error handling), `DuckResult<impl Iterator<Item = Row>>` (construction may
/// fail), and `DuckFullIteratorResult<Row>` (both construction and per-row production may fail,
/// and a row may be NULL). `Row` must implement `DuckColumns` (typically via
/// `#[derive(DuckStruct)]`) and its columns become the result columns.
#[proc_macro_attribute]
pub fn duck_table_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_table_function())
}

/// 把「按批写行」的 Rust 函数注册成 DuckDB 的 COPY 函数，为 `COPY ... TO` 提供自定义文件格式。
///
/// 需要 `duckfn` 打开 `duckdb-1-5` feature（DuckDB 1.5.0+ 的 C API 才提供 COPY 函数）。
///
/// 输出 schema 是**运行时**的：适配层在 bind 阶段把结果各列的类型反推成动态描述，sink 阶段再把每个
/// 数据块读成动态行，因此 `LIST` / `STRUCT` / `MAP` 等嵌套列同样能写出。
///
/// 签名固定为「writer + 行批」两参，顺序可互换：
///
/// ```ignore
/// use duckfn::{
///     duck_copy_function, DuckCopyOptions, DuckCopyToWriter, DuckDynamicRow, DuckResult,
///     DuckResultSchema,
/// };
/// use std::fs::File;
/// use std::io::{BufWriter, Write};
///
/// /// writer 状态：持有已打开的输出文件与 bind 阶段定下的 schema。
/// pub struct MyWriter {
///     file: BufWriter<File>,
///     schema: DuckResultSchema,
/// }
///
/// impl DuckCopyToWriter for MyWriter {
///     fn open(path: &str, schema: &DuckResultSchema, _options: &DuckCopyOptions)
///         -> DuckResult<Self>
///     {
///         Ok(Self {
///             file: BufWriter::new(File::create(path).map_err(/* … */)?),
///             schema: schema.clone(),
///         })
///     }
///
///     fn write_rows(&mut self, rows: &[DuckDynamicRow]) -> DuckResult<()> {
///         // 逐行逐列渲染；LIST / STRUCT / MAP 都能从 DuckDynamicValue 里取到
///         Ok(())
///     }
///
///     fn finish(&mut self) -> DuckResult<()> {
///         self.file.flush().map_err(/* … */)?;
///         Ok(())
///     }
/// }
///
/// /// 每个数据块调用一次；函数名即 `FORMAT <函数名>` 里的格式名。
/// #[duck_copy_function]
/// fn my_copy(writer: &mut MyWriter, rows: &[DuckDynamicRow]) -> DuckResult<()> {
///     writer.write_rows(rows)
/// }
/// ```
///
/// - `&mut Writer`：COPY 的 writer 状态，需实现 `duckfn::DuckCopyToWriter`（`open` 在 global init
///   阶段拿到路径、动态 schema 与 COPY 选项，`finish` 在 finalize 阶段收尾）；
/// - `&[DuckDynamicRow]`：本批要写出的动态行（至多 `vector_size()` 行）；行的列与 `open` 收到的
///   schema 逐列对应，`None` 单元格就是 SQL NULL；
/// - 返回 `DuckResult<()>`，入参或写出失败会让整条 `COPY` 失败，panic 也会被转成查询错误。
///
/// 四个生命周期阶段的回调由适配层生成：bind 把输出列反推成动态 schema 并读出 COPY 选项，global init
/// 调用 `DuckCopyToWriter::open`，sink 把数据块读成动态行后调用被标注的函数，finalize 调用
/// `DuckCopyToWriter::finish`。
///
/// 宏生成同名模块，导出 `copy_function_builder()` 与 `copy_function_register(connection)`；
/// `auto_register = false` 时只生成它们、不自动注册。
///
/// Registers a batch-writing Rust function as a DuckDB copy function, providing a custom file format
/// for `COPY ... TO`. Requires `duckfn`'s `duckdb-1-5` feature (only the DuckDB 1.5.0+ C API provides
/// copy functions). The output schema is a **runtime** one: bind reconstructs every result column's
/// type into a dynamic description and sink reads each data chunk into dynamic rows, so nested
/// columns (`LIST` / `STRUCT` / `MAP`) can be written too. The signature is fixed to "writer + rows"
/// (in either order): `&mut Writer` is the writer state implementing `duckfn::DuckCopyToWriter` (its
/// `open` receives the path, the dynamic schema and the COPY options during global init; its
/// `finish` wraps up during finalize), and `&[DuckDynamicRow]` is the batch to write, one column per
/// schema column with `None` cells as SQL NULL. It returns `DuckResult<()>`: a failure fails the whole
/// `COPY`, and a panic is turned into a query error as well. A module named after the function is
/// generated, exporting `copy_function_builder()` and `copy_function_register(connection)`; with
/// `auto_register = false` they are generated but nothing is registered.
#[proc_macro_attribute]
pub fn duck_copy_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_copy_function())
}

/// 把「按批取行」的 Rust 函数注册成 DuckDB 的 COPY 读取格式，为 `COPY ... FROM` 提供自定义文件格式。
///
/// 需要 `duckfn` 打开 `duckdb-1-5` feature。
///
/// 目标表的 schema 由 DuckDB 给出，读取器**不能**声明结果列；装载时逐批取动态行写进目标表，因此
/// `LIST` / `STRUCT` / `MAP` 等嵌套列同样能读入。
///
/// 签名固定为「reader + limit」两参，顺序可互换：
///
/// ```ignore
/// use duckfn::{
///     duck_copy_from_function, DuckCopyFromReader, DuckDynamicRow, DuckResult, DuckResultSchema,
///     DuckStruct,
/// };
///
/// /// bind 参数：字段 0 必须是文件路径（唯一的位置参数），其余字段是 COPY 的命名选项。
/// #[derive(Default, Debug, Clone, DuckStruct)]
/// #[duck(named_param_from = "skip_rows")]
/// pub struct MyFromArgs {
///     pub path: String,
///     /// `COPY ... FROM 'f' (FORMAT my_from, SKIP_ROWS 2)`
///     pub skip_rows: Option<i64>,
/// }
///
/// pub struct MyReader { /* … */ }
///
/// impl DuckCopyFromReader for MyReader {
///     type Args = MyFromArgs;
///
///     fn open(args: Self::Args, schema: &DuckResultSchema) -> DuckResult<Self> {
///         // schema 就是目标表的列名与类型（含嵌套类型）
///         /* … */
///     }
/// }
///
/// /// 每次取一批行；空 Vec 表示文件读完。函数名即 `FORMAT <函数名>` 里的格式名。
/// #[duck_copy_from_function]
/// fn my_from(reader: &mut MyReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> {
///     reader.next_batch(limit)
/// }
/// ```
///
/// - `&mut Reader`：读取器状态，需实现 `duckfn::DuckCopyFromReader`（`open` 在 bind 阶段拿到参数与
///   目标表 schema）；
/// - `limit: usize`：本批最多多少行（即一个 DuckDB 向量的行数）；
/// - 返回 `DuckResult<Vec<DuckDynamicRow>>`：空 `Vec` 表示流结束；行数与类型不匹配目标表时，错误由
///   适配层的按列校验给出。
///
/// 位置参数必须**恰好一个**（文件路径，`VARCHAR`），bind 阶段会校验；其余 `COPY ... FROM (...)` 选项
/// 以命名参数到达（大小写不敏感），由 `Reader::Args` 声明 —— 未声明的选项 DuckDB 会在 bind 之前报错。
///
/// 宏生成同名模块，导出 `copy_from_register(connection)`；`auto_register = false` 时只生成、不注册。
///
/// Registers a batch-reading Rust function as a DuckDB copy-from format for `COPY ... FROM`.
/// Requires `duckfn`'s `duckdb-1-5` feature. The target table's schema comes from DuckDB and the
/// reader **must not** declare result columns; loading pulls dynamic rows batch by batch, so nested
/// columns (`LIST` / `STRUCT` / `MAP`) load as well. The signature is fixed to "reader + limit" (in
/// either order): `&mut Reader` is the reader state implementing `duckfn::DuckCopyFromReader` (its
/// `open` receives the arguments and the target schema during bind) and `limit: usize` is the batch
/// size (one DuckDB vector's row count). It returns `DuckResult<Vec<DuckDynamicRow>>`, where an empty
/// `Vec` ends the stream; a row whose width or types disagree with the target table is reported by the
/// adapter's per-column validation. Exactly **one** positional parameter (the file path, `VARCHAR`)
/// is required and checked during bind; the remaining `COPY ... FROM (...)` options arrive as named
/// parameters (case-insensitively) declared through `Reader::Args` — an undeclared option is rejected
/// by DuckDB before bind. A module named after the function is generated, exporting
/// `copy_from_register(connection)`; with `auto_register = false` it is generated but nothing is
/// registered.
#[proc_macro_attribute]
pub fn duck_copy_from_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_copy_from_function())
}

/// 手动注册入口：把 `fn(&Connection) -> DuckResult<()>` 交给扩展初始化时调用。
///
/// 用于 `auto_register = false` 的场景：宏生成的各种 `*_builder()` 需要自己注册，
/// 这里是最方便的挂载点。
///
/// Manual registration entry point: makes a `fn(&Connection) -> DuckResult<()>` run during
/// extension initialisation. It suits the `auto_register = false` case, where the generated
/// `*_builder()` functions must be registered by hand.
///
/// # 示例 / Example
///
/// ```ignore
/// #[duck_custom_register]
/// fn register_my_stuff(c: &Connection) -> DuckResult<()> {
///     unsafe { my_scalar::scalar_function_builder().register(c) }
/// }
/// ```
#[proc_macro_attribute]
pub fn duck_custom_register(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_custom_register())
}

/// 把 `fn(源值) -> 目标值` 注册成 DuckDB 的 cast 函数，覆盖 `CAST(源 AS 目标)`。
///
/// 源类型来自唯一参数，目标类型来自返回类型；返回形式与 `duck_scalar_function` 一致：
///
/// ```ignore
/// #[duck_cast_function]
/// fn dfn_cast_str_to_int(s: String) -> DuckOptionResult<i32> {
///     s.parse().map_err(|_| duck_error("not an integer"))
/// }
///
/// // 允许把 NULL 带进函数体：入参写 Option<T>
/// #[duck_cast_function]
/// fn dfn_cast_bigint_to_double(v: Option<i64>) -> Option<f64> { ... }
///
/// // 允许 DuckDB 自动插入该转换
/// #[duck_cast_function(implicit_cost = 100)]
/// fn dfn_cast_str_to_bigint(s: String) -> i64 { ... }
/// ```
///
/// - `CAST(x AS T)` 出错 -> 整条查询失败（`set_error`）；
/// - `TRY_CAST(x AS T)` 出错 -> 该行输出 NULL 并记录行级错误（`set_row_error`）；
/// - 属性支持 `auto_register = false` / `implicit_cost = N`，生成模块里导出
///   `cast_function_builder()` 和 `cast_function_register()` 供手动注册。
///
/// Registers `fn(source) -> target` as a DuckDB cast function covering `CAST(source AS target)`.
/// The source type comes from the single argument and the target type from the return type, with
/// the same return shapes as `duck_scalar_function`. An error fails the whole query for
/// `CAST(x AS T)` (`set_error`) but writes NULL and records a row-level error for
/// `TRY_CAST(x AS T)` (`set_row_error`). The attribute accepts `auto_register = false` and
/// `implicit_cost = N`, and the generated module exports `cast_function_builder()` and
/// `cast_function_register()` for manual registration.
#[proc_macro_attribute]
pub fn duck_cast_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_cast_function())
}

/// 注册一个 SQL 宏：函数返回 SQL 文本（或 builder），扩展初始化时执行/注册。
///
/// 支持的返回形式：
///
/// - `SqlMacro` / `DuckResult<SqlMacro>`：交给 quack-rs 直接注册；
/// - `String` / `&'static str` / `DuckResult<...>`：作为 SQL 文本执行，
///   一段文本里可以有多条 `CREATE OR REPLACE MACRO` 语句。
///
/// Registers a SQL macro: the function returns SQL text (or a builder) that is executed or
/// registered when the extension initialises. Supported return shapes are `SqlMacro` /
/// `DuckResult<SqlMacro>` (registered through quack-rs) and `String` / `&'static str` /
/// `DuckResult<...>` (executed as SQL text, which may contain several
/// `CREATE OR REPLACE MACRO` statements).
#[proc_macro_attribute]
pub fn duck_sql_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_sql_macro())
}

/// 把 `SELECT * FROM 'data.myformat'` 这类「未知表名/文件路径」重定向到某个表函数。
///
/// 函数签名只接受一个「表名（路径）」参数，返回目标表函数的名字：
///
/// ```ignore
/// #[duck_replacement_scan]
/// fn dfn_scan_points(path: &str) -> DuckOptionResult<String> {
///     if path.ends_with(".points") {
///         return Ok(Some("dfn_read_points".to_string()));
///     }
///     Ok(None)
/// }
/// ```
///
/// - `Ok(Some(table_function))`：接管，并把路径作为第一个 VARCHAR 参数传给该表函数；
/// - `Ok(None)`：不接管，DuckDB 继续尝试其他 replacement scan；
/// - `Err(..)` / panic：整条查询以该错误结束。
///
/// 返回值可以是 `Option<String>` / `Option<&'static str>` /
/// `DuckOptionResult<String>` / `DuckOptionResult<&'static str>`。
///
/// Redirects "unknown table names / file paths" such as `SELECT * FROM 'data.myformat'` to a
/// table function. The signature takes exactly one table-name (path) argument and returns the
/// name of the target table function: `Ok(Some(table_function))` takes over and passes the path
/// as the first VARCHAR argument; `Ok(None)` declines so DuckDB tries the next replacement scan;
/// `Err(..)` or a panic fails the whole query. The return type may be `Option<String>` /
/// `Option<&'static str>` / `DuckOptionResult<String>` / `DuckOptionResult<&'static str>`.
#[proc_macro_attribute]
pub fn duck_replacement_scan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_duck_function(_attr, item, |wrapper| wrapper.build_replacement_scan())
}

/// Generate DuckDB extension entry point.
///
/// 生成 DuckDB 扩展入口：`duckfn_entrypoint!("rusty_quack");`
/// 扩展名必须全小写、只含字母/数字/下划线。
///
/// Generates the DuckDB extension entry point: `duckfn_entrypoint!("rusty_quack");`. The
/// extension name must be lowercase and contain only letters, digits and underscores.
///
/// Expands to:
///
/// ```ignore
/// quack_rs::entry_point_v2!(
///     rusty_quack_init_c_api,
///     duckfn::register_all_duckfn
/// );
/// ```
#[proc_macro]
pub fn duckfn_entrypoint(input: TokenStream) -> TokenStream {
    entrypoint::duckfn_entrypoint(input)
}

/// 一次注册多个 SQL 脚本文件（快捷方式）。
///
/// ```ignore
/// duck_sql_macro_files!("sql/a.sql", "sql/b.sql", "sql/c.sql");
/// ```
///
/// 参数是可变多个字符串字面量（文件路径），支持尾随逗号，至少一个。
/// 每个文件在编译期用 `include_str!` 内联（路径相对「调用本宏的 .rs 文件」），
/// 扩展初始化时按书写顺序依次执行整段脚本 —— 一份脚本里可以有多条
/// 分号分隔的 `CREATE OR REPLACE MACRO` 语句。
///
/// 与 `#[duck_sql_macro]` 的关系：后者作用在函数上，由函数返回 SQL；
/// 本宏不需要写函数，直接把若干 .sql 文件注册出去。
///
/// Registers several SQL script files at once (a shortcut). The arguments are one or more
/// string literals (file paths) with an optional trailing comma; at least one is required.
/// Each file is inlined at compile time with `include_str!` (paths are resolved relative to the
/// `.rs` file that invokes the macro) and the whole script is executed in written order when the
/// extension initialises — one script may contain several semicolon-separated
/// `CREATE OR REPLACE MACRO` statements. Unlike `#[duck_sql_macro]`, which annotates a function
/// returning SQL, this macro needs no function and simply registers the `.sql` files.
#[proc_macro]
pub fn duck_sql_macro_files(input: TokenStream) -> TokenStream {
    sql_macro_files::duck_sql_macro_files(input)
}

