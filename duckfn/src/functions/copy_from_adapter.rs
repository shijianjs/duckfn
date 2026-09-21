//! COPY FROM 适配层：把「按批产出动态行」的 Rust 读取器注册成 DuckDB 的 COPY 读取格式。
//!
//! Copy-`FROM` adapter: registers a Rust reader that produces dynamic rows batch by batch as a
//! DuckDB copy-from format.
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
//! 为什么这里不用 `quack-rs` 的 `TableFunctionBuilder`？因为 COPY FROM 需要把 reader 表函数的**原始
//! 句柄**交给 `duckdb_copy_function_set_copy_from_function`，而 quack-rs 0.16 的 builder 只在
//! `register` 内部创建并销毁句柄、不外泄。所以这里直接用 `libduckdb_sys` 建表函数，但复用
//! quack-rs 的 `FfiBindData` / `FfiInitData` 完成 bind → init → scan 的状态传递。
//!
//! Why not `quack-rs`'s `TableFunctionBuilder`? Because COPY FROM has to hand the reader table
//! function's **raw handle** to `duckdb_copy_function_set_copy_from_function`, and the 0.16 builder
//! creates and destroys that handle inside `register` without exposing it. So the table function is
//! built straight through `libduckdb_sys`, while the bind → init → scan state hand-off reuses
//! quack-rs' `FfiBindData` / `FfiInitData`.
//!
//! 三条 DuckDB 侧的硬约束（都由本适配层负责）：
//!
//! 1. reader 的位置参数必须**恰好一个**（文件路径，`VARCHAR`），bind 时会校验；
//! 2. bind 阶段**不能声明结果列** —— schema 已由目标表固定，改为读 `duckdb_table_function_bind_get_result_column_*`；
//! 3. `COPY ... FROM 'f' (FORMAT x, SKIP_ROWS 1)` 里的额外选项以**命名参数**到达（大小写不敏感），
//!    由 [`DuckCopyFromReader::Args`] 声明；未声明的选项 DuckDB 会在 bind 之前报错，且报错信息指向
//!    表函数名 —— 所以表函数与格式同名。
//!
//! Three hard constraints on the DuckDB side, all handled here: the reader takes **exactly one**
//! positional parameter (the file path, `VARCHAR`), checked during bind; bind must **not** declare
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
    DuckDBSuccess, duckdb_bind_info, duckdb_copy_function, duckdb_copy_function_set_copy_from_function,
    duckdb_copy_function_set_name, duckdb_create_copy_function, duckdb_create_table_function,
    duckdb_data_chunk, duckdb_data_chunk_set_size, duckdb_destroy_copy_function,
    duckdb_destroy_table_function, duckdb_function_info, duckdb_init_info,
    duckdb_init_set_init_data, duckdb_register_copy_function, duckdb_table_function,
    duckdb_table_function_add_named_parameter, duckdb_table_function_add_parameter,
    duckdb_table_function_bind_get_result_column_count,
    duckdb_table_function_bind_get_result_column_name,
    duckdb_table_function_bind_get_result_column_type, duckdb_table_function_set_bind,
    duckdb_table_function_set_extra_info, duckdb_table_function_set_function,
    duckdb_table_function_set_init, duckdb_table_function_set_name,
};
use quack_rs::connection::Connection;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{LogicalType, TypeId};
use quack_rs::table::{BindInfo, FfiBindData, FfiInitData, FunctionInfo, InitInfo};
use quack_rs::vector::vector_size;
use std::ffi::{CStr, CString};
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
/// 除了 [`Self::next_batch`]（由宏转调被标注的函数），其余全是提供好的实现：手搓 reader 表函数、
/// bind/init/scan 三个回调、以及注册。
///
/// Apart from [`Self::next_batch`] (which the macro forwards to the annotated function) every
/// method has a default: the reader table function is built by hand, the three callbacks are wired,
/// and registration is provided.
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
                ensure_single_path_parameter::<Self>()?;
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

    /// 建出 reader 表函数（含参数声明与三个回调）。
    ///
    /// Builds the reader table function (parameter declaration plus the three callbacks).
    ///
    /// # Safety
    ///
    /// 返回的句柄由调用方负责：交给 `duckdb_copy_function_set_copy_from_function` 后**不要**再销毁
    /// （见 [`Self::register`] 的说明）。
    ///
    /// The returned handle is the caller's responsibility: after handing it to
    /// `duckdb_copy_function_set_copy_from_function`, do **not** destroy it (see [`Self::register`]).
    ///
    /// # Errors
    ///
    /// 名字含 NUL 字节、或 DuckDB 创建 / 设置表函数失败时返回错误。
    ///
    /// Returns an error when a name contains a NUL byte, or when DuckDB fails to create / configure
    /// the table function.
    unsafe fn reader_table_function() -> DuckResult<duckdb_table_function> {
        let name = CString::new(Self::NAME)
            .map_err(|_| duck_error(format!("{}: format name contains a NUL byte", Self::NAME)))?;
        // 先把命名参数的 C 字符串全部备好，避免创建句柄后再出错、留下未释放的表函数。
        //
        // Prepare every named parameter's C string first, so a later failure cannot leave an
        // unreleased table function behind.
        let mut named = Vec::new();
        for (param_name, logical_type) in
            <Self::Reader as DuckCopyFromReader>::Args::bind_param_logical()
        {
            if let Some(param_name) = param_name {
                let c_name = CString::new(param_name.as_str()).map_err(|_| {
                    duck_error(format!(
                        "{}: COPY option name contains a NUL byte",
                        Self::NAME
                    ))
                })?;
                named.push((c_name, logical_type));
            }
        }

        // SAFETY: 以下调用都只依赖刚创建 / 已校验的句柄，DuckDB 会拷贝类型信息。
        let function = unsafe { duckdb_create_table_function() };
        if function.is_null() {
            return Err(duck_error(format!(
                "{}: duckdb_create_table_function returned null",
                Self::NAME
            )));
        }
        unsafe {
            duckdb_table_function_set_name(function, name.as_ptr());
            // 位置参数：恰好一个文件路径。
            //
            // Positional parameter: exactly one file path.
            let path_type = LogicalType::new(TypeId::Varchar);
            duckdb_table_function_add_parameter(function, path_type.as_raw());
            for (c_name, logical_type) in &named {
                duckdb_table_function_add_named_parameter(
                    function,
                    c_name.as_ptr(),
                    logical_type.as_raw(),
                );
            }
            duckdb_table_function_set_bind(function, Some(Self::c_bind));
            duckdb_table_function_set_init(function, Some(Self::c_init));
            duckdb_table_function_set_function(function, Some(Self::c_scan));
            if let Some((ptr, destroy)) = raw_extra_info(Self::extra_info()) {
                // SAFETY: ptr 由 `DuckExtraInfo::into_raw` 产生，destroy 与它配对；reader 表函数句柄
                // 交给 DuckDB 后由 DuckDB 在销毁它时调用 destroy。
                //
                // SAFETY: `ptr` comes from `DuckExtraInfo::into_raw` and `destroy` matches it; once
                // the reader table-function handle is handed to DuckDB, DuckDB calls `destroy` when
                // it destroys it.
                duckdb_table_function_set_extra_info(function, ptr, destroy);
            }
        }
        Ok(function)
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
    /// DuckDB 注册失败时返回错误。
    ///
    /// Returns an error when DuckDB rejects the registration.
    ///
    /// # 内存说明 / Memory note
    ///
    /// reader 表函数的句柄被 `set_copy_from_function` 接收后**不再由我们销毁**：DuckDB 是拷贝它还是
    /// 接管它没有权威文档，而「自己销毁 + 对方接管」是 double free（致命），「不销毁 + 对方拷贝」只是
    /// 每次 `LOAD` 泄漏一个句柄（无害），因此这里选后者。
    ///
    /// The reader table function handle is **not** destroyed by us once
    /// `set_copy_from_function` has accepted it: there is no authoritative documentation on whether
    /// DuckDB copies or takes it over, and "we destroy + DuckDB took it" is a fatal double free while
    /// "we do not destroy + DuckDB copied it" only leaks one handle per `LOAD` (harmless), so the
    /// latter is chosen.
    unsafe fn register(c: &Connection) -> DuckResult<()> {
        // SAFETY: 由调用方保证连接有效。
        let reader = unsafe { Self::reader_table_function()? };
        // SAFETY: reader 是刚创建的、尚未交给任何人的表函数句柄。
        let copy_function = unsafe { duckdb_create_copy_function() };
        if copy_function.is_null() {
            // reader 还没有交给谁，这里可以安全地释放。
            //
            // The reader has not been handed to anyone yet, so it can be released safely.
            let mut reader = reader;
            unsafe { duckdb_destroy_table_function(&raw mut reader) };
            return Err(duck_error(format!(
                "{}: duckdb_create_copy_function returned null",
                Self::NAME
            )));
        }

        let name = CString::new(Self::NAME)
            .map_err(|_| duck_error(format!("{}: format name contains a NUL byte", Self::NAME)))?;
        // SAFETY: 两个句柄都在上面校验过非空；reader 的所有权随 set_copy_from_function 交出。
        //
        // SAFETY: both handles were checked non-null; ownership of `reader` goes with
        // `set_copy_from_function`.
        unsafe {
            duckdb_copy_function_set_name(copy_function, name.as_ptr());
            duckdb_copy_function_set_copy_from_function(copy_function, reader);
        }
        // SAFETY: copy_function 已配置好 reader；register 成功与否都要由我们销毁它（DuckDB 会拷贝
        // 它需要的内容）。
        //
        // SAFETY: `copy_function` carries the reader; either way we destroy it (DuckDB copies what
        // it needs).
        let result = unsafe { duckdb_register_copy_function(c.as_raw_connection(), copy_function) };
        let mut copy_function: duckdb_copy_function = copy_function;
        unsafe { duckdb_destroy_copy_function(&raw mut copy_function) };

        if result == DuckDBSuccess {
            Ok(())
        } else {
            Err(duck_error(format!(
                "{}: duckdb_register_copy_function failed",
                Self::NAME
            )))
        }
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

/// 校验 reader 恰好声明一个位置参数（文件路径）。
///
/// Validates that the reader declares exactly one positional parameter (the file path).
///
/// DuckDB 自己不校验这一点（`copy_from` 侧才校验），所以这里自己查：多一个位置参数会让 COPY 的
/// 实际调用与 reader 的预期不一致，早报错比读错数据好。
///
/// DuckDB does not check this itself (the `copy_from` side does), so it is checked here: an extra
/// positional parameter would make the actual COPY call disagree with the reader's expectations, and
/// failing early beats reading wrong data.
///
/// # Errors
///
/// 位置参数个数不为 1 时返回错误（含具体个数）。
///
/// Returns an error (with the actual count) unless exactly one positional parameter is declared.
fn ensure_single_path_parameter<A: CopyFromFunctionAdapter>() -> DuckResult<()> {
    let positional =
        <<A as CopyFromFunctionAdapter>::Reader as DuckCopyFromReader>::Args::bind_param_logical()
            .iter()
            .filter(|(name, _)| name.is_none())
            .count();
    if positional == 1 {
        return Ok(());
    }
    Err(duck_error(format!(
        "{}: a COPY FROM reader must declare exactly one positional parameter (the file path), but \
         its arguments declare {positional}",
        A::NAME
    )))
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
