use quack_rs::prelude::Value;
use crate::{DuckResult, DuckValueReader, DuckValueType, DuckValueWriter};

pub trait FieldNames: Sized + Clone+ Send + Sync + 'static {
    // const FIELD_NAMES: &'static [&'static str] = &["hello_count"];
    const FIELD_NAMES: &'static [&'static str];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckStruct1<F0: DuckValueType, N: FieldNames> {
    pub f0: Option<F0>,
    pub field_names_type: std::marker::PhantomData<N>,
}


impl<F0: DuckValueType, N: FieldNames> DuckValueType for DuckStruct1<F0, N> {
    fn type_id() -> quack_rs::prelude::TypeId {
        quack_rs::prelude::TypeId::Struct
    }
    fn logical_type() -> quack_rs::prelude::LogicalType {
        quack_rs::prelude::LogicalType::struct_type_from_logical(&vec![(N::FIELD_NAMES[0], F0::logical_type())])
    }

    fn create_reader_from_vector(vector: libduckdb_sys::duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let f0_reader = F0::struct_field_reader(&reader, 0);
        reader.child_reader = vec![f0_reader];
        reader
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self {
            f0: F0::read(&reader.child_reader[0], row),
            field_names_type: std::marker::PhantomData,
        })
    }
    // fn create_writer(output: libduckdb_sys::duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //     let f0_writer = F0::struct_field_writer(&writer, 0);
    //     writer.child_writer = vec![f0_writer];
    //     writer
    // }

    fn create_writer_batch(vector: libduckdb_sys::duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);

        let f0_vec: Vec<Option<&F0>> = output_vec
            .iter()
            .map(|x| x.as_ref().and_then(|v| v.f0.as_ref()))
            .collect();
        let f0_writer = F0::struct_field_writer_batch(&writer, 0, &f0_vec);
        writer.child_writer = vec![f0_writer];

        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        F0::write(&mut writer.child_writer[0], idx, &vo.f0);
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Self{
            f0: { if let Some(v) = value.struct_child(0) { F0::read_by_duck_value(&v)? } else { None } },
            field_names_type: std::marker::PhantomData,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckStruct2<F0: DuckValueType, F1: DuckValueType, N: FieldNames> {
    pub f0: Option<F0>,
    pub f1: Option<F1>,
    pub field_names_type: std::marker::PhantomData<N>,
}

impl<F0: DuckValueType, F1: DuckValueType, N: FieldNames> DuckValueType for DuckStruct2<F0, F1, N> {
    fn type_id() -> quack_rs::prelude::TypeId {
        quack_rs::prelude::TypeId::Struct
    }
    fn logical_type() -> quack_rs::prelude::LogicalType {
        quack_rs::prelude::LogicalType::struct_type_from_logical(&vec![
            (N::FIELD_NAMES[0], F0::logical_type()),
            (N::FIELD_NAMES[1], F1::logical_type()),
        ])
    }

    fn create_reader_from_vector(vector: libduckdb_sys::duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        reader.child_reader = vec![
            F0::struct_field_reader(&reader, 0),
            F1::struct_field_reader(&reader, 1),
        ];
        reader
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self {
            f0: F0::read(&reader.child_reader[0], row),
            f1: F1::read(&reader.child_reader[1], row),
            field_names_type: std::marker::PhantomData,
        })
    }
    // fn create_writer(output: libduckdb_sys::duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //     let f0_writer = F0::struct_field_writer(&writer, 0);
    //     let f1_writer = F1::struct_field_writer(&writer, 1);
    //     writer.child_writer = vec![f0_writer, f1_writer];
    //     writer
    // }

    fn create_writer_batch(vector: libduckdb_sys::duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);

        writer.child_writer = vec![
            F0::struct_field_writer_batch(
                &writer,
                0,
                &output_vec
                    .iter()
                    .map(|x| x.as_ref().and_then(|v| v.f0.as_ref()))
                    .collect::<Vec<_>>(),
            ),
            F1::struct_field_writer_batch(
                &writer,
                1,
                &output_vec
                    .iter()
                    .map(|x| x.as_ref().and_then(|v| v.f1.as_ref()))
                    .collect::<Vec<_>>(),
            ),
        ];

        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        F0::write(&mut writer.child_writer[0], idx, &vo.f0);
        F1::write(&mut writer.child_writer[1], idx, &vo.f1);
    }

    fn read_by_duck_value_valid(value: &Value) -> DuckResult<Self> {
        Ok(Self{
            f0: { if let Some(v) = value.struct_child(0) { F0::read_by_duck_value(&v)? } else { None } },
            f1: { if let Some(v) = value.struct_child(1) { F1::read_by_duck_value(&v)? } else { None } },

            field_names_type: std::marker::PhantomData,
        })
    }
}
