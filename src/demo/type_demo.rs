
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckStruct2Tmp<F0: easy_duckdb_extension::DuckValueType, F1: easy_duckdb_extension::DuckValueType, N: easy_duckdb_extension::FieldNames> {
    pub f0: Option<F0>,
    pub f1: F1,
    pub field_names_type: std::marker::PhantomData<N>,
}

impl<F0: easy_duckdb_extension::DuckValueType, F1: easy_duckdb_extension::DuckValueType, N: easy_duckdb_extension::FieldNames> easy_duckdb_extension::DuckValueType for DuckStruct2Tmp<F0, F1, N> {
    fn type_id() -> quack_rs::prelude::TypeId {
        quack_rs::prelude::TypeId::Struct
    }
    fn logical_type() -> quack_rs::prelude::LogicalType {
        quack_rs::prelude::LogicalType::struct_type_from_logical(&vec![
            (N::FIELD_NAMES[0], F0::logical_type()),
            (N::FIELD_NAMES[1], F1::logical_type()),
        ])
    }

    fn create_reader_from_vector(vector: libduckdb_sys::duckdb_vector, size: usize) -> easy_duckdb_extension::DuckValueReader {
        let mut reader = easy_duckdb_extension::DuckValueReader::new_from_vector(vector, size);
        reader.child_reader = vec![
            F0::struct_field_reader(&reader, 0),
            F1::struct_field_reader(&reader, 1),
        ];
        reader
    }

    fn read_valid(reader: &easy_duckdb_extension::DuckValueReader, row: usize) -> Option<Self> {
        Some(Self {
            f0: F0::read(&reader.child_reader[0], row),
            f1: F1::read(&reader.child_reader[1], row)?,
            field_names_type: std::marker::PhantomData,
        })
    }
    // fn create_writer(output: libduckdb_sys::duckdb_vector) -> easy_duckdb_extension::DuckValueWriter {
    //     let mut writer = easy_duckdb_extension::DuckValueWriter::new_from_vector(output);
    //     let f0_writer = F0::struct_field_writer(&writer, 0);
    //     let f1_writer = F1::struct_field_writer(&writer, 1);
    //     writer.child_writer = vec![f0_writer, f1_writer];
    //     writer
    // }

    fn create_writer_batch(vector: libduckdb_sys::duckdb_vector, output_vec: &[Option<&Self>]) -> easy_duckdb_extension::DuckValueWriter {
        let mut writer = easy_duckdb_extension::DuckValueWriter::new_from_vector(vector);

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
                    .map(|x| x.as_ref().and_then(|v| Some(&v.f1)))
                    .collect::<Vec<_>>(),
            ),
        ];

        writer
    }

    fn write_valid(writer: &mut easy_duckdb_extension::DuckValueWriter, idx: usize, vo: &Self) {
        F0::write(&mut writer.child_writer[0], idx, &vo.f0);
        F1::write_valid(&mut writer.child_writer[1], idx, &vo.f1);
    }
}
