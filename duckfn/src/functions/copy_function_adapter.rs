//! COPY TO 适配层：把一个「按 chunk 写出」的 Rust 函数注册成 DuckDB 的 COPY 函数（自定义文件格式）。
//!
//! Copy-function (`COPY ... TO`) adapter: registers a Rust function that writes one data chunk at a
//! time as a DuckDB copy function (a custom file format).
//!
//! 生命周期与 quack-rs 的 `CopyFunctionBuilder` 一致，四个阶段各对应一个回调：
//!
//! 1. **bind**：拿到输出列的 `LogicalType`，收进 bind data；
//! 2. **global init**：拿到输出文件路径，调用 [`DuckCopyWriter::open`] 打开 writer；
//! 3. **sink**：每个数据块调用一次 [`CopyFunctionAdapter::write_chunk`]（即被标注的函数）；
//! 4. **finalize**：调用 [`DuckCopyWriter::finish`] 收尾并关闭。
//!
//! The life cycle matches quack-rs' `CopyFunctionBuilder`; each of the four phases maps onto one
//! callback: **bind** records the output column logical types, **global init** opens the writer
//! through [`DuckCopyWriter::open`], **sink** calls [`CopyFunctionAdapter::write_chunk`] once per
//! data chunk (this is the annotated function) and **finalize** calls
//! [`DuckCopyWriter::finish`].
//!
//! 一般不用手写这个 impl，直接用 `#[duck_copy_function]` 作用在
//! `fn my_copy(writer: &mut MyWriter, chunk: &DataChunk) -> DuckResult<()>` 上即可。
//!
//! Usually you do not implement this manually: annotate
//! `fn my_copy(writer: &mut MyWriter, chunk: &DataChunk) -> DuckResult<()>` with
//! `#[duck_copy_function]`.

use crate::{DuckResult, duck_error, panic_to_string};
use quack_rs::connection::Connection;
use quack_rs::copy_function::{
    CopyBindInfo, CopyFinalizeInfo, CopyFunctionBuilder, CopyGlobalInitInfo, CopySinkInfo,
};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::LogicalType;
use libduckdb_sys::{
    duckdb_copy_function_bind_info, duckdb_copy_function_finalize_info,
    duckdb_copy_function_global_init_info, duckdb_copy_function_sink_info, duckdb_data_chunk,
};
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// COPY 输出的 writer 状态：由 [`DuckCopyWriter::open`] 创建，跨数据块复用。
///
/// The writer state of a `COPY ... TO` format: created by [`DuckCopyWriter::open`] and reused
/// across data chunks.
///
/// 实现者通常持有「已打开的输出文件」，并在 [`Self::finish`] 里 flush / 关闭。
///
/// An implementor typically owns an open output file and flushes / closes it in [`Self::finish`].
pub trait DuckCopyWriter: Sized + 'static {
    /// global init 阶段：打开输出目标。
    ///
    /// `path` 是 `COPY ... TO 'path'` 里的目标路径；`columns` 是查询结果各列的逻辑类型
    /// （bind 阶段从 [`CopyBindInfo`] 读到，顺序与数据块列顺序一致）。
    ///
    /// Global-init phase: opens the output target. `path` is the destination of
    /// `COPY ... TO 'path'` and `columns` holds the logical type of every output column (read from
    /// [`CopyBindInfo`] during bind, in the same order as the data-chunk columns).
    fn open(path: &str, columns: &[LogicalType]) -> DuckResult<Self>;

    /// finalize 阶段：flush / 关闭输出目标（默认什么都不做）。
    ///
    /// Finalize phase: flushes / closes the output target (a no-op by default).
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}

