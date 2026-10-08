//! COPY FROM 适配层：把「按批产出动态行」的 Rust 读取器注册成 DuckDB 的 COPY 读取格式。
//!
//! Copy-`FROM` adapter: registers a Rust reader that produces dynamic rows batch by batch as a
//! DuckDB copy-from format.
//!
//! `catch_unwind` 的 panic 兜底**仅原生成立**；在 wasm/浏览器 (`wasm32-unknown-emscripten`) 上
//! 接不住 —— panic 无法跨 JS 边界展开，会变成 `Maximum call stack size exceeded` 栈溢出。
//! 报错请用 `Err(duck_error(..))` 而非 `panic!`（详见 `crate::utils::helpers` 模块文档）。
//!
//! The `catch_unwind` panic guard is native-only: on wasm a `panic!` cannot unwind across the JS
//! boundary and surfaces as a stack overflow. Report errors with `Err(duck_error(..))`, not `panic!`.
//!
//! 与 COPY TO 对称：`COPY ... TO` 用 [`DuckCopyToWriter`](crate::DuckCopyToWriter) 把动态行写出去，
//! `COPY ... FROM` 用 [`DuckCopyFromReader`] 把动态行读进来。目标表的 schema 由 **DuckDB** 给出
//! （reader 不声明结果列），因此 `LIST` / `STRUCT` / `MAP` 这些嵌套列同样能装载。
//!
//! Symmetric to COPY TO: `COPY ... TO` writes dynamic rows through
//! [`DuckCopyToWriter`](crate::DuckCopyToWriter), `COPY ... FROM` reads them through
//! [`DuckCopyFromReader`]. The target table's schema comes from **DuckDB** (the reader declares no
//! result columns), so nested columns (`LIST` / `STRUCT` / `MAP`) load as well.
//!
//! reader 表函数与格式注册都走 quack-rs：`TableFunctionBuilder::build_handle` 建出一个**尚未注册**
//! 的表函数句柄，`CopyFunctionBuilder::copy_from` 把它挂到格式上（顺带替我们校验了「恰好一个
//! `VARCHAR` 位置参数」这条 DuckDB 自己从不检查的硬约束），最后 `register` 完成注册。
//! bind → init → scan 的状态传递仍复用 quack-rs 的 `FfiBindData` / `FfiInitData`。
//!
//! Both the reader table function and the format registration go through quack-rs:
//! `TableFunctionBuilder::build_handle` produces an **unregistered** table-function handle,
//! `CopyFunctionBuilder::copy_from` attaches it to the format — it also enforces the "exactly one
//! `VARCHAR` positional parameter" contract that DuckDB itself never checks — and `register` does
//! the registration. The bind → init → scan state hand-off still reuses quack-rs' `FfiBindData` /
//! `FfiInitData`.
//!
//! 三条 DuckDB 侧的硬约束（都由本适配层负责）：
//!
//! 1. reader 的位置参数必须**恰好一个**（文件路径，`VARCHAR`）；本适配层只登记这一个，多声明的位置
//!    参数会被 quack-rs 在**注册期**挡下来；
//! 2. bind 阶段**不能声明结果列** —— schema 已由目标表固定，改为读 `duckdb_table_function_bind_get_result_column_*`；
//! 3. `COPY ... FROM 'f' (FORMAT x, SKIP_ROWS 1)` 里的额外选项以**命名参数**到达（大小写不敏感），
//!    由 [`DuckCopyFromReader::Args`] 声明；未声明的选项 DuckDB 会在 bind 之前报错，且报错信息指向
//!    表函数名 —— 所以表函数与格式同名。
//!
//! Three hard constraints on the DuckDB side, all handled here: the reader takes **exactly one**
//! positional parameter (the file path, `VARCHAR`) — this adapter declares just that one, and quack-rs
//! refuses a reader declaring more at **registration** time; bind must **not** declare
//! result columns (the schema is fixed by the target table, so it reads
//! `duckdb_table_function_bind_get_result_column_*` instead); and the extra
//! `COPY ... FROM 'f' (FORMAT x, SKIP_ROWS 1)` options arrive as **named parameters**
//! (case-insensitively), declared through [`DuckCopyFromReader::Args`]. Undeclared options are
//! rejected by DuckDB before bind with an error naming the *table function* — hence the table
//! function carries the format name.
//!
//! 一般不用手写这个 impl，用 `#[duck_copy_from_function]` 标注取批函数即可：
//!
//! Usually you do not implement this manually: annotate the batch function with
//! `#[duck_copy_from_function]`.

