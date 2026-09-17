//! COPY TO 适配层：把一个「按批写行」的 Rust 函数注册成 DuckDB 的 COPY 函数（自定义文件格式）。
//!
//! Copy-`TO` adapter: registers a Rust function that writes rows batch by batch as a DuckDB copy
//! function (a custom file format).
//!
//! 与静态列版本的关键区别：**输出 schema 是运行时的**。适配层在 bind 阶段把查询结果各列的
//! `LogicalType` 反推成 [`DuckTypeDesc`]，组成 [`DuckResultSchema`]；sink 阶段再把每个数据块按这份
//! schema 读成 [`DuckDynamicRow`]，因此 `LIST` / `STRUCT` / `MAP` 等嵌套列同样可以写出，而不是只有
//! 标量。
//!
//! The key difference from the static-column flavour: **the output schema is a runtime one**. During
//! bind the adapter turns every result column's `LogicalType` into a [`DuckTypeDesc`] and assembles a
//! [`DuckResultSchema`]; during sink it reads each data chunk into [`DuckDynamicRow`]s against that
//! schema. Nested columns (`LIST` / `STRUCT` / `MAP`) can therefore be written out, not just scalars.
//!
//! 生命周期与 quack-rs 的 `CopyFunctionBuilder` 一致，四个阶段各对应一个回调：
//!
//! 1. **bind**：读出输出列（→ schema）与 `COPY ... TO (...)` 的选项（→ [`DuckCopyOptions`]），
//!    一起存成 bind data；
//! 2. **global init**：拿输出路径与 bind data，调用 [`DuckCopyToWriter::open`]；
//! 3. **sink**：每个数据块调用一次 —— 按 schema 读出动态行，交给
//!    [`CopyToFunctionAdapter::write_rows`]；
//! 4. **finalize**：调用 [`DuckCopyToWriter::finish`] 收尾。
//!
//! The life cycle matches quack-rs' `CopyFunctionBuilder`; each of the four phases maps onto one
//! callback: **bind** reads the output columns (→ schema) and the `COPY ... TO (...)` options
//! (→ [`DuckCopyOptions`]) into bind data, **global init** takes the path and that data and calls
//! [`DuckCopyToWriter::open`], **sink** runs once per data chunk (reads dynamic rows and hands them
//! to [`CopyToFunctionAdapter::write_rows`]) and **finalize** calls [`DuckCopyToWriter::finish`].
//!
//! 一般不用手写这个 impl，直接用 `#[duck_copy_function]` 作用在
//! `fn my_copy(writer: &mut MyWriter, rows: &[DuckDynamicRow]) -> DuckResult<()>` 上即可。
//!
//! Usually you do not implement this manually: annotate
//! `fn my_copy(writer: &mut MyWriter, rows: &[DuckDynamicRow]) -> DuckResult<()>` with
//! `#[duck_copy_function]`.

use crate::{
    DuckDynamicRow, DuckDynamicValue, DuckResult, DuckResultSchema, DuckTypeDesc, duck_error,
    duck_value_is_null, panic_to_string,
};
use libduckdb_sys::{
    duckdb_copy_function_bind_get_options, duckdb_copy_function_bind_info,
    duckdb_copy_function_finalize_info, duckdb_copy_function_global_init_info,
    duckdb_copy_function_sink_info, duckdb_data_chunk, duckdb_get_value_type,
};
use quack_rs::connection::Connection;
use quack_rs::copy_function::{
    CopyBindInfo, CopyFinalizeInfo, CopyFunctionBuilder, CopyGlobalInitInfo, CopySinkInfo,
};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{LogicalType, Value};
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// `COPY ... TO (...)` 的选项：有序的「选项名 -> 值」集合。
///
/// The options of `COPY ... TO (...)`: an ordered set of `option name -> value` pairs.
///
/// 选项来自 `duckdb_copy_function_bind_get_options`（单个 `STRUCT`），名字与类型都由 DuckDB 给出，
/// 因此这里不预设任何选项。查名字是**大小写不敏感**的（与 DuckDB 的 COPY 选项一致）。
///
/// The options come from `duckdb_copy_function_bind_get_options` (a single `STRUCT`), with names and
/// types supplied by DuckDB, so nothing is assumed here. Lookups are **case-insensitive**, matching
/// DuckDB's own COPY options.
///
/// 无法用 [`DuckTypeDesc`] 表达（因而读不出来）的选项会被忽略：COPY 选项的类型由 DuckDB 决定，
/// 为一个无关选项让整条 `COPY` 失败不合理。
///
/// Options that cannot be expressed as a [`DuckTypeDesc`] (and thus cannot be read) are skipped: the
/// option types are DuckDB's business, and failing a whole `COPY` over an unrelated option would not
/// be reasonable.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DuckCopyOptions {
    /// 选项，顺序与 DuckDB 给出的一致。
    ///
    /// The options, in the order DuckDB reports them.
    entries: Vec<(String, Option<DuckDynamicValue>)>,
}