/// 把 [`DuckCopyWriter`] 的四个生命周期阶段接到 DuckDB 的 COPY 回调上。
///
/// Wires the four life-cycle phases of a [`DuckCopyWriter`] onto DuckDB's copy callbacks.
///
/// 适配层负责：
///
/// - 把 bind 阶段读到的列类型存成 bind data，并在 global init 阶段取回；
/// - 用 `Box` 承载 writer，通过 `set_global_state` 交给 DuckDB，并注册析构回调负责 drop；
/// - 每个阶段都用 `catch_unwind` 包住，panic 与 `Err` 都转成查询错误（`set_error`），
///   不会跨 FFI 展开。
///
/// The adapter stores the column types read during bind as bind data (retrieved again during
/// global init), boxes the writer and hands it to DuckDB through `set_global_state` with a
/// destructor callback that drops it, and wraps every phase in `catch_unwind` so that panics and
/// `Err`s become query errors (`set_error`) instead of unwinding across FFI.
pub trait CopyFunctionAdapter: Sized + 'static {
    /// 回调名称，同时用作 `COPY ... (FORMAT <name>)` 里的格式名。
    ///
    /// The callback name, also used as the format name in `COPY ... (FORMAT <name>)`.
    const NAME: &'static str;

    /// writer 状态类型（由 [`DuckCopyWriter`] 描述）。
    ///
    /// The writer state type (described by [`DuckCopyWriter`]).
    type Writer: DuckCopyWriter;

    /// sink 阶段：写出一个数据块（由宏生成的代码转调被标注的函数）。
    ///
    /// Sink phase: writes one data chunk (the macro-generated code forwards this to the annotated
    /// function).
    fn write_chunk(writer: &mut Self::Writer, chunk: &DataChunk) -> DuckResult<()>;

    /// bind 阶段：记录输出列的 `LogicalType`，供 global init 阶段交给
    /// [`DuckCopyWriter::open`]。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Bind phase: records the output columns' `LogicalType` so that global init can hand them to
    /// [`DuckCopyWriter::open`]. Called by DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_bind(info: duckdb_copy_function_bind_info) {
        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CopyBindInfo::new(info) };
        handle(
            AssertUnwindSafe(|| {
                let count = info.column_count();
                let mut columns: Vec<LogicalType> = Vec::with_capacity(count as usize);
                for index in 0..count {
                    // SAFETY: index < column_count()。
                    columns.push(unsafe { info.column_type(index) });
                }
                let boxed = Box::new(columns);
                // SAFETY: Box 的所有权交给 DuckDB，析构回调负责 drop。
                unsafe {
                    info.set_bind_data(
                        Box::into_raw(boxed).cast::<c_void>(),
                        Some(drop_boxed::<Vec<LogicalType>>),
                    );
                }
                Ok(())
            }),
            |message| info.set_error(message),
        );
    }

    /// global init 阶段：取回 bind data 与输出路径，调用 [`DuckCopyWriter::open`]。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Global-init phase: retrieves the bind data and the output path, then calls
    /// [`DuckCopyWriter::open`]. Called by DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_global_init(info: duckdb_copy_function_global_init_info) {
        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CopyGlobalInitInfo::new(info) };
        handle(
            AssertUnwindSafe(|| {
                // SAFETY: 回调期间有效；DuckDB 负责释放它自己的缓冲区。
                let path = unsafe { info.get_file_path() };
                // SAFETY: bind 阶段设置过 bind_data；DuckDB 保证它在整个查询期间有效。
                let bind_ptr = unsafe { info.get_bind_data() }.cast::<Vec<LogicalType>>();
                let empty: Vec<LogicalType> = Vec::new();
                // SAFETY: 非空时指向 bind 阶段放入的 `Vec<LogicalType>`。
                let columns: &[LogicalType] = if bind_ptr.is_null() {
                    &empty
                } else {
                    unsafe { &*bind_ptr }
                };
                let writer = Self::Writer::open(&path, columns)?;
                let boxed = Box::new(writer);
                // SAFETY: Box 的所有权交给 DuckDB，析构回调负责 drop。
                unsafe {
                    info.set_global_state(
                        Box::into_raw(boxed).cast::<c_void>(),
                        Some(drop_boxed::<Self::Writer>),
                    );
                }
                Ok(())
            }),
            |message| info.set_error(message),
        );
    }

    /// sink 阶段：取回 writer，写出当前数据块。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 与 `chunk` 均由 DuckDB 保证有效。
    ///
    /// Sink phase: retrieves the writer and writes the current data chunk. Called by DuckDB;
    /// `info` and `chunk` are guaranteed valid.
    unsafe extern "C" fn c_sink(info: duckdb_copy_function_sink_info, chunk: duckdb_data_chunk) {
        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CopySinkInfo::new(info) };
        handle(
            AssertUnwindSafe(|| {
                // SAFETY: global init 阶段设置过 global_state；DuckDB 保证它在整个查询期间有效。
                let state_ptr = unsafe { info.get_global_state() }.cast::<Self::Writer>();
                if state_ptr.is_null() {
                    return Err(duck_error(format!(
                        "{}: copy writer was not initialized",
                        Self::NAME
                    )));
                }
                // SAFETY: 非空指针指向 global init 阶段放入的 writer，且同一时刻只有一个 sink 回调。
                let writer = unsafe { &mut *state_ptr };
                // SAFETY: chunk 由 DuckDB 传入，在回调期间有效。
                let chunk = unsafe { DataChunk::from_raw(chunk) };
                Self::write_chunk(writer, &chunk)
            }),
            |message| info.set_error(message),
        );
    }

    /// finalize 阶段：取回 writer 并调用 [`DuckCopyWriter::finish`]。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Finalize phase: retrieves the writer and calls [`DuckCopyWriter::finish`]. Called by
    /// DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_finalize(info: duckdb_copy_function_finalize_info) {
        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CopyFinalizeInfo::new(info) };
        handle(
            AssertUnwindSafe(|| {
                // SAFETY: global init 阶段设置过 global_state；DuckDB 保证它在整个查询期间有效。
                let state_ptr = unsafe { info.get_global_state() }.cast::<Self::Writer>();
                if state_ptr.is_null() {
                    return Ok(());
                }
                // SAFETY: 非空指针指向 global init 阶段放入的 writer。
                let writer = unsafe { &mut *state_ptr };
                writer.finish()
            }),
            |message| info.set_error(message),
        );
    }

    /// 构造带四个回调的 COPY 函数 builder。
    ///
    /// Builds the copy-function builder carrying the four callbacks.
    ///
    /// # Errors
    ///
    /// 格式名含 NUL 字节时返回错误。
    ///
    /// Returns an error when the format name contains a NUL byte.
    fn copy_function_builder() -> DuckResult<CopyFunctionBuilder> {
        let builder = CopyFunctionBuilder::try_new(Self::NAME)?
            .bind(Self::c_bind)
            .global_init(Self::c_global_init)
            .sink(Self::c_sink)
            .finalize(Self::c_finalize);
        Ok(builder)
    }

    /// # Safety
    ///
    /// `c` 必须是有效的 DuckDB 连接。
    ///
    /// `c` must be a valid DuckDB connection.
    ///
    /// # Errors
    ///
    /// 注册失败（例如缺少回调）时返回对应错误。
    ///
    /// Returns the underlying error when registration fails (for example a missing callback).
    unsafe fn register(c: &Connection) -> DuckResult<()> {
        let builder = Self::copy_function_builder()?;
        // SAFETY: 由调用方保证连接有效；builder 已设置 bind/sink/finalize。
        unsafe { builder.register(c.as_raw_connection()) }
    }
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

/// `duckdb_delete_callback_t`：drop 掉 `Box<T>`。
///
/// `duckdb_delete_callback_t`: drops the `Box<T>`.
///
/// # Safety
///
/// `ptr` 必须是 `Box::into_raw(Box<T>)` 的结果，或为空。
///
/// `ptr` must be the result of `Box::into_raw(Box<T>)`, or null.
unsafe extern "C" fn drop_boxed<T>(ptr: *mut c_void) {
    if !ptr.is_null() {
        // SAFETY: 调用方保证 ptr 来自 Box::into_raw，且只 drop 一次。
        drop(unsafe { Box::from_raw(ptr.cast::<T>()) });
    }
}
