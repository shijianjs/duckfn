//! DuckDB 值类型的核心 trait `DuckValueType`，以及按行/按批读写的辅助类型。
//!
//! The core `DuckValueType` trait plus the helpers for row-wise and batch read/write.

use crate::value_types::vector_layout::struct_field;
use crate::{DuckOptionResult, DuckResult};
use libduckdb_sys::{duckdb_is_null_value, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{LogicalType, TypeId, Value, VectorReader, VectorWriter};
use std::fmt::Debug;
use std::sync::{Arc, Weak};

/// Rust 类型与 DuckDB 逻辑类型之间的映射：既负责声明 DuckDB 类型，也负责从向量读、往向量写。
///
/// 映射规则：
/// - 如果 Rust 基础类型已经完整表达了业务语义，可以直接映射；
/// - 如果多个逻辑类型共享同一个物理表示，就应该 newtype 包装。
///
/// 实现者通常只重写 `read_valid_by_vector_reader` 与 `write_valid_to_vector_writer`
/// （或者 `read_valid` / `write_valid` 以便自定义 NULL 处理），其余走默认实现。
///
/// The mapping between a Rust type and a DuckDB logical type: it both declares the DuckDB type
/// and knows how to read from and write to DuckDB vectors.
///
/// Mapping rules: if a Rust primitive already expresses the business semantics fully, map it
/// directly; if several logical types share one physical representation, wrap them in newtypes.
/// Implementors usually override only `read_valid_by_vector_reader` and
/// `write_valid_to_vector_writer` (or `read_valid` / `write_valid` to customise NULL handling)
/// and rely on the defaults for everything else.
pub trait DuckValueType: Clone + Debug + Sized + Send + Sync + 'static {
    /// 对应的 DuckDB 类型 id。
    ///
    /// The corresponding DuckDB type id.
    fn type_id() -> TypeId;
    /// 对应的 DuckDB 逻辑类型；默认由 [`Self::type_id`] 构造，
    /// 复杂类型（LIST / MAP / ARRAY / STRUCT / DECIMAL）会重写它。
    ///
    /// The corresponding DuckDB logical type; by default built from [`Self::type_id`]. Complex
    /// types (LIST / MAP / ARRAY / STRUCT / DECIMAL) override it.
    fn logical_type() -> LogicalType {
        LogicalType::new(Self::type_id())
    }

    /// 本类型能否用「值」表示 SQL NULL：`Some(v)` 表示可以（`Option<T>` 返回 `Some(None)`），
    /// `None` 表示存不下 NULL。
    ///
    /// 容器（LIST / ARRAY / MAP）与结构体字段读取时据此决定：读到 NULL 时是「该位置的值就是
    /// NULL」（取 `Some(v)`）还是「本类型表达不了 NULL，整个值作废」（`None`，整行变成 NULL）。
    ///
    /// 默认实现返回 `None`，因此自定义类型无需改动；只有 [`Option<T>`](Option) 覆写它。
    ///
    /// Whether this type can represent SQL NULL *as a value*: `Some(v)` means yes (`Option<T>`
    /// returns `Some(None)`), `None` means the type cannot hold NULL. Container (LIST / ARRAY /
    /// MAP) and struct-field reads use it to decide whether a NULL slot becomes that slot's value
    /// (`Some(v)`) or invalidates the whole value (`None`, i.e. the row becomes NULL). The
    /// default returns `None`, so existing custom types need no change; only `Option<T>`
    /// overrides it.
    fn from_null() -> Option<Self> {
        None
    }

    /// 为 `chunk` 的第 `column_index` 列创建读取器。
    ///
    /// Creates a reader for column `column_index` of `chunk`.
    fn create_reader(chunk: &DataChunk, column_index: usize) -> DuckValueReader {
        let vector = unsafe { chunk.vector(column_index) };
        let size = chunk.size();
        Self::create_reader_from_vector(vector, size)
    }
    /// 直接为一个裸向量创建读取器；复杂类型会额外为子向量创建子读取器。
    ///
    /// Creates a reader straight from a raw vector; complex types additionally build child
    /// readers for their sub-vectors.
    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckValueReader::new_from_vector(vector, size)
    }

    /// 读取第 `row` 行的值，仅处理null，子类不能重写。
    ///
    /// 向量里该行有效时交给 [`Self::read_valid`]，否则返回 `None`（SQL NULL）。
    ///
    /// Reads the value at row `row`; handles NULL only, and subclasses must not override it.
    /// When the row is valid it delegates to [`Self::read_valid`]; otherwise it returns `None`
    /// (SQL NULL).
    fn read(reader: &DuckValueReader, row: usize) -> Option<Self> {
        if unsafe { reader.vector_reader.is_valid(row) } {
            Self::read_valid(reader, row)
        } else {
            None
        }
    }

    /// 读取一个确定有效的行；默认转发给 [`Self::read_valid_by_vector_reader`]。
    ///
    /// Reads a row known to be valid; by default it forwards to
    /// [`Self::read_valid_by_vector_reader`].
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self::read_valid_by_vector_reader(
            &reader.vector_reader,
            row,
        ))
    }

    /// 读取一个「槽位」（LIST/ARRAY 的元素、MAP 的键或值、STRUCT 的字段）。
    ///
    /// 与 [`Self::read`] 的区别只在 NULL：`read` 一律把 NULL 变成 `None`，而这里在 NULL 时
    /// 会尝试 [`Self::from_null`] —— 类型能表示 NULL（`Option<T>`）就取到空值，取不到才返回
    /// `None` 让上层把整个值当作 NULL。容器与结构体字段的读取都走这里。
    ///
    /// Reads one *slot* (a LIST/ARRAY element, a MAP key or value, a STRUCT field). It differs
    /// from [`Self::read`] only for NULL: `read` always maps NULL to `None`, whereas this falls
    /// back to [`Self::from_null`] — a nullable type yields its empty value and only a type that
    /// cannot hold NULL returns `None`, invalidating the enclosing value.
    fn read_slot(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Self::read(reader, row).or_else(Self::from_null)
    }

    /// [`Self::read_slot`] 的 [`Value`] 版本（表函数 bind 参数等自描述数据）。
    ///
    /// `Ok(None)` 表示该槽位是 NULL 且本类型表达不了 NULL，调用方据此给出自己的错误信息。
    ///
    /// The [`Value`] flavour of [`Self::read_slot`] (used for table-function bind arguments and
    /// other self-describing data). `Ok(None)` means the slot is NULL and the type cannot
    /// represent NULL; callers turn that into their own error message.
    fn read_slot_by_duck_value(value: &Value) -> DuckResult<Option<Self>> {
        Ok(Self::read_by_duck_value(value)?.or_else(Self::from_null))
    }

    /// 从裸 [`VectorReader`] 读取一个有效值（子类实现）。
    ///
    /// Reads one valid value from the raw [`VectorReader`] (implemented by subclasses).
    fn read_valid_by_vector_reader(_reader: &VectorReader, _row: usize) -> Self {
        todo!("subclass must implement read_valid_by_vector_reader")
    }

    /// 把一批（可为 NULL 的）值写入输出向量。
    ///
    /// 流程：创建批量写入器 -> 逐行调用 [`Self::write`] -> 调用 [`Self::write_finish`] 收尾。
    ///
    /// Writes a batch of (possibly NULL) values into the output vector: it creates the batch
    /// writer, calls [`Self::write`] for every row and finishes with [`Self::write_finish`].
    fn write_batch(output: duckdb_vector, output_vec: &[Option<&Self>]) {
        let mut writer = Self::create_writer_batch(output, output_vec);
        for (idx, result) in output_vec.iter().enumerate() {
            Self::write(&mut writer, idx, *result);
        }
        Self::write_finish(&mut writer);
    }

    /// 创建批量写入器；复杂类型会在这里预留并挂上子写入器。
    ///
    /// Creates the batch writer; complex types reserve space and attach child writers here.
    fn create_writer_batch(vector: duckdb_vector, _output_vec: &[Option<&Self>]) -> DuckValueWriter {
        // Self::create_writer(vector)
        DuckValueWriter::new_from_vector(vector)
    }

    /// 写入一行：仅处理null，子类不能重写、因为可能调不到。
    ///
    /// `None` 走 [`Self::write_null`]，`Some` 走 [`Self::write_valid`]。
    ///
    /// Writes one row; handles NULL only and subclasses must not override it (it may never be
    /// called). `None` goes to [`Self::write_null`] and `Some` to [`Self::write_valid`].
    fn write(writer: &mut DuckValueWriter, idx: usize, vo: Option<&Self>) {
        // Self::write_to_vector(&mut writer.vector_writer, idx, vo);
        match vo {
            None => Self::write_null(writer, idx),

            Some(v) => Self::write_valid(writer, idx, v),
        }
    }

    /// 仅处理null：默认只把当前向量置为NULL。
    ///
    /// - 子类可重写，用于同步处理子向量；
    /// - 例如struct：DuckDB的struct_extract直接重解释子向量、不检查父向量的validity，
    ///   父向量置NULL后子向量仍是未初始化内存，所以必须把子字段一起置NULL。
    ///
    /// Handles NULL only: by default it just marks the current vector entry as NULL.
    /// Subclasses may override it to propagate NULL into child vectors — for instance a STRUCT,
    /// because DuckDB's `struct_extract` reinterprets child vectors without checking the parent's
    /// validity, so leaving children untouched would expose uninitialised memory.
    fn write_null(writer: &mut DuckValueWriter, idx: usize) {
        unsafe { writer.vector_writer.set_null(idx) }
    }

    /// 写入一个确定有效的值。
    ///
    /// 外部可以调用write_valid
    /// - 只要已经处理了null，就不需要管其他的
    ///
    /// Writes a value known to be valid. External code may call `write_valid` directly: as long
    /// as NULL has already been handled elsewhere, nothing else needs attention.
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        Self::write_valid_to_vector_writer(&mut writer.vector_writer, idx, vo)
    }

    /// 往裸 [`VectorWriter`] 写一个有效值（子类实现）。
    ///
    /// Writes one valid value into the raw [`VectorWriter`] (implemented by subclasses).
    fn write_valid_to_vector_writer(_writer: &mut VectorWriter, _idx: usize, _v: &Self) {
        todo!("subclass must implement write_valid")
    }

    /// 批量写入收尾：默认什么都不做。
    ///
    /// 复杂类型（LIST / MAP）会在这里设置子向量长度等收尾信息。
    ///
    /// Finishes a batch write; a no-op by default. Complex types (LIST / MAP) set child-vector
    /// lengths here.
    fn write_finish(_writer: &mut DuckValueWriter) {}

    /// 从 STRUCT 向量中取出第 `field_index` 个子字段的读取器。
    ///
    /// Builds a reader for child field `field_index` of a STRUCT vector.
    fn struct_field_reader(struct_reader: &DuckValueReader, field_index: usize) -> DuckValueReader {
        let row_count = struct_reader.vector_reader.row_count();
        let vector = struct_reader.c_duckdb_vector;
        let field_vector = struct_field(vector, field_index);
        Self::create_reader_from_vector(field_vector, row_count)
    }

    /// 为 STRUCT 的第 `field_index` 个子字段创建批量写入器。
    ///
    /// Creates a batch writer for child field `field_index` of a STRUCT.
    fn struct_field_writer_batch(
        struct_writer: &DuckValueWriter,
        field_index: usize,
        output_vec: &[Option<&Self>],
    ) -> DuckValueWriter {
        let vector = struct_writer.c_duckdb_vector;
        let field_vector = struct_field(vector, field_index);
        Self::create_writer_batch(field_vector, output_vec)
    }

    /// 从 [`Value`] 读出一个值（表函数解析参数时使用）；NULL 返回 `Ok(None)`。
    ///
    /// Reads a value from a [`Value`] (used when table functions parse their arguments); NULL
    /// yields `Ok(None)`.
    fn read_by_duck_value(value: &Value) -> DuckOptionResult<Self> {
        if duck_value_is_null(value) {
            Ok(None)
        } else {
            Ok(Some(Self::read_by_duck_value_valid(value)?))
        }
    }
    /// 从一个确定非 NULL 的 [`Value`] 读出值；默认转发给
    /// [`Self::read_by_duck_value_valid_simple`]。
    ///
    /// Reads a value from a non-NULL [`Value`]; by default it forwards to
    /// [`Self::read_by_duck_value_valid_simple`].
    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Self::read_by_duck_value_valid_simple(value))
    }
    /// 从非 NULL 的 [`Value`] 直接取值（子类实现，不涉及 NULL 判定）。
    ///
    /// Extracts the value from a non-NULL [`Value`] (implemented by subclasses; no NULL check).
    fn read_by_duck_value_valid_simple(_value: &Value) -> Self {
        todo!("subclass must implement read_by_duck_value_valid_simple")
    }
}