impl DuckCopyOptions {
    /// 由 `(选项名, 值)` 列表构造。
    ///
    /// Builds the options from a list of `(name, value)` pairs.
    #[must_use]
    pub fn new(entries: Vec<(String, Option<DuckDynamicValue>)>) -> Self {
        Self { entries }
    }

    /// 全部选项（`None` 表示该选项的值是 NULL）。
    ///
    /// Every option (`None` means the option's value is NULL).
    #[must_use]
    pub fn entries(&self) -> &[(String, Option<DuckDynamicValue>)] {
        &self.entries
    }

    /// 按名字取值（大小写不敏感）；没有该选项时返回 `None`。
    ///
    /// Looks an option up by name (case-insensitively); `None` when it is absent.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&DuckDynamicValue> {
        self.entries
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value.as_ref())
    }

    /// 该选项是否被显式给出（哪怕值为 NULL）。
    ///
    /// Whether the option was given at all (even as NULL).
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.entries
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(name))
    }

    /// 取布尔选项；选项不存在或为 NULL 时返回 `None`。
    ///
    /// Reads a boolean option; `None` when it is absent or NULL.
    #[must_use]
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.get(name) {
            Some(DuckDynamicValue::Boolean(value)) => Some(*value),
            _ => None,
        }
    }

    /// 取整数选项（任意宽度的有符号 / 无符号整数都收进来）；不存在或为 NULL 时返回 `None`。
    ///
    /// Reads an integer option (any signed / unsigned width is accepted); `None` when it is absent
    /// or NULL.
    #[must_use]
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        match self.get(name) {
            Some(DuckDynamicValue::TinyInt(value)) => Some(i64::from(*value)),
            Some(DuckDynamicValue::SmallInt(value)) => Some(i64::from(*value)),
            Some(DuckDynamicValue::Integer(value)) => Some(i64::from(*value)),
            Some(DuckDynamicValue::BigInt(value)) => Some(*value),
            Some(DuckDynamicValue::UTinyInt(value)) => Some(i64::from(*value)),
            Some(DuckDynamicValue::USmallInt(value)) => Some(i64::from(*value)),
            Some(DuckDynamicValue::UInteger(value)) => Some(i64::from(*value)),
            Some(DuckDynamicValue::UBigInt(value)) => i64::try_from(*value).ok(),
            _ => None,
        }
    }

    /// 取字符串选项；不存在或为 NULL 时返回 `None`。
    ///
    /// Reads a string option; `None` when it is absent or NULL.
    #[must_use]
    pub fn get_str(&self, name: &str) -> Option<&str> {
        match self.get(name) {
            Some(DuckDynamicValue::Varchar(value)) => Some(value.as_str()),
            _ => None,
        }
    }

    /// 选项数量。
    ///
    /// The number of options.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否没有任何选项。
    ///
    /// Whether there are no options at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// COPY TO 的 writer 状态：由 [`DuckCopyToWriter::open`] 创建，跨数据块复用。