use crate::{
    DuckBindArgs, DuckDynamicRow, DuckExtraInfo, DuckResult, DuckResultSchema, DuckTypeDesc,
    duck_error, panic_to_string, raw_extra_info,
};
use libduckdb_sys::{
    duckdb_bind_info, duckdb_data_chunk, duckdb_data_chunk_set_size, duckdb_function_info,
    duckdb_init_info, duckdb_init_set_init_data,
    duckdb_table_function_bind_get_result_column_count,
    duckdb_table_function_bind_get_result_column_name,
    duckdb_table_function_bind_get_result_column_type,
};
use quack_rs::connection::Connection;
use quack_rs::copy_function::CopyFunctionBuilder;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{LogicalType, TypeId};
use quack_rs::table::{
    BindInfo, FfiBindData, FfiInitData, FunctionInfo, InitInfo, TableFunctionBuilder,
    TableFunctionHandle,
};
use quack_rs::vector::vector_size;
use std::ffi::CStr;
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

/// COPY FROM 的读取器：bind 阶段建状态，scan 阶段按批产出动态行。
///
/// A COPY FROM reader: it builds its state during bind and produces dynamic rows batch by batch
/// during scan.
///
/// 两阶段是必须的：`open` 要拿到**参数**（路径与 COPY 选项）与**目标表 schema**，两者都只有 bind
/// 阶段才有；`next_batch` 则在 scan 阶段被反复调用。
///
/// The two phases are unavoidable: `open` needs the **arguments** (the path and the COPY options)
/// and the **target schema**, both of which only exist during bind, while `next_batch` is called
/// repeatedly during scan.
///
/// ```ignore
/// #[derive(Default, Debug, Clone, DuckStruct)]
/// #[duck(named_param_from = "skip_rows")]
/// pub struct MyFromArgs {
///     /// 位置参数 0：文件路径。
///     pub path: String,
///     /// 命名 COPY 选项：`COPY ... FROM 'f' (FORMAT my_fmt, SKIP_ROWS 2)`。
///     pub skip_rows: Option<i64>,
/// }
///
/// impl DuckCopyFromReader for MyReader {
///     type Args = MyFromArgs;
///
///     fn open(args: Self::Args, schema: &DuckResultSchema) -> DuckResult<Self> { /* ... */ }
/// }
///
/// #[duck_copy_from_function]
/// fn my_from(reader: &mut MyReader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>> { /* ... */ }
/// ```
pub trait DuckCopyFromReader: Sized + Send + 'static {
    /// bind 参数：**字段 0 必须是文件路径**（唯一的位置参数），其余字段是用户声明的命名 COPY 选项。
    ///
    /// The bind arguments: **field 0 must be the file path** (the one positional parameter), and the
    /// remaining fields are the named COPY options the reader supports.
    type Args: DuckBindArgs;

    /// bind 阶段：拿参数与目标表 schema 建状态。
    ///
    /// `schema` 就是目标表的列名与列类型（含嵌套类型），由适配层从
    /// `duckdb_table_function_bind_get_result_column_*` 读出来 —— reader 不需要（也不允许）自己声明
    /// 结果列。
    ///
    /// Bind phase: builds the state from the arguments and the target schema. `schema` is the target
    /// table's column names and types (nested types included), read by the adapter from
    /// `duckdb_table_function_bind_get_result_column_*` — the reader neither needs to nor may declare
    /// result columns itself.
    fn open(args: Self::Args, schema: &DuckResultSchema) -> DuckResult<Self>;

    /// finalize 阶段：收尾（默认什么都不做）。
    ///
    /// 表函数没有 finalize 回调，所以它是在查询结束、状态被释放时被调用的；**返回的错误无法上报**，
    /// 只会打成一行 `-- [duckfn]` 警告 —— 真正要中断装载的错误请在 [`Self::open`] 或
    /// [`CopyFromFunctionAdapter::next_batch`] 里返回。
    ///
    /// Finalize phase: closes the reader (a no-op by default). A table function has no finalize
    /// callback, so this runs when the query ends and the state is dropped; **an error returned here
    /// cannot be reported**, it only becomes a `-- [duckfn]` warning. Anything that must abort the
    /// load belongs in [`Self::open`] or [`CopyFromFunctionAdapter::next_batch`].
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}