/// 用来给宏校验 DuckValueType 是否被类型实现。
///
/// Used by the macros to assert at compile time that a type implements [`DuckValueType`].
pub fn assert_impl_duck_value_type<T: DuckValueType>() {}

/// 判断一个 [`Value`] 是否为 SQL NULL —— **只调 `Value::is_null()` 是不够的**。
///
/// quack-rs 的 `Value::is_null()` 只看句柄指针是否为空，这在表函数 bind 参数上有漏洞：SQL 里
/// 显式写 `arg = NULL` 时句柄并非空指针，`is_null()` 返回 `false`，随后按类型取值就会撞上 FFI 里
/// 的 foreign exception（`fatal runtime error: Rust cannot catch foreign exceptions, aborting`）；
/// 只有**省略**该参数才会拿到空指针。这里补一次 `duckdb_is_null_value` 判定，两种情况都能识别。
///
/// 表函数参数的读取路径（[`DuckValueType::read_by_duck_value`]、动态列的
/// `DuckDynamicValue::from_duck_value`）都必须走这里，不要各自判断。
///
/// Whether a [`Value`] is SQL NULL — **calling `Value::is_null()` alone is not enough**. That check
/// only looks at whether the handle pointer is null, which is not the whole story for table-function
/// bind arguments: writing `arg = NULL` in SQL yields a non-null handle, so `is_null()` reports
/// `false` and the type-specific read then hits a foreign exception inside the FFI (`fatal runtime
/// error: Rust cannot catch foreign exceptions, aborting`); only *omitting* the argument gives a
/// null handle. Adding the `duckdb_is_null_value` check below covers both cases.
///
/// Every read path for table-function arguments ([`DuckValueType::read_by_duck_value`], the dynamic
/// `DuckDynamicValue::from_duck_value`) goes through here instead of testing on its own.
#[must_use]
pub fn duck_value_is_null(value: &Value) -> bool {
    value.is_null() ||
        // SAFETY: value.as_raw() 是这个 Value 自己持有的有效句柄。
        //
        // SAFETY: value.as_raw() is the valid handle owned by this Value.
        unsafe { duckdb_is_null_value(value.as_raw()) }
}

