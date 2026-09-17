//! `#[derive(DuckStruct)]` 生成的 STRUCT 结构体所需接口，以及把它们接入适配层的通用实现。
//!
//! The interface required by `#[derive(DuckStruct)]`-generated STRUCT structs, plus the
//! blanket implementations wiring them into the adapters.

use crate::value_types::vector_layout::struct_field;
use crate::{
    DuckBindArgs, DuckColumns, DuckOptionResult, DuckResult, DuckValueReader, DuckValueType,
    DuckValueWriter, duck_error, vec_option_to_ref,
};
use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{BindInfo, LogicalType, TypeId, Value};

/// (列名, 逻辑类型构造函数)
///
/// A `(column name, logical-type constructor)` pair.
pub type DuckNamedColumnType = (&'static str, fn() -> LogicalType);

/// 「具名结构体 ↔ STRUCT」的底层接口：由 `#[derive(DuckStruct)]` 生成实现。
///
/// 每个字段都被当成一「列」：字段名即列名/参数名，字段类型即 DuckDB 逻辑类型。
/// 本 trait 只提供最基础的、逐字段的构建块（`s_` 前缀即 "struct"），
/// 真正面向用户的三个 trait 由下面的 blanket impl 自动接上：
///
/// - [`DuckValueType`]：作为单列 STRUCT 读写；
/// - [`DuckColumns`]：作为一行多列读写（标量函数参数、表函数输出）；
/// - [`DuckBindArgs`]：作为表函数的 bind 参数解析。
///
/// The low-level interface for "named struct ↔ STRUCT", implemented by
/// `#[derive(DuckStruct)]`. Each field is treated as a "column": the field name is the column
/// (or parameter) name and the field type is the DuckDB logical type. This trait only provides
/// the elementary, per-field building blocks (the `s_` prefix stands for "struct"); the three
/// user-facing traits below are attached automatically by the blanket impls: [`DuckValueType`]
/// (read/write as a single STRUCT column), [`DuckColumns`] (read/write a row of columns, used
/// for scalar arguments and table-function output) and [`DuckBindArgs`] (parse table-function
/// bind parameters).
pub trait DuckStructTrait: DuckValueType {
    // ===== schema =====
    /// duckdb类型声明
    ///
    /// 按字段顺序返回 `(列名, 逻辑类型构造函数)` 列表。
    ///
    /// The DuckDB schema declaration: returns the list of `(field name, logical-type
    /// constructor)` pairs in field order.
    fn s_named_columns_type_fn() -> &'static [DuckNamedColumnType];

    /// 字段数量
    ///
    /// Number of fields.
    fn s_fields_count() -> usize {
        Self::s_named_columns_type_fn().len()
    }

    /// 获表函数取命名参数开始位置
    ///
    /// 返回「从哪个字段开始是命名参数」的字段名；`None` 表示全部按位置参数处理。
    ///
    /// The field name from which table-function named parameters start; `None` means all
    /// fields are positional parameters.
    fn s_named_param_from() -> Option<String>;

    // ===== reader =====

    /// 创建子字段读取器
    ///
    /// 为每个字段创建子读取器（`vectors` 与字段一一对应）。
    ///
    /// Creates one child reader per field (`vectors` corresponds to the fields one-to-one).
    fn s_child_readers(row_count: usize, vectors: Vec<duckdb_vector>) -> Vec<DuckValueReader>;

    /// 从读取器列表中读取一个struct数据
    /// - 结果可以是一行记录
    /// - 可以是单列的一个struct数据
    ///
    /// Reads one STRUCT value from the given readers. The result may be a row of records or a
    /// single STRUCT column value.
    fn s_read_columns(readers: &[DuckValueReader], row: usize) -> Option<Self>;

    // ===== DuckValue =====
    /// 从表函数参数中读取一个struct数据
    ///
    /// 从 [`Value`] 列表（表函数 bind 参数）读取一个结构体。
    ///
    /// Reads a struct from the table-function arguments (a list of [`Value`]s).
    fn s_read_duck_values(values: &[Option<&Value>]) -> crate::DuckResult<Self>;

    // ===== writer =====
    /// 表函数批量输出
    ///
    /// 把一批行写入输出 `DataChunk`（每行一个结构体）。
    ///
    /// Writes a batch of rows (one struct each) into the output `DataChunk`.
    fn s_write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &[Option<&Self>]);

    /// 创建子字段写入器
    ///
    /// 为每个字段创建子写入器。
    ///
    /// Creates one child writer per field.
    fn s_create_writer_batch(
        struct_writer: &DuckValueWriter,
        output_vec: &[Option<&Self>],
    ) -> Vec<DuckValueWriter>;

    /// 写入一个struct数据
    ///
    /// 把一个结构体写成一个 STRUCT 值（含各子字段）。
    ///
    /// Writes one struct as a STRUCT value (including all child fields).
    fn s_write_valid(writer: &mut crate::DuckValueWriter, row: usize, vo: &Self);

    /// 写入完成
    /// - 有子元素递归处理，主要是用来给list、map这样的不定长结构确认长度
    ///
    /// Finishes writing: recurses into child elements, mainly to fix the lengths of
    /// variable-length structures such as list and map.
    fn s_write_finish(writer: &mut crate::DuckValueWriter);

    // ===== generic helpers =====
    /// 从数据块中获取字段向量列表
    ///
    /// Fetches the field vectors from a data chunk.
    fn s_duckdb_vector_list_by_chunk(chunk: &DataChunk) -> Vec<duckdb_vector> {
        (0..Self::s_fields_count())
            .map(|i| unsafe { chunk.vector(i) })
            .collect()
    }

    /// 从结构体向量中获取字段向量列表
    ///
    /// Fetches the field vectors from a STRUCT vector.
    //
    fn s_duckdb_vector_list_by_struct(
        struct_vector: ::libduckdb_sys::duckdb_vector,
    ) -> Vec<duckdb_vector> {
        (0..Self::s_fields_count())
            .map(|i| struct_field(struct_vector, i))
            .collect()
    }

    /// 从DuckValue中读取一个struct可空字段数据
    ///
    /// Reads one nullable field of a struct from a [`Value`].
    fn s_read_by_duck_value_option<F: DuckValueType>(
        option_value: Option<&Value>,
    ) -> DuckOptionResult<F> {
        if let Some(value) = option_value {
            F::read_by_duck_value(value)
        } else {
            Ok(None)
        }
    }
    /// 从DuckValue中读取一个struct非空字段数据，如果为空则报错
    ///
    /// Reads one non-null field of a struct from a [`Value`]; errors out when it is NULL.
    fn s_read_by_duck_value_notnull<F: DuckValueType>(
        option_value: Option<&Value>,
        param_name: &str,
    ) -> DuckResult<F> {
        Self::s_read_by_duck_value_option(option_value)?
            .ok_or_else(|| duck_error(format!("Parameter {} cannot be null", param_name)))
    }

    /// 读取一个字段（[`Value`] 来源，如表函数的 bind 参数）：可空性由字段类型自己决定。
    ///
    /// 字段写 `T` 就是 NOT NULL —— 值为 NULL 或缺省时报错；写 `Option<T>` 就是可空 ——
    /// 此时取到 `None`。判据是 [`DuckValueType::from_null`]，因此自定义的可空类型同样适用。
    ///
    /// Reads one field from a [`Value`] (table-function bind arguments and friends), with
    /// nullability decided by the field type itself: `T` means NOT NULL (a NULL or missing value
    /// is an error) while `Option<T>` accepts NULL as `None`. The criterion is
    /// [`DuckValueType::from_null`], so custom nullable types work too.
    fn s_read_field_by_duck_value<F: DuckValueType>(
        option_value: Option<&Value>,
        param_name: &str,
    ) -> DuckResult<F> {
        let read = match option_value {
            Some(value) => F::read_slot_by_duck_value(value)?,
            None => F::from_null(),
        };
        read.ok_or_else(|| duck_error(format!("Parameter {} cannot be null", param_name)))
    }

    /// 是否命名参数
    ///
    /// 返回与字段一一对应的布尔表：`true` 表示该字段是命名参数。
    ///
    /// Whether each field is a named parameter: returns one boolean per field, `true` meaning a
    /// named parameter.
    fn s_is_named_param_vec() -> Vec<bool> {
        let Some(param) = Self::s_named_param_from() else {
            return vec![false; Self::s_fields_count()];
        };

        let mut started = false;

        Self::s_named_columns_type_fn()
            .iter()
            .map(|(name, _)| {
                started |= *name == param;
                started
            })
            .collect()
    }
    /// 批量写入一个struct数据
    ///
    /// 把一批结构体的某个字段（由 `get_data` 取出）批量写入 `chunk` 的第 `index` 列。
    ///
    /// Writes one field (extracted by `get_data`) of a batch of structs into column `index` of
    /// `chunk`.
    fn s_write_column_batch<F: DuckValueType>(
        chunk: &::quack_rs::prelude::DataChunk,
        row: &[Option<&Self>],
        index: usize,
        get_data: fn(&Self) -> Option<&F>,
    ) {
        F::write_batch(
            unsafe { chunk.vector(index) },
            &row.iter().map(|o| o.and_then(get_data)).collect::<Vec<_>>(),
        );
    }

    /// 创建子字段写入器
    ///
    /// 为 STRUCT 的第 `field_index` 个子字段创建批量写入器（数据由 `get_data` 取出）。
    ///
    /// Creates a batch writer for child field `field_index` of a STRUCT (data extracted by
    /// `get_data`).
    fn s_create_field_writer_batch<F: DuckValueType>(
        struct_writer: &DuckValueWriter,
        field_index: usize,
        output_vec: &[Option<&Self>],
        get_data: fn(&Self) -> Option<&F>,
    ) -> DuckValueWriter {
        F::struct_field_writer_batch(
            struct_writer,
            field_index,
            &output_vec
                .iter()
                .map(|x| x.and_then(get_data))
                .collect::<Vec<_>>(),
        )
    }
    /// 写入一个struct字段数据
    ///
    /// Writes one field of a struct.
    fn s_write_field<F: DuckValueType>(
        writer: &mut crate::DuckValueWriter,
        row: usize,
        field_idx: usize,
        data: Option<&F>,
    ) {
        F::write(&mut writer.child_writer[field_idx], row, data)
    }

    /// 写入一个struct的NULL行
    /// - DuckDB的struct_extract直接重解释子向量、不检查父向量的validity，
    ///   所以父向量置NULL时必须把子字段也一起置NULL
    /// - 只递归struct字段：list / map / array的子向量是按元素下标写入的，
    ///   不能按行下标去置NULL
    ///
    /// Writes a NULL row of a struct. Because DuckDB's `struct_extract` reinterprets child
    /// vectors without checking the parent's validity, marking the parent NULL requires marking
    /// the child fields NULL as well. Only struct fields are recursed into: the child vectors of
    /// list / map / array are addressed by element index and must not be NULL-ed by row index.
    fn s_write_null(writer: &mut crate::DuckValueWriter, row: usize);
}