/// 把 [`DuckCopyFromReader`] 接成 DuckDB 的 COPY FROM 格式。
///
/// Wires a [`DuckCopyFromReader`] into a DuckDB copy-from format.
///
/// 除了 [`Self::next_batch`]（由宏转调被标注的函数），其余全是提供好的实现：reader 表函数由
/// quack-rs 的 builder 建出并挂到格式上，bind/init/scan 三个回调接好，注册也一并完成。
///
/// Apart from [`Self::next_batch`] (which the macro forwards to the annotated function) every
/// method has a default: the reader table function is built by quack-rs' builder and attached to
/// the format, the three callbacks are wired, and registration is provided.
pub trait CopyFromFunctionAdapter: Sized + 'static {
    /// 格式名，同时也是 reader 表函数的名字。
    ///
    /// The format name, which is also the reader table function's name.
    ///
    /// 两者必须相同：未声明的 COPY 选项会在 bind 之前报错，而那条报错信息指向的是表函数名。
    ///
    /// They must match: an undeclared COPY option is rejected before bind, and that message names
    /// the table function.
    const NAME: &'static str;

    /// 读取器类型。
    ///
    /// The reader type.
    type Reader: DuckCopyFromReader;

    /// 注册期附加的数据（DuckDB 的 `extra_info`）；默认不附加。
    ///
    /// 它挂在 reader 表函数上：`open` / `next_batch` 拿不到它，需要时重写
    /// [`Self::c_bind`] / [`Self::c_init`] / [`Self::c_scan`]，用
    /// `unsafe { duckfn::extra_info_ref::<MyConfig>(&info) }` 取回。类型必须是
    /// `Send + Sync + 'static`：reader 句柄跨查询共享，数据在句柄销毁时才释放。
    ///
    /// Function-level data attached at registration time (DuckDB's `extra_info`); nothing is
    /// attached by default. It is attached to the reader table function: `open` / `next_batch` cannot
    /// reach it, so override [`Self::c_bind`] / [`Self::c_init`] / [`Self::c_scan`] and call
    /// `unsafe { duckfn::extra_info_ref::<MyConfig>(&info) }` to read it. The type must be
    /// `Send + Sync + 'static`: the reader handle is shared across queries and the data is only
    /// released when that handle is destroyed.
    fn extra_info() -> Option<DuckExtraInfo> {
        None
    }

    /// scan 阶段：取下一批行；返回空 `Vec` 表示流结束。
    ///
    /// 批大小由 `limit` 给出（即一个 DuckDB 向量的行数），实现应尽量按它取满，但少取也可以。
    ///
    /// Scan phase: takes the next batch of rows; an empty `Vec` means end of stream. `limit` is the
    /// batch size (one DuckDB vector's row count); taking fewer rows is allowed.
    fn next_batch(reader: &mut Self::Reader, limit: usize) -> DuckResult<Vec<DuckDynamicRow>>;

    /// bind 阶段：解析参数、读目标表 schema、建 reader 状态。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Bind phase: parses the arguments, reads the target schema and builds the reader state. Called
    /// by DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_bind(raw: duckdb_bind_info) {
        // SAFETY: raw 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { BindInfo::new(raw) };
        handle(
            AssertUnwindSafe(|| {
                let args = <Self::Reader as DuckCopyFromReader>::Args::read_bind_args(&info)?;
                // SAFETY: raw 在回调期间有效。
                let schema = unsafe { read_target_schema(raw)? };
                let reader = Self::Reader::open(args, &schema)?;
                let state = CopyFromState {
                    schema,
                    reader,
                };
                // SAFETY: raw 有效；Mutex 让 init 回调能把状态取走（bind 数据本身由 DuckDB 释放）。
                //
                // SAFETY: `raw` is valid; the Mutex lets the init callback take the state out (DuckDB
                // itself releases the bind data).
                unsafe {
                    FfiBindData::<Mutex<Option<CopyFromState<Self::Reader>>>>::set(
                        raw,
                        Mutex::new(Some(state)),
                    );
                }
                Ok(())
            }),
            |message| info.set_error(message),
        );
    }

    /// init 阶段：把 bind 阶段建好的状态搬进 init data（scan 阶段要用 `&mut`）。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Init phase: moves the state built during bind into the init data (scan needs `&mut`). Called
    /// by DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_init(raw: duckdb_init_info) {
        // SAFETY: raw 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { InitInfo::new(raw) };
        handle(
            AssertUnwindSafe(|| {
                // SAFETY: bind 回调把状态放成了 `Mutex<Option<CopyFromState<_>>>`。
                let cell = unsafe {
                    FfiBindData::<Mutex<Option<CopyFromState<Self::Reader>>>>::get_from_init(raw)
                };
                let Some(cell) = cell else {
                    return Err(duck_error(format!(
                        "{}: missing reader state (was the bind callback skipped?)",
                        Self::NAME
                    )));
                };
                let mut guard = cell.lock().map_err(|_| {
                    duck_error(format!("{}: reader state mutex was poisoned", Self::NAME))
                })?;
                let Some(state) = guard.take() else {
                    return Err(duck_error(format!(
                        "{}: reader state was already consumed",
                        Self::NAME
                    )));
                };
                // SAFETY: raw 有效；我们自定义析构回调，好在查询结束时调用 `Reader::finish`。
                //
                // SAFETY: `raw` is valid; the destructor is ours so that `Reader::finish` runs when
                // the query ends.
                unsafe {
                    duckdb_init_set_init_data(
                        raw,
                        Box::into_raw(Box::new(state)).cast::<c_void>(),
                        Some(destroy_copy_from_state::<Self>),
                    );
                }
                // 状态只要求 `Send`（不要求 `Sync`），因此强制串行扫描。
                //
                // The state is only `Send`, not `Sync`, so scans are forced to be serial.
                info.set_max_threads(1);
                Ok(())
            }),
            |message| info.set_error(message),
        );
    }

    /// scan 阶段：取一批行写进输出数据块；空批表示结束。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 与 `output` 均由 DuckDB 保证有效。
    ///
    /// Scan phase: takes one batch of rows into the output chunk; an empty batch ends the stream.
    /// Called by DuckDB; `info` and `output` are guaranteed valid.
    unsafe extern "C" fn c_scan(raw: duckdb_function_info, output: duckdb_data_chunk) {
        // SAFETY: raw 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { FunctionInfo::new(raw) };
        let result = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: init 回调设置过 init data；扫描串行执行，因此不存在别名 `&mut`。
            let state = unsafe { FfiInitData::<CopyFromState<Self::Reader>>::get_mut(raw) };
            let Some(state) = state else {
                return Err(duck_error(format!(
                    "{}: missing reader state (was the init callback skipped?)",
                    Self::NAME
                )));
            };
            let rows = Self::next_batch(&mut state.reader, vector_size() as usize)?;
            if rows.is_empty() {
                // 空批 = EOF：块大小置 0，DuckDB 停止扫描。
                //
                // An empty batch is EOF: chunk size 0 stops the scan.
                unsafe { duckdb_data_chunk_set_size(output, 0) };
                return Ok(());
            }
            // SAFETY: output 由 DuckDB 传入，在回调期间有效。
            let chunk = unsafe { DataChunk::from_raw(output) };
            let refs: Vec<Option<&DuckDynamicRow>> = rows.iter().map(Some).collect();
            DuckDynamicRow::write_batch(&chunk, &state.schema, &refs)?;
            unsafe { chunk.set_size(rows.len()) };
            Ok(())
        }));

        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                info.set_error(e.as_str());
                // 出错时必须把块大小归零，否则 DuckDB 会读一块没写完的数据。
                //
                // On error the chunk size must be zeroed, or DuckDB reads a half-written chunk.
                unsafe { duckdb_data_chunk_set_size(output, 0) };
            }
            Err(payload) => {
                info.set_error(&panic_to_string(payload));
                unsafe { duckdb_data_chunk_set_size(output, 0) };
            }
        }
    }

    /// 建出 reader 表函数（含参数声明与三个回调），但**不注册**。
    ///
    /// Builds the reader table function (parameter declaration plus the three callbacks) without
    /// registering it.
    ///
    /// # Errors
    ///
    /// 格式名不合法（空、含 NUL 字节、含非法字符），或 DuckDB 创建 / 设置表函数失败时返回错误。
    /// 另外 quack-rs 的 builder 会替你检查「恰好一个 `VARCHAR` 位置参数」这条 DuckDB 自己从不检查
    /// 的约束：[`Self::reader_table_function`] 只登记一个 `VARCHAR` 位置参数（文件路径），其余位置
    /// 参数一律忽略，所以多声明的 reader 在注册期就会被挡下来。
    ///
    /// Returns an error when the format name is invalid (empty, containing a NUL byte or an illegal
    /// character), or when DuckDB fails to create / configure the table function. quack-rs' builder
    /// also enforces the "exactly one `VARCHAR` positional parameter" contract that DuckDB itself
    /// never checks: [`Self::reader_table_function`] declares exactly one `VARCHAR` positional
    /// parameter (the file path) and ignores any others, so a reader declaring more is refused at
    /// registration time.
    ///
    /// # Safety
    ///
    /// 需要 DuckDB 的 C API 分发表已初始化（在扩展里始终成立）。
    ///
    /// The DuckDB C API dispatch table must be initialised (it always is inside an extension).
    unsafe fn reader_table_function() -> DuckResult<TableFunctionHandle> {
        let mut builder = TableFunctionBuilder::try_new(Self::NAME)?
            // 位置参数：恰好一个文件路径；COPY 选项全部以命名参数到达（见模块文档第 3 条）。
            //
            // Positional parameter: exactly one file path; every COPY option arrives as a named
            // parameter (see constraint 3 in the module docs).
            .param(TypeId::Varchar)
            .bind(Self::c_bind)
            .init(Self::c_init)
            .scan(Self::c_scan);
        for (name, logical_type) in <Self::Reader as DuckCopyFromReader>::Args::bind_param_logical() {
            if let Some(name) = name {
                builder = builder.named_param_logical(&name, logical_type);
            }
        }
        if let Some((ptr, Some(destroy))) = raw_extra_info(Self::extra_info()) {
            // SAFETY: ptr 由 `DuckExtraInfo::into_raw` 产生，destroy 与它配对（析构回调内部兜了
            // panic）；`build_handle` 之后所有权交给 DuckDB，销毁 reader 表函数时它会被调用。
            //
            // SAFETY: `ptr` comes from `DuckExtraInfo::into_raw` and `destroy` matches it (that
            // destructor catches panics); `build_handle` hands ownership to DuckDB, which calls it
            // when it destroys the reader table function.
            builder = unsafe { builder.extra_info(ptr, destroy) };
        }
        // SAFETY: 由本方法的契约保证 DuckDB 的 C API 分发表已初始化。
        unsafe { builder.build_handle() }
    }

    /// 注册成 `COPY ... FROM 'f' (FORMAT <NAME>)` 可用的格式。
    ///
    /// Registers the format usable as `COPY ... FROM 'f' (FORMAT <NAME>)`.
    ///
    /// # Safety
    ///
    /// `c` 必须是有效的 DuckDB 连接。
    ///
    /// `c` must be a valid DuckDB connection.
    ///
    /// # Errors
    ///
    /// 表函数建不出来、reader 不满足 quack-rs 的参数契约（位置参数不是一个 `VARCHAR`），或 DuckDB
    /// 注册失败（含格式名已被占用）时返回错误。
    ///
    /// Returns an error when the table function cannot be built, when the reader violates quack-rs'
    /// parameter contract (its positional parameter is not a `VARCHAR`), or when DuckDB rejects the
    /// registration (including a format name that is already taken).
    unsafe fn register(c: &Connection) -> DuckResult<()> {
        let reader = unsafe { Self::reader_table_function()? };
        // SAFETY: 由调用方保证连接有效；`copy_from` 按值把 reader 拷进 copy function（随后 quack-rs
        // 释放我们这份句柄），并校验了它的参数契约。
        //
        // SAFETY: the caller guarantees a valid connection; `copy_from` copies the reader into the
        // copy function by value (quack-rs then releases our handle) and has checked its parameter
        // contract.
        unsafe {
            CopyFunctionBuilder::try_new(Self::NAME)?
                .copy_from(reader)?
                .register(c.as_raw_connection())?
        };
        Ok(())
    }
}

