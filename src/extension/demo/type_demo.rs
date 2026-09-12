// 示例代码：手动实现 DuckStruct 各方法的演示，未全部被调用。
#![allow(dead_code)]

use quack_rs::connection::Connection;
use duckfn::DuckResult;

fn word_count_w_reg(c: &Connection) -> DuckResult<()> {
    use quack_rs::prelude::Registrar;
    let sql_macro = quack_rs::prelude::SqlMacro::scalar("clamp", &["x", "lo", "hi"], "greatest(lo, least(hi, x))")?;
    unsafe { c.register_sql_macro(sql_macro) }
}

#[derive(Clone, Default, Debug /*DuckStruct*/)]
// #[duck(named_param_from = "data")]
pub struct DuckStructDemo1 {
    pub count: i64,
    pub data: Vec<i64>,
    pub age: Option<i32>,
    pub nest_data: Option<Vec<Vec<i64>>>,
}
impl ::duckfn::DuckStructTrait for DuckStructDemo1 {
    fn s_named_columns_type_fn() -> &'static [(&'static str, fn() -> quack_rs::prelude::LogicalType)]
    {
        use duckfn::DuckValueType;
        ::duckfn::assert_impl_duck_value_type::<i64>();
        ::duckfn::assert_impl_duck_value_type::<Vec<i64>>();
        ::duckfn::assert_impl_duck_value_type::<i32>();
        ::duckfn::assert_impl_duck_value_type::<Vec<Vec<i64>>>();
        &[
            ("count", i64::logical_type),
            ("data", Vec::<i64>::logical_type),
            ("age", i32::logical_type),
            ("nest_data", Vec::<Vec<i64>>::logical_type),
        ]
    }

    fn s_named_param_from() -> Option<String> {
        Some("data".to_string())
    }

    fn s_child_readers(
        row_count: usize,
        vectors: Vec<libduckdb_sys::duckdb_vector>,
    ) -> Vec<duckfn::DuckValueReader> {
        use duckfn::DuckValueType;
        Vec::from([
            i64::create_reader_from_vector(vectors[0], row_count),
            Vec::<i64>::create_reader_from_vector(vectors[1], row_count),
            i32::create_reader_from_vector(vectors[2], row_count),
            Vec::<Vec<i64>>::create_reader_from_vector(vectors[3], row_count),
        ])
    }

    fn s_read_columns(readers: &[duckfn::DuckValueReader], row: usize) -> Option<Self> {
        use duckfn::DuckValueType;
        Some(Self {
            count: i64::read(&readers[0usize], row)?,
            data: Vec::<i64>::read(&readers[1usize], row)?,
            age: i32::read(&readers[2usize], row),
            nest_data: Vec::<Vec<i64>>::read(&readers[3usize], row),
        })
    }

    fn s_read_duck_values(
        values: &Vec<Option<&quack_rs::value::Value>>,
    ) -> duckfn::DuckResult<Self> {
        Ok(Self {
            count: Self::s_read_by_duck_value_notnull(values[0], "count")?,
            data: Self::s_read_by_duck_value_notnull(values[1], "data")?,
            age: Self::s_read_by_duck_value_option(values[2])?,
            nest_data: Self::s_read_by_duck_value_option(values[3])?,
        })
    }

    fn s_write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>) {
        Self::s_write_column_batch(chunk, row, 0, |v| Some(&v.count));
        Self::s_write_column_batch(chunk, row, 1, |v| Some(&v.data));
        Self::s_write_column_batch(chunk, row, 2, |v| v.age.as_ref());
        Self::s_write_column_batch(chunk, row, 3, |v| v.nest_data.as_ref());
    }

    fn s_create_writer_batch(
        struct_writer: &duckfn::DuckValueWriter,
        output_vec: &[Option<&Self>],
    ) -> Vec<duckfn::DuckValueWriter> {
        Vec::from([
            Self::s_create_field_writer_batch(struct_writer, 0, output_vec, |v| Some(&v.count)),
            Self::s_create_field_writer_batch(struct_writer, 1, output_vec, |v| Some(&v.data)),
            Self::s_create_field_writer_batch(struct_writer, 2, output_vec, |v| v.age.as_ref()),
            Self::s_create_field_writer_batch(struct_writer, 3, output_vec, |v| v.nest_data.as_ref()),
        ])
    }

    fn s_write_valid(writer: &mut duckfn::DuckValueWriter, row: usize, v: &Self) {
        Self::s_write_field(writer, row, 0, Some(&v.count));
        Self::s_write_field(writer, row, 1, Some(&v.data));
        Self::s_write_field(writer, row, 2, v.age.as_ref());
        Self::s_write_field(writer, row, 3, v.nest_data.as_ref());
    }

    fn s_write_null(writer: &mut duckfn::DuckValueWriter, row: usize) {
        use duckfn::DuckValueType;
        unsafe { writer.vector_writer.set_null(row) };
        i64::write_null(&mut writer.child_writer[0usize], row);
        Vec::<i64>::write_null(&mut writer.child_writer[1usize], row);
        i32::write_null(&mut writer.child_writer[2usize], row);
        Vec::<Vec<i64>>::write_null(&mut writer.child_writer[3usize], row);
    }

    fn s_write_finish(writer: &mut ::duckfn::DuckValueWriter) {
        use duckfn::DuckValueType;
        i64::write_finish(&mut writer.child_writer[0usize]);
        Vec::<i64>::write_finish(&mut writer.child_writer[1usize]);
        i32::write_finish(&mut writer.child_writer[2usize]);
        Vec::<Vec<i64>>::write_finish(&mut writer.child_writer[3usize]);
    }
}
impl DuckStructDemo1 {
    fn type_id() -> ::quack_rs::prelude::TypeId {
        ::quack_rs::prelude::TypeId::Struct
    }
    fn logical_type() -> ::quack_rs::prelude::LogicalType {
        use duckfn::DuckValueType;
        ::duckfn::assert_impl_duck_value_type::<i64>();
        ::duckfn::assert_impl_duck_value_type::<Vec<i64>>();
        ::duckfn::assert_impl_duck_value_type::<i32>();
        ::duckfn::assert_impl_duck_value_type::<Vec<Vec<i64>>>();
        ::quack_rs::prelude::LogicalType::struct_type_from_logical(&Vec::from([
            ("count", i64::logical_type()),
            ("data", Vec::<i64>::logical_type()),
            ("age", i32::logical_type()),
            ("nest_data", Vec::<Vec<i64>>::logical_type()),
        ]))
    }
    fn create_reader_from_vector(
        vector: ::libduckdb_sys::duckdb_vector,
        size: usize,
    ) -> ::duckfn::DuckValueReader {
        use duckfn::DuckValueType;
        let mut reader = ::duckfn::DuckValueReader::new_from_vector(vector, size);
        reader.child_reader = Vec::from([
            i64::struct_field_reader(&reader, 0usize),
            Vec::<i64>::struct_field_reader(&reader, 1usize),
            i32::struct_field_reader(&reader, 2usize),
            Vec::<Vec<i64>>::struct_field_reader(&reader, 3usize),
        ]);
        reader
    }
    fn read_valid(reader: &::duckfn::DuckValueReader, row: usize) -> Option<Self> {
        use duckfn::DuckValueType;
        let readers = &reader.child_reader;
        Some(Self {
            count: i64::read(&readers[0usize], row)?,
            data: Vec::<i64>::read(&readers[1usize], row)?,
            age: i32::read(&readers[2usize], row),
            nest_data: Vec::<Vec<i64>>::read(&readers[3usize], row),
        })
    }
    fn create_writer_batch(
        vector: ::libduckdb_sys::duckdb_vector,
        output_vec: &[Option<&Self>],
    ) -> ::duckfn::DuckValueWriter {
        use duckfn::DuckValueType;
        let mut writer = ::duckfn::DuckValueWriter::new_from_vector(vector);
        writer.child_writer = Vec::from([
            i64::struct_field_writer_batch(
                &writer,
                0usize,
                &output_vec
                    .iter()
                    .map(|x| x.and_then(|v| Some(&v.count)))
                    .collect::<Vec<_>>(),
            ),
            Vec::<i64>::struct_field_writer_batch(
                &writer,
                1usize,
                &output_vec
                    .iter()
                    .map(|x| x.and_then(|v| Some(&v.data)))
                    .collect::<Vec<_>>(),
            ),
            i32::struct_field_writer_batch(
                &writer,
                2usize,
                &output_vec
                    .iter()
                    .map(|x| x.and_then(|v| v.age.as_ref()))
                    .collect::<Vec<_>>(),
            ),
            Vec::<Vec<i64>>::struct_field_writer_batch(
                &writer,
                3usize,
                &output_vec
                    .iter()
                    .map(|x| x.and_then(|v| v.nest_data.as_ref()))
                    .collect::<Vec<_>>(),
            ),
        ]);
        writer
    }
    fn write_valid(writer: &mut ::duckfn::DuckValueWriter, idx: usize, vo: &Self) {
        use duckfn::DuckValueType;
        i64::write_valid(&mut writer.child_writer[0usize], idx, &vo.count);
        Vec::<i64>::write_valid(&mut writer.child_writer[1usize], idx, &vo.data);
        i32::write(&mut writer.child_writer[2usize], idx, vo.age.as_ref());
        Vec::<Vec<i64>>::write(&mut writer.child_writer[3usize], idx, vo.nest_data.as_ref());
    }
    fn write_finish(writer: &mut ::duckfn::DuckValueWriter) {
        use duckfn::DuckValueType;
        i64::write_finish(&mut writer.child_writer[0usize]);
        Vec::<i64>::write_finish(&mut writer.child_writer[1usize]);
        i32::write_finish(&mut writer.child_writer[2usize]);
        Vec::<Vec<i64>>::write_finish(&mut writer.child_writer[3usize]);
    }
    fn read_by_duck_value_valid(value: &quack_rs::prelude::Value) -> ::duckfn::DuckResult<Self> {
        use duckfn::DuckValueType;
        Ok(Self {
            count: {
                if let Some(v) = value.struct_child(0usize) {
                    i64::read_by_duck_value(&v)?
                } else {
                    None
                }
            }
            .ok_or_else(|| duckfn::duck_error("count cannot be null"))?,
            data: {
                if let Some(v) = value.struct_child(1usize) {
                    Vec::<i64>::read_by_duck_value(&v)?
                } else {
                    None
                }
            }
            .ok_or_else(|| duckfn::duck_error("data cannot be null"))?,
            age: {
                if let Some(v) = value.struct_child(2usize) {
                    i32::read_by_duck_value(&v)?
                } else {
                    None
                }
            },
            nest_data: {
                if let Some(v) = value.struct_child(3usize) {
                    Vec::<Vec<i64>>::read_by_duck_value(&v)?
                } else {
                    None
                }
            },
        })
    }
}
impl DuckStructDemo1 {
    fn create_column_readers(
        chunk: &quack_rs::data_chunk::DataChunk,
    ) -> Vec<::duckfn::DuckValueReader> {
        use duckfn::DuckValueType;
        Vec::from([
            i64::create_reader(chunk, 0usize),
            Vec::<i64>::create_reader(chunk, 1usize),
            i32::create_reader(chunk, 2usize),
            Vec::<Vec<i64>>::create_reader(chunk, 3usize),
        ])
    }
    fn read_columns(readers: &[::duckfn::DuckValueReader], row: usize) -> Option<Self> {
        use duckfn::DuckValueType;
        Some(Self {
            count: i64::read(&readers[0usize], row)?,
            data: Vec::<i64>::read(&readers[1usize], row)?,
            age: i32::read(&readers[2usize], row),
            nest_data: Vec::<Vec<i64>>::read(&readers[3usize], row),
        })
    }
    fn named_column_types() -> Vec<(String, ::quack_rs::prelude::LogicalType)> {
        use duckfn::DuckValueType;
        Vec::from([
            ("count".to_string(), i64::logical_type()),
            ("data".to_string(), Vec::<i64>::logical_type()),
            ("age".to_string(), i32::logical_type()),
            ("nest_data".to_string(), Vec::<Vec<i64>>::logical_type()),
        ])
    }
    fn write_columns_batch(chunk: &::quack_rs::prelude::DataChunk, row: &Vec<Option<&Self>>) {
        use duckfn::DuckValueType;
        i64::write_batch(
            unsafe { chunk.vector(0usize) },
            &row.iter().map(|o| o.map(|r| &r.count)).collect::<Vec<_>>(),
        );
        Vec::<i64>::write_batch(
            unsafe { chunk.vector(1usize) },
            &row.iter().map(|o| o.map(|r| &r.data)).collect::<Vec<_>>(),
        );
        i32::write_batch(
            unsafe { chunk.vector(2usize) },
            &row.iter()
                .map(|o| o.and_then(|r| r.age.as_ref()))
                .collect::<Vec<_>>(),
        );
        Vec::<Vec<i64>>::write_batch(
            unsafe { chunk.vector(3usize) },
            &row.iter()
                .map(|o| o.and_then(|r| r.nest_data.as_ref()))
                .collect::<Vec<_>>(),
        );
    }
}
impl DuckStructDemo1 {
    fn read_bind_args(bind: &quack_rs::prelude::BindInfo) -> duckfn::DuckResult<Self> {
        use duckfn::DuckValueType;
        Ok(DuckStructDemo1 {
            count: i64::read_by_duck_value(&(unsafe { bind.get_parameter_value(0usize as u64) }))?
                .ok_or_else(|| duckfn::duck_error("count cannot be null"))?,
            data: Vec::<i64>::read_by_duck_value(
                &(unsafe { bind.get_named_parameter_value("data") }),
            )?
            .ok_or_else(|| duckfn::duck_error("data cannot be null"))?,
            age: i32::read_by_duck_value(&(unsafe { bind.get_named_parameter_value("age") }))?,
            nest_data: Vec::<Vec<i64>>::read_by_duck_value(
                &(unsafe { bind.get_named_parameter_value("nest_data") }),
            )?,
        })
    }
    fn bind_param_logical() -> Vec<(Option<String>, quack_rs::prelude::LogicalType)> {
        use duckfn::DuckValueType;
        Vec::from([
            (None, i64::logical_type()),
            (Some("data".to_string()), Vec::<i64>::logical_type()),
            (Some("age".to_string()), i32::logical_type()),
            (
                Some("nest_data".to_string()),
                Vec::<Vec<i64>>::logical_type(),
            ),
        ])
    }
}