/// blanket impl：把 [`DuckStructTrait`] 接入 [`DuckValueType`]，使其可作为单列 STRUCT 读写。
///
/// Blanket impl wiring [`DuckStructTrait`] into [`DuckValueType`] so the struct can be read and
/// written as a single STRUCT column.
impl<T: DuckStructTrait> DuckValueType for T {
    fn type_id() -> TypeId {
        TypeId::Struct
    }
    fn logical_type() -> LogicalType {
        let data = Self::s_named_columns_type_fn();
        let vec1: Vec<(&str, LogicalType)> = data.iter().map(|(name, ty)| (*name, ty())).collect();
        LogicalType::struct_type_from_logical(&vec1)
    }
    fn create_reader_from_vector(
        vector: ::libduckdb_sys::duckdb_vector,
        size: usize,
    ) -> crate::DuckValueReader {
        let mut reader = crate::DuckValueReader::new_from_vector(vector, size);
        reader.child_reader =
            Self::s_child_readers(size, Self::s_duckdb_vector_list_by_struct(vector));
        reader
    }
    fn read_valid(reader: &crate::DuckValueReader, row: usize) -> Option<Self> {
        let readers = &reader.child_reader;
        Self::s_read_columns(readers, row)
    }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = crate::DuckValueWriter::new_from_vector(vector);
        writer.child_writer = Self::s_create_writer_batch(&writer, output_vec);
        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        Self::s_write_valid(writer, idx, vo);
    }

    fn write_null(writer: &mut DuckValueWriter, row: usize) {
        Self::s_write_null(writer, row);
    }

    fn write_finish(writer: &mut crate::DuckValueWriter) {
        Self::s_write_finish(writer);
    }
    fn read_by_duck_value_valid(value: &quack_rs::prelude::Value) -> crate::DuckResult<Self> {
        let vec = (0..Self::s_fields_count())
            .map(|i| value.struct_child(i))
            .collect::<Vec<Option<Value>>>();
        Self::s_read_duck_values(&vec_option_to_ref(&vec))
    }
}

