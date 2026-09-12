use crate::{
    DuckBindArgs, DuckColumns, DuckOptionResult, DuckResult, DuckValueReader, DuckValueType,
    DuckValueWriter, duck_error, vec_option_to_ref,
};
use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{BindInfo, LogicalType, StructVector, TypeId, Value};

pub trait DuckStructTrait: DuckValueType {
    // ===== schema =====
    /// duckdb类型声明
    fn s_named_columns_type_fn() -> &'static [(&'static str, fn() -> LogicalType)];

    /// 字段数量
    fn s_fields_count() -> usize {
        Self::s_named_columns_type_fn().len()
    }

    /// 获表函数取命名参数开始位置
    fn s_named_param_from() -> Option<String>;

    // ===== reader =====

    /// 创建子字段读取器
    fn s_child_readers(row_count: usize, vectors: Vec<duckdb_vector>) -> Vec<DuckValueReader>;

    /// 从读取器列表中读取一个struct数据
    /// - 结果可以是一行记录
    /// - 可以是单列的一个struct数据
    fn s_read_columns(readers: &[DuckValueReader], row: usize) -> Option<Self>;

    // ===== DuckValue =====
    /// 从表函数参数中读取一个struct数据
    fn s_read_duck_values(values: &Vec<Option<&Value>>) -> crate::DuckResult<Self>;

    // ===== writer =====
    /// 表函数批量输出
    fn s_write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>);

    /// 创建子字段写入器
    fn s_create_writer_batch(
        struct_writer: &DuckValueWriter,
        output_vec: &[Option<&Self>],
    ) -> Vec<DuckValueWriter>;

    /// 写入一个struct数据
    fn s_write_valid(writer: &mut crate::DuckValueWriter, row: usize, vo: &Self);

    /// 写入完成
    /// - 有子元素递归处理，主要是用来给list、map这样的不定长结构确认长度
    fn s_write_finish(writer: &mut crate::DuckValueWriter);

    // ===== generic helpers =====
    /// 从数据块中获取字段向量列表
    fn s_duckdb_vector_list_by_chunk(chunk: &DataChunk) -> Vec<duckdb_vector> {
        (0..Self::s_fields_count())
            .map(|i| unsafe { chunk.vector(i) })
            .collect()
    }

    /// 从结构体向量中获取字段向量列表
    fn s_duckdb_vector_list_by_struct(
        struct_vector: ::libduckdb_sys::duckdb_vector,
    ) -> Vec<duckdb_vector> {
        (0..Self::s_fields_count())
            .map(|i| unsafe { StructVector::get_child(struct_vector, i) })
            .collect()
    }

    /// 从DuckValue中读取一个struct可空字段数据
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
    fn s_read_by_duck_value_notnull<F: DuckValueType>(
        option_value: Option<&Value>,
        param_name: &str,
    ) -> DuckResult<F> {
        Self::s_read_by_duck_value_option(option_value)?
            .ok_or_else(|| duck_error(format!("Parameter {} cannot be null", param_name)))
    }

    /// 是否命名参数
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
    fn s_write_column_batch<F: DuckValueType>(
        chunk: &::quack_rs::prelude::DataChunk,
        row: &Vec<Option<&Self>>,
        index: usize,
        get_data: fn(&Self) -> Option<&F>,
    ) {
        F::write_batch(
            unsafe { chunk.vector(index) },
            &row.iter().map(|o| o.and_then(get_data)).collect::<Vec<_>>(),
        );
    }

    /// 创建子字段写入器
    fn s_create_field_writer_batch<F: DuckValueType>(
        struct_writer: &DuckValueWriter,
        field_index: usize,
        output_vec: &[Option<&Self>],
        get_data: fn(&Self) -> Option<&F>,
    ) -> DuckValueWriter {
        F::struct_field_writer_batch(
            &struct_writer,
            field_index,
            &output_vec
                .iter()
                .map(|x| x.and_then(get_data))
                .collect::<Vec<_>>(),
        )
    }
    /// 写入一个struct字段数据
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
    fn s_write_null(writer: &mut crate::DuckValueWriter, row: usize);
}

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
            .into_iter()
            .map(|i| value.struct_child(i))
            .collect::<Vec<Option<Value>>>();
        Self::s_read_duck_values(&vec_option_to_ref(&vec))
    }
}
impl<T: DuckStructTrait> DuckColumns for T {
    fn create_column_readers(chunk: &DataChunk) -> Vec<DuckValueReader> {
        Self::s_child_readers(chunk.size(), Self::s_duckdb_vector_list_by_chunk(chunk))
    }

    fn read_columns(readers: &[DuckValueReader], row: usize) -> Option<Self> {
        Self::s_read_columns(readers, row)
    }

    fn named_column_types() -> Vec<(String, LogicalType)> {
        Self::s_named_columns_type_fn()
            .into_iter()
            .map(|(name, ty)| (name.to_string(), ty()))
            .collect()
    }
    fn write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>) {
        Self::s_write_columns_batch(chunk, row);
    }
}
impl<T: DuckStructTrait> DuckBindArgs for T {
    fn read_bind_args(bind: &BindInfo) -> DuckResult<Self> {
        let is_named = Self::s_is_named_param_vec();
        let vec1: Vec<Option<Value>> = Self::s_named_columns_type_fn()
            .into_iter()
            .enumerate()
            .map(|(idx, (name, _logical_type))| {
                Some(if is_named[idx] {
                    unsafe { bind.get_named_parameter_value(&*name) }
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
            .into_iter()
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
