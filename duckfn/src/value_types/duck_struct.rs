use crate::{
    DuckBindArgs, DuckColumns, DuckOptionResult, DuckResult, DuckValueReader, DuckValueType,
    DuckValueWriter, duck_error, vec_option_to_ref,
};
use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{BindInfo, LogicalType, StructVector, TypeId, Value};

pub trait DuckStructTrait: DuckValueType {
    fn s_duckdb_vector_list_by_chunk(chunk: &DataChunk) -> Vec<duckdb_vector> {
        (0..Self::s_fields_count())
            .map(|i| unsafe { chunk.vector(i) })
            .collect()
    }
    fn s_duckdb_vector_list_by_struct(
        struct_vector: ::libduckdb_sys::duckdb_vector,
    ) -> Vec<duckdb_vector> {
        (0..Self::s_fields_count())
            .map(|i| unsafe { StructVector::get_child(struct_vector, i) })
            .collect()
    }

    fn s_child_readers(row_count: usize, vectors: Vec<duckdb_vector>) -> Vec<DuckValueReader>;

    fn s_read_columns(readers: &[DuckValueReader], row: usize) -> Option<Self>;

    fn s_fields_count() -> usize {
        Self::s_named_columns_type_fn().len()
    }

    fn s_named_columns_type_fn() ->  &'static [(&'static str, fn() -> LogicalType)];

    fn s_named_param_after() -> Option<String>;

    fn s_read_by_duck_value_option<F: DuckValueType>(
        option_value: Option<&Value>,
    ) -> DuckOptionResult<F> {
        if let Some(value) = option_value {
            F::read_by_duck_value(value)
        } else {
            Ok(None)
        }
    }

    fn s_read_by_duck_value_notnull<F: DuckValueType>(
        option_value: Option<&Value>,
        param_name: &str,
    ) -> DuckResult<F> {
        Self::s_read_by_duck_value_option(option_value)?.ok_or_else(|| duck_error(format!("Parameter {} cannot be null", param_name)))
    }

    // fn struct_read_bind_args(bind: &BindInfo) -> DuckResult<Self>;
    fn s_read_duck_values(values: &Vec<Option<&Value>>) -> crate::DuckResult<Self>;

    fn s_is_named_param_vec() -> Vec<bool> {
        let Some(param) = Self::s_named_param_after() else {
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

    fn s_write_column_batch<F: DuckValueType>(
        chunk: &::quack_rs::prelude::DataChunk,
        row: &Vec<Option<&Self>>,
        index: usize,
        get_data: fn(&Self) -> Option<&F>,
    ) {
        use crate::DuckValueType;
        F::write_batch(
            unsafe { chunk.vector(index) },
            &row.iter().map(|o|o.and_then(get_data) ).collect::<Vec<_>>(),
        );
    }
    fn s_write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>);

    fn s_field_writer_batch<F: DuckValueType>(
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
    fn s_create_writer_batch(
        struct_writer: &DuckValueWriter,
        output_vec: &[Option<&Self>],
    ) -> Vec<DuckValueWriter>;

    fn s_write_field<F: DuckValueType>(
        writer: &mut crate::DuckValueWriter, row: usize,field_idx:usize, data: Option<&F>
    ){
        F::write(&mut writer.child_writer[field_idx], row, data)
    }
    fn s_write_valid(writer: &mut crate::DuckValueWriter, row: usize, vo: &Self);

    fn s_write_finish(writer: &mut crate::DuckValueWriter);
}

impl<T: DuckStructTrait> DuckValueType for T {
    fn type_id() -> TypeId {
        TypeId::Struct
    }
    fn logical_type() -> LogicalType {
        let data = Self::s_named_columns_type_fn();
        let vec1: Vec<(&str, LogicalType)> = data
            .iter()
            .map(|(name, ty)| (*name, ty()))
            .collect();
        LogicalType::struct_type_from_logical(&vec1)
    }
    fn create_reader_from_vector(
        vector: ::libduckdb_sys::duckdb_vector,
        size: usize,
    ) -> crate::DuckValueReader {
        let mut reader = crate::DuckValueReader::new_from_vector(vector, size);
        reader.child_reader = Self::s_child_readers(size, Self::s_duckdb_vector_list_by_struct(vector));
        reader
    }
    fn read_valid(reader: &crate::DuckValueReader, row: usize) -> Option<Self> {
        use crate::DuckValueType;
        let readers = &reader.child_reader;
        Self::s_read_columns(readers, row)
    }
    fn read_by_duck_value_valid(value: &quack_rs::prelude::Value) -> crate::DuckResult<Self> {
        use crate::DuckValueType;
        let vec = (0..Self::s_fields_count())
            .into_iter()
            .map(|i| value.struct_child(i))
            .collect::<Vec<Option<Value>>>();
        Self::s_read_duck_values(&vec_option_to_ref(&vec))
    }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        use crate::DuckValueType;
        let mut writer = crate::DuckValueWriter::new_from_vector(vector);
        writer.child_writer = Self::s_create_writer_batch(&writer, output_vec);
        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        Self::s_write_valid(writer, idx, vo);
    }
    fn write_finish(writer: &mut crate::DuckValueWriter) {
        Self::s_write_finish(writer);
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
        let vec = Self::s_is_named_param_vec();
        let vec1: Vec<Option<Value>> = Self::s_named_columns_type_fn()
            .into_iter()
            .enumerate()
            .map(|(idx, (name, logical_type))| {
                Some(if vec[idx] {
                    (unsafe { bind.get_parameter_value(idx as u64) })
                } else {
                    (unsafe { bind.get_named_parameter_value(&*name) })
                })
            })
            .collect::<Vec<Option<Value>>>();
        Self::s_read_duck_values(&vec_option_to_ref(&vec1))
    }

    fn bind_param_logical() -> Vec<(Option<String>, LogicalType)> {
        let vec = Self::s_is_named_param_vec();

        Self::s_named_columns_type_fn()
            .into_iter()
            .enumerate()
            .map(|(idx, (name, logical_type))| {
                (if vec[idx] { Some(name.to_string()) } else { None }, logical_type())
            })
            .collect()
    }
}