/// blanket impl：把 [`DuckStructTrait`] 接入 [`DuckColumns`]，使其可作为一行多列读写。
///
/// Blanket impl wiring [`DuckStructTrait`] into [`DuckColumns`] so the struct can be read and
/// written as a row of columns.
impl<T: DuckStructTrait> DuckColumns for T {
    fn create_column_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        Self::s_child_readers(chunk.size(), Self::s_duckdb_vector_list_by_chunk(chunk))
    }

    fn read_columns(readers: &[DuckValueReader], row: usize) -> Option<Self> {
        Self::s_read_columns(readers, row)
    }

    fn named_column_types() -> Vec<(String, LogicalType)> {
        Self::s_named_columns_type_fn()
            .iter()
            .map(|(name, ty)| (name.to_string(), ty()))
            .collect()
    }
    fn write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &[Option<&Self>]) {
        Self::s_write_columns_batch(chunk, row);
    }
}

/// blanket impl：把 [`DuckStructTrait`] 接入 [`DuckBindArgs`]，使其可解析表函数的 bind 参数。
///
/// Blanket impl wiring [`DuckStructTrait`] into [`DuckBindArgs`] so the struct can parse
/// table-function bind parameters.
impl<T: DuckStructTrait> DuckBindArgs for T {
    fn read_bind_args(bind: &BindInfo) -> DuckResult<Self> {
        let is_named = Self::s_is_named_param_vec();
        let vec1: Vec<Option<Value>> = Self::s_named_columns_type_fn()
            .iter()
            .enumerate()
            .map(|(idx, (name, _logical_type))| {
                Some(if is_named[idx] {
                    unsafe { bind.get_named_parameter_value(name) }
                } else {
                    unsafe { bind.get_parameter_value(idx as u64) }
                })
            })
            .collect::<Vec<Option<Value>>>();
        Self::s_read_duck_values(&vec_option_to_ref(&vec1))
    }

    fn bind_param_logical() -> Vec<(Option<String>, LogicalType)> {
        let is_named = Self::s_is_named_param_vec();

        Self::s_named_columns_type_fn()
            .iter()
            .enumerate()
            .map(|(idx, (name, logical_type))| {
                let name_option = if is_named[idx] {
                    Some(name.to_string())
                } else {
                    None
                };
                (name_option, logical_type())
            })
            .collect()
    }
}