/// 一个 [`DuckValueReader`] 的存活凭证：reader 还在，凭证就在；reader 被释放后，
/// 由它派生的 [`DuckLazy`](crate::DuckLazy) 才能发现自己已经失效。
///
/// A liveness token for one [`DuckValueReader`]: it lives exactly as long as the reader, which is
/// what lets a [`DuckLazy`](crate::DuckLazy) derived from it notice that it has gone stale.
pub(crate) struct ChunkToken;

/// 一行值的读取器：同时持有 quack-rs 的高层 [`VectorReader`] 与 DuckDB 的裸向量。
///
/// A reader for one row of values: it holds both the high-level quack-rs [`VectorReader`] and
/// the raw DuckDB vector.
pub struct DuckValueReader {
    /// rust api, 和下面的c_duckdb_vector一比一对应
    ///
    /// The Rust-side API; in one-to-one correspondence with `c_duckdb_vector` below.
    pub vector_reader: VectorReader,
    /// c api, 和上面的reader一比一对应
    ///
    /// The C-side API; in one-to-one correspondence with the reader above.
    pub c_duckdb_vector: duckdb_vector,
    /// 基于上面的c_duckdb_vector的子reader
    ///
    /// Child readers built on top of `c_duckdb_vector` (e.g. LIST elements, STRUCT fields).
    pub child_reader: Vec<DuckValueReader>,
    /// 存活凭证：拿到一个 reader，就等于拿到了「这块向量此刻有效」的证明。
    ///
    /// [`DuckLazy`](crate::DuckLazy) 只持有它的 [`Weak`]，于是「源 reader 是否还在」就成了唯一
    /// 的有效性判据 —— 回调结束、局部 `readers` 被释放后，延迟取值会得到明确的报错，
    /// 而不是解引用一块已经失效的向量。
    ///
    /// The liveness token: holding a reader is holding proof that this vector is valid right now.
    /// [`DuckLazy`](crate::DuckLazy) keeps only a [`Weak`] to it, so "is the source reader still
    /// alive?" becomes the one and only validity check — once the callback returns and the local
    /// `readers` are dropped, a deferred read reports an error instead of dereferencing a stale
    /// vector.
    pub(crate) token: Arc<ChunkToken>,
}