/// scan 状态：目标表 schema + 读取器。
///
/// The scan state: the target schema plus the reader.
struct CopyFromState<R: DuckCopyFromReader> {
    /// 目标表列定义（含嵌套类型）。
    ///
    /// The target table's column definitions (nested types included).
    schema: DuckResultSchema,
    /// 用户读取器。
    ///
    /// The user reader.
    reader: R,
}

/// 读出目标表的 schema（列名 + 列类型）。
///
/// Reads the target table's schema (column names plus column types).
///
/// 这一步是 COPY FROM 的关键：schema 由目标表固定，reader 只能读、不能声明。
///
/// This is the crux of COPY FROM: the schema is fixed by the target table, so the reader can only
/// read it, never declare it.
///
/// 列名返回的 `*const c_char` **由 DuckDB 持有**（生命周期覆盖整个 bind 回调），不能对它调用
/// `duckdb_free` —— 实测那样做会堆损坏（`0xC0000374`）。所以这里只拷贝，不释放。
///
/// The returned column-name `*const c_char` is **owned by DuckDB** (valid for the whole bind
/// callback) and must not be passed to `duckdb_free`: doing so corrupts the heap in practice
/// (`0xC0000374`). It is copied, never released.
///
/// # Safety
///
/// `raw` 必须是 bind 回调期间有效的 `duckdb_bind_info`。
///
/// `raw` must be a `duckdb_bind_info` valid for the duration of the bind callback.
unsafe fn read_target_schema(raw: duckdb_bind_info) -> DuckResult<DuckResultSchema> {
    // SAFETY: 由调用方保证 raw 有效。
    let count = unsafe { duckdb_table_function_bind_get_result_column_count(raw) };
    if count == 0 {
        return Err(duck_error(
            "copy from: the target table has no columns; a COPY FROM reader reads the target \
             schema instead of declaring result columns",
        ));
    }

    let mut columns = Vec::with_capacity(count as usize);
    for index in 0..count {
        // SAFETY: index < count；返回的字符串由 DuckDB 分配，按 C API 惯例由调用方 duckdb_free。
        //
        // SAFETY: `index < count`; DuckDB allocates the returned string and, per the C API
        // convention, the caller releases it with `duckdb_free`.
        let name_ptr = unsafe { duckdb_table_function_bind_get_result_column_name(raw, index) };
        let name = if name_ptr.is_null() {
            format!("column_{index}")
        } else {
            // SAFETY: 非空且以 NUL 结尾，在回调期间有效。
            //
            // SAFETY: non-null, NUL-terminated, valid for the callback.
            unsafe { CStr::from_ptr(name_ptr) }
                .to_string_lossy()
                .into_owned()
        };

        // SAFETY: index < count；返回的逻辑类型由我们持有并在本轮结束时释放。
        //
        // SAFETY: `index < count`; the returned logical type is ours and released at the end of this
        // iteration.
        let logical_type =
            unsafe { LogicalType::from_raw(duckdb_table_function_bind_get_result_column_type(raw, index)) };
        columns.push((name, DuckTypeDesc::from_logical_type(&logical_type)?));
    }
    Ok(DuckResultSchema::new(columns))
}