///
/// The writer state of a `COPY ... TO` format: created by [`DuckCopyToWriter::open`] and reused
/// across data chunks.
///
/// 实现者通常持有「已打开的输出文件」与 bind 阶段定下的 [`DuckResultSchema`]（写表头、按列渲染都
/// 需要它），并在 [`Self::finish`] 里 flush / 关闭。
///
/// An implementor typically owns an open output file plus the [`DuckResultSchema`] fixed during bind
/// (needed for headers and per-column rendering), and flushes / closes it in [`Self::finish`].
pub trait DuckCopyToWriter: Sized + 'static {
    /// global init 阶段：打开输出目标。
    ///
    /// `path` 是 `COPY ... TO 'path'` 里的目标路径；`schema` 是查询结果的列名与逻辑类型
    /// （bind 阶段由适配层从 [`CopyBindInfo`] 读出并反推）；`options` 是 `COPY ... TO (...)` 里除
    /// `FORMAT` 之外的选项。
    ///
    /// 写表头这类「只需要做一次」的事情放在这里做最合适（例如 `header` 选项）。
    ///
    /// Global-init phase: opens the output target. `path` is the destination of
    /// `COPY ... TO 'path'`; `schema` holds the result column names and logical types (read and
    /// reconstructed from [`CopyBindInfo`] during bind); `options` holds the `COPY ... TO (...)`
    /// options other than `FORMAT`. One-off work such as writing a header (say for a `header`
    /// option) belongs here.
    fn open(path: &str, schema: &DuckResultSchema, options: &DuckCopyOptions) -> DuckResult<Self>;

    /// finalize 阶段：flush / 关闭输出目标（默认什么都不做）。
    ///
    /// Finalize phase: flushes / closes the output target (a no-op by default).
    fn finish(&mut self) -> DuckResult<()> {
        Ok(())
    }
}