impl DuckValueReader {
    /// 为 `chunk` 的第 `column_index` 列创建读取器。
    ///
    /// Creates a reader for column `column_index` of `chunk`.
    pub fn new_from_chunk(chunk: &DataChunk, column_index: usize) -> Self {
        let vector = unsafe { chunk.vector(column_index) };
        let size = chunk.size();
        Self::new_from_vector(vector, size)
    }

    /// 从一个裸向量创建读取器（`child_reader` 为空，由复杂类型自行填充）。
    ///
    /// Creates a reader from a raw vector (`child_reader` starts empty and is filled in by
    /// complex types).
    //
    // 裸指针由 DuckDB FFI 提供，此处直接解引用
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn new_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckValueReader {
            vector_reader: unsafe { VectorReader::from_vector(vector, size) },
            c_duckdb_vector: vector,
            child_reader: vec![],
            token: Arc::new(ChunkToken),
        }
    }

    /// 本 reader 存活凭证的弱引用，交给 [`DuckLazy`](crate::DuckLazy) 判断自己是否还有效。
    ///
    /// A weak reference to this reader's liveness token, handed to
    /// [`DuckLazy`](crate::DuckLazy) so it can tell whether it is still usable.
    pub(crate) fn alive_weak(&self) -> Weak<ChunkToken> {
        Arc::downgrade(&self.token)
    }
}