/// init data 的析构回调：调用 [`DuckCopyFromReader::finish`] 后释放状态。
///
/// The init-data destructor: calls [`DuckCopyFromReader::finish`] and releases the state.
///
/// 表函数没有 finalize 回调，查询结束时的收尾只能挂在这里。这里**无法上报错误**，所以失败只打一行
/// 警告（真正要中断装载的错误应在 `open` / `next_batch` 里返回）。
///
/// A table function has no finalize callback, so end-of-query cleanup hangs here. Errors **cannot be
/// reported** from here and only produce a warning (anything that must abort the load belongs in
/// `open` / `next_batch`).
///
/// # Safety
///
/// `ptr` 必须是 `Box::into_raw(Box<CopyFromState<A::Reader>>)` 的结果，或为空。
///
/// `ptr` must be the result of `Box::into_raw(Box::<CopyFromState<A::Reader>>::new(..))`, or null.
unsafe extern "C" fn destroy_copy_from_state<A: CopyFromFunctionAdapter>(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: 调用方保证 ptr 来自 Box::into_raw，且只 drop 一次。
    let mut state = unsafe { Box::from_raw(ptr.cast::<CopyFromState<A::Reader>>()) };
    if let Err(e) = state.reader.finish() {
        eprintln!(
            "-- [duckfn] {}: reader finish failed at end of query: {}",
            A::NAME,
            e.as_str()
        );
    }
    drop(state);
}

/// 跑一个生命周期阶段：把 `Err` 与 panic 都交给 `set_error`，正常结束什么都不做。
///
/// Runs one life-cycle phase: both `Err` and a panic are reported through `set_error`, and a
/// normal finish does nothing.
fn handle<F>(f: AssertUnwindSafe<F>, set_error: impl FnOnce(&str))
where
    F: FnOnce() -> DuckResult<()>,
{
    match catch_unwind(f) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => set_error(e.as_str()),
        Err(payload) => set_error(&panic_to_string(payload)),
    }
}