/// 把 [`DuckCopyToWriter`] 的四个生命周期阶段接到 DuckDB 的 COPY 回调上。
///
/// Wires the four life-cycle phases of a [`DuckCopyToWriter`] onto DuckDB's copy callbacks.
///
/// 适配层负责：
///
/// - 把 bind 阶段读到的 schema 与选项存成 bind data，并在 global init / sink 阶段取回；
/// - sink 阶段按 schema 把数据块读成 [`DuckDynamicRow`]（每列只建一次读取器），再交给用户函数；
/// - 用 `Box` 承载 writer，通过 `set_global_state` 交给 DuckDB，并注册析构回调负责 drop；
/// - 每个阶段都用 `catch_unwind` 包住，panic 与 `Err` 都转成查询错误（`set_error`），
///   不会跨 FFI 展开。
///
/// The adapter stores the schema and options read during bind as bind data (retrieved again during
/// global init / sink), reads each data chunk into [`DuckDynamicRow`]s during sink (one reader per
/// column), boxes the writer and hands it to DuckDB through `set_global_state` with a destructor
/// callback that drops it, and wraps every phase in `catch_unwind` so that panics and `Err`s become
/// query errors (`set_error`) instead of unwinding across FFI.
pub trait CopyToFunctionAdapter: Sized + 'static {
    /// 回调名称，同时用作 `COPY ... (FORMAT <name>)` 里的格式名。
    ///
    /// The callback name, also used as the format name in `COPY ... (FORMAT <name>)`.
    const NAME: &'static str;

    /// writer 状态类型（由 [`DuckCopyToWriter`] 描述）。
    ///
    /// The writer state type (described by [`DuckCopyToWriter`]).
    type Writer: DuckCopyToWriter;

    /// sink 阶段：写出一批动态行（由宏生成的代码转调被标注的函数）。
    ///
    /// 行与 bind 阶段定下的 schema 逐列对应；`None` 单元格就是 SQL NULL。**逐批的写出逻辑放在被
    /// `#[duck_copy_function]` 标注的函数里**，[`DuckCopyToWriter`] 只管「打开 / 收尾」。
    ///
    /// Sink phase: writes one batch of dynamic rows (the macro-generated code forwards this to the
    /// annotated function). The rows line up column by column with the schema fixed during bind, and
    /// a `None` cell is SQL NULL. **The per-batch writing logic lives in the function annotated with
    /// `#[duck_copy_function]`**; [`DuckCopyToWriter`] only owns "open / close".
    fn write_rows(writer: &mut Self::Writer, rows: &[DuckDynamicRow]) -> DuckResult<()>;

    /// bind 阶段：把输出列反推成 [`DuckResultSchema`]、读出 COPY 选项，一起存成 bind data。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Bind phase: reconstructs the output columns into a [`DuckResultSchema`] and reads the COPY
    /// options, storing both as bind data. Called by DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_bind(raw: duckdb_copy_function_bind_info) {
        // SAFETY: raw 由 DuckDB 传入，在回调期间有效。
        //
        // SAFETY: `raw` comes from DuckDB and is valid for the duration of the callback.
        let info = unsafe { CopyBindInfo::new(raw) };
        handle(
            AssertUnwindSafe(|| {
                let schema = Self::read_schema(&info)?;
                // SAFETY: raw 在回调期间有效；选项 value 的所有权留在 DuckDB 那边（见
                // `read_copy_options`）。
                //
                // SAFETY: `raw` is valid for the callback; the options value stays owned by DuckDB
                // (see `read_copy_options`).
                let options = unsafe { read_copy_options(raw)? };
                let boxed = Box::new(CopyToBindData { schema, options });
                // SAFETY: Box 的所有权交给 DuckDB，析构回调负责 drop。
                //
                // SAFETY: ownership of the Box goes to DuckDB; the destructor drops it.
                unsafe {
                    info.set_bind_data(
                        Box::into_raw(boxed).cast::<c_void>(),
                        Some(drop_boxed::<CopyToBindData>),
                    );
                }
                Ok(())
            }),
            |message| info.set_error(message),
        );
    }

    /// global init 阶段：取回 bind data 与输出路径，调用 [`DuckCopyToWriter::open`]。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Global-init phase: retrieves the bind data and the output path, then calls
    /// [`DuckCopyToWriter::open`]. Called by DuckDB; `info` is guaranteed valid.
    unsafe extern "C" fn c_global_init(info: duckdb_copy_function_global_init_info) {
        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CopyGlobalInitInfo::new(info) };
        handle(
            AssertUnwindSafe(|| {
                // SAFETY: 回调期间有效；DuckDB 负责释放它自己的缓冲区。
                let path = unsafe { info.get_file_path() };
                let bind = bind_data_of(&info)?;
                let writer = Self::Writer::open(&path, &bind.schema, &bind.options)?;
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

    /// sink 阶段：取回 writer 与 schema，把当前数据块读成动态行后写出。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 与 `chunk` 均由 DuckDB 保证有效。
    ///
    /// Sink phase: retrieves the writer and the schema, reads the current data chunk into dynamic
    /// rows and writes them out. Called by DuckDB; `info` and `chunk` are guaranteed valid.
    unsafe extern "C" fn c_sink(raw: duckdb_copy_function_sink_info, raw_chunk: duckdb_data_chunk) {
        // SAFETY: raw 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CopySinkInfo::new(raw) };
        handle(
            AssertUnwindSafe(|| {
                let writer = writer_of::<Self::Writer>(&info)?;
                let bind = bind_data_of(&info)?;
                // SAFETY: chunk 由 DuckDB 传入，在回调期间有效。
                let chunk = unsafe { DataChunk::from_raw(raw_chunk) };
                let rows = DuckDynamicRow::read_batch(&chunk, &bind.schema)?;
                Self::write_rows(writer, &rows)
            }),
            |message| info.set_error(message),
        );
    }

    /// finalize 阶段：取回 writer 并调用 [`DuckCopyToWriter::finish`]。
    ///
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` 由 DuckDB 保证有效。
    ///
    /// Finalize phase: retrieves the writer and calls [`DuckCopyToWriter::finish`]. Called by
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

    /// 把 COPY 的输出列读成动态 schema：列类型经 [`DuckTypeDesc::from_logical_type`] 反推，列名合成。
    ///
    /// COPY 的输出列没有名字（它们来自一个查询而不是一张表），因此这里合成 `column_0`、`column_1`…
    /// 需要真实列名的格式应自己定义选项来携带，而不是猜。
    ///
    /// Reads the COPY output columns into a dynamic schema: the types go through
    /// [`DuckTypeDesc::from_logical_type`] and the names are synthesised, because COPY output columns
    /// are unnamed (they come from a query, not a table) — hence `column_0`, `column_1`, ... A format
    /// that needs real names should carry them through its own option rather than guess.
    ///
    /// # Errors
    ///
    /// 某一列的类型无法用 [`DuckTypeDesc`] 表达时返回错误（`ENUM` / `ARRAY` / `UNION` / `BIT`）。
    ///
    /// Returns an error when a column's type cannot be expressed as a [`DuckTypeDesc`] (`ENUM` /
    /// `ARRAY` / `UNION` / `BIT`).
    fn read_schema(info: &CopyBindInfo) -> DuckResult<DuckResultSchema> {
        let count = info.column_count();
        let mut columns = Vec::with_capacity(count as usize);
        for index in 0..count {
            // SAFETY: index < column_count()；返回的 LogicalType 由我们持有并在本轮结束时释放。
            let logical_type = unsafe { info.column_type(index) };
            columns.push((
                format!("column_{index}"),
                DuckTypeDesc::from_logical_type(&logical_type)?,
            ));
        }
        Ok(DuckResultSchema::new(columns))
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

/// bind 阶段存下的东西：输出 schema + COPY 选项。
///
/// What bind stores: the output schema plus the COPY options.
struct CopyToBindData {
    /// 输出列定义。
    ///
    /// The output column definitions.
    schema: DuckResultSchema,
    /// `COPY ... TO (...)` 的选项。
    ///
    /// The `COPY ... TO (...)` options.
    options: DuckCopyOptions,
}

/// 读出 `COPY ... TO (...)` 的选项。
///
/// Reads the `COPY ... TO (...)` options.
///
/// 选项是以单个 `STRUCT` 值给出的：先用 `duckdb_get_value_type` 拿到它的逻辑类型（从而拿到字段名与
/// 字段类型），再逐字段转成动态值。类型无法用 [`DuckTypeDesc`] 表达、或值读不出来的选项会被跳过。
///
/// The options arrive as a single `STRUCT` value: `duckdb_get_value_type` gives its logical type
/// (hence the field names and field types) and every field is then converted into a dynamic value.
/// Options whose type cannot be expressed as a [`DuckTypeDesc`], or whose value cannot be read, are
/// skipped.
///
/// # Safety
///
/// `raw` 必须是 bind 回调期间有效的 `duckdb_copy_function_bind_info`。
///
/// `raw` must be a `duckdb_copy_function_bind_info` valid for the duration of the bind callback.
unsafe fn read_copy_options(raw: duckdb_copy_function_bind_info) -> DuckResult<DuckCopyOptions> {
    // SAFETY: 由调用方保证 raw 有效。
    //
    // SAFETY: the caller guarantees `raw`.
    let value = unsafe { Value::from_raw(duckdb_copy_function_bind_get_options(raw)) };
    let options = parse_copy_options(&value);
    // 把句柄**还给 DuckDB**：它返回的是一个由它自己持有（生命周期覆盖整个 bind 回调）的 value，
    // 让 `Value` 的 Drop 对它调用 `duckdb_destroy_value` 会造成堆损坏（`0xC0000374`，实测）。所以
    // 这里只借用它的访问器，然后 `into_raw` 放弃所有权。
    //
    // Hand the handle **back to DuckDB**: it returns a value it owns itself (valid for the whole bind
    // callback), and letting `Value`'s Drop call `duckdb_destroy_value` on it corrupts the heap
    // (`0xC0000374`, observed in practice). So only its accessors are borrowed and `into_raw` gives the
    // ownership back.
    let _ = value.into_raw();
    options
}

/// 解析选项的那一个 `STRUCT`：只借用 `value`，完全不涉及所有权。
///
/// Parses the single `STRUCT` of options: it only borrows `value` and never touches ownership.
///
/// 单独拆出来是为了让上面的 [`read_copy_options`] 成为唯一的出口 —— 中途 `?` 返回也不会提前 drop
/// 掉那个不能销毁的 handle。
///
/// It is split out so that [`read_copy_options`] has a single exit point: an early `?` return can no
/// longer drop the handle that must not be destroyed.
fn parse_copy_options(value: &Value) -> DuckResult<DuckCopyOptions> {
    if duck_value_is_null(value) {
        return Ok(DuckCopyOptions::default());
    }

    // SAFETY: value 非 NULL，其逻辑类型由 DuckDB 返回、由我们拥有并在作用域结束时释放（释放它是安全
    // 的，只有 value 本身不能释放）。
    //
    // SAFETY: the value is non-null and DuckDB returns its logical type, which we own and release at
    // the end of the scope (releasing *it* is fine; only the value itself must not be released).
    let logical_type = unsafe { LogicalType::from_raw(duckdb_get_value_type(value.as_raw())) };
    // SAFETY: 上面的逻辑类型来自一个 STRUCT 值。
    let count = unsafe { logical_type.struct_child_count() };

    let mut entries = Vec::with_capacity(count as usize);
    for index in 0..count {
        // SAFETY: index < struct_child_count()。
        let name = unsafe { logical_type.struct_child_name(index) };
        // SAFETY: index < struct_child_count()。
        let field_type = unsafe { logical_type.struct_child_type(index) };
        let Ok(desc) = DuckTypeDesc::from_logical_type(&field_type) else {
            // 选项类型表达不了（例如 ENUM）：跳过而不是让整条 COPY 失败。
            //
            // The option type cannot be expressed (say ENUM): skip it rather than fail the COPY.
            continue;
        };
        // `struct_child` 返回的是**新的、由我们拥有**的 value（DuckDB 会为子值分配），所以它可以
        // 正常释放。
        //
        // `struct_child` returns a **new value owned by us** (DuckDB allocates one for the child), so
        // it is released normally.
        let child = match value.struct_child(index as usize) {
            Some(child) => DuckDynamicValue::from_duck_value(&child, &desc)?,
            None => None,
        };
        entries.push((name, child));
    }
    Ok(DuckCopyOptions::new(entries))
}

/// 从回调信息里取回 bind data；缺失时报错。
///
/// Retrieves the bind data from a callback info record, reporting an error when it is missing.
fn bind_data_of(info: &impl BindDataAccessor) -> DuckResult<&CopyToBindData> {
    // SAFETY: 回调期间有效；bind 阶段一定设置过 bind data。
    let ptr = unsafe { info.bind_data() }.cast::<CopyToBindData>();
    if ptr.is_null() {
        return Err(duck_error(
            "copy function: bind data was not initialized; was the bind callback skipped?",
        ));
    }
    // SAFETY: 非空指针指向 bind 阶段放入的 `CopyToBindData`，生命周期覆盖整个查询。
    Ok(unsafe { &*ptr })
}

/// 从回调信息里取回 writer；缺失时报错。
///
/// Retrieves the writer from a callback info record, reporting an error when it is missing.
fn writer_of<W>(info: &impl GlobalStateAccessor) -> DuckResult<&mut W> {
    // SAFETY: 回调期间有效；global init 阶段一定设置过 global state。
    let ptr = unsafe { info.global_state() }.cast::<W>();
    if ptr.is_null() {
        return Err(duck_error(
            "copy function: writer was not initialized; was global init skipped?",
        ));
    }
    // SAFETY: 非空指针指向 global init 阶段放入的 writer，且同一时刻只有一个 sink 回调。
    Ok(unsafe { &mut *ptr })
}

/// 「能取 bind data」的回调信息：`CopyGlobalInitInfo` 与 `CopySinkInfo` 都实现了它。
///
/// The callback info records that can hand out bind data: both `CopyGlobalInitInfo` and
/// `CopySinkInfo` implement it.
trait BindDataAccessor {
    /// 取回 bind data 的裸指针（可能为空）。
    ///
    /// Returns the raw bind-data pointer (possibly null).
    unsafe fn bind_data(&self) -> *mut c_void;
}

impl BindDataAccessor for CopyGlobalInitInfo {
    /// 委托给 quack-rs 的 `get_bind_data`。
    ///
    /// Delegates to quack-rs' `get_bind_data`.
    unsafe fn bind_data(&self) -> *mut c_void {
        // SAFETY: 由调用方保证回调期间有效。
        unsafe { self.get_bind_data() }
    }
}

impl BindDataAccessor for CopySinkInfo {
    /// 委托给 quack-rs 的 `get_bind_data`。
    ///
    /// Delegates to quack-rs' `get_bind_data`.
    unsafe fn bind_data(&self) -> *mut c_void {
        // SAFETY: 由调用方保证回调期间有效。
        unsafe { self.get_bind_data() }
    }
}

/// 「能取 global state」的回调信息：`CopySinkInfo` 与 `CopyFinalizeInfo` 都实现了它。
///
/// The callback info records that can hand out the global state: both `CopySinkInfo` and
/// `CopyFinalizeInfo` implement it.
trait GlobalStateAccessor {
    /// 取回 global state 的裸指针（可能为空）。
    ///
    /// Returns the raw global-state pointer (possibly null).
    unsafe fn global_state(&self) -> *mut c_void;
}

impl GlobalStateAccessor for CopySinkInfo {
    /// 委托给 quack-rs 的 `get_global_state`。
    ///
    /// Delegates to quack-rs' `get_global_state`.
    unsafe fn global_state(&self) -> *mut c_void {
        // SAFETY: 由调用方保证回调期间有效。
        unsafe { self.get_global_state() }
    }
}

impl GlobalStateAccessor for CopyFinalizeInfo {
    /// 委托给 quack-rs 的 `get_global_state`。
    ///
    /// Delegates to quack-rs' `get_global_state`.
    unsafe fn global_state(&self) -> *mut c_void {
        // SAFETY: 由调用方保证回调期间有效。
        unsafe { self.get_global_state() }
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