/// 一批值的写入器：同时持有 quack-rs 的高层 [`VectorWriter`] 与 DuckDB 的裸向量。
///
/// A writer for a batch of values: it holds both the high-level quack-rs [`VectorWriter`] and
/// the raw DuckDB vector.
pub struct DuckValueWriter {
    /// Rust 侧写入 API。
    ///
    /// The Rust-side write API.
    pub vector_writer: VectorWriter,
    /// 与 `vector_writer` 对应的裸向量。
    ///
    /// The raw vector corresponding to `vector_writer`.
    pub c_duckdb_vector: duckdb_vector,
    /// 子向量写入器（LIST 元素 / MAP 键值 / STRUCT 字段）。
    ///
    /// Child writers (LIST elements, MAP keys/values, STRUCT fields).
    pub child_writer: Vec<DuckValueWriter>,
    /// 当前已写入 LIST / MAP 子向量的元素偏移量。
    ///
    /// Current element offset written to the LIST / MAP child vector.
    pub offset: usize,
    // pub list_builder:Option<ListBuilder>,
}

impl DuckValueWriter {
    /// 从一个裸向量创建批量写入器。
    ///
    /// Creates a batch writer from a raw vector.
    //
    // 裸指针由 DuckDB FFI 提供，此处直接解引用
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn new_from_vector(vector: duckdb_vector) -> Self {
        Self {
            vector_writer: unsafe { VectorWriter::new(vector) },
            c_duckdb_vector: vector,
            child_writer: vec![],
            offset: 0,
            // list_builder: None,
        }
    }
}

// TypeId::TimestampNs
// pub const unsafe fn write_timestamp_ns(&mut self, idx: usize, nanos_since_epoch: i64) {

// pub const unsafe fn write_date(&mut self, idx: usize, days_since_epoch: i32) {

// pub const unsafe fn write_uuid(&mut self, idx: usize, value: i128) {

// ===================================================
// 这几个类型quack-rs没做read、write方法
// These types have no read/write methods in quack-rs.
//
// TypeId::Enum
// TypeId::Union
// TypeId::Bit
// TypeId::TimeNs      // duckdb-1-5
// TypeId::Any              // duckdb-1-5
// TypeId::Varint           // duckdb-1-5
// TypeId::SqlNull          // duckdb-1-5
// TypeId::IntegerLiteral   // duckdb-1-5
// TypeId::StringLiteral    // duckdb-1-5
// TypeId::Geometry         // duckdb-1-5-3
// TypeId::Variant          // duckdb-1-5-3
//
//
//
//
// 这几个包装类型后面再说
// These wrapper types are left for later.
//
// TypeId::Map
// TypeId::Array
//
//
//
//
//
// ===================================================
