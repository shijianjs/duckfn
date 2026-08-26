use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{DuckInterval, ListBuilder, ListVector, LogicalType, StructVector, TypeId, VectorReader, VectorWriter};
use std::marker::PhantomData;

pub struct DuckTypeInfo {
    pub type_id: TypeId,
    pub logical_type: LogicalType,
}

pub struct DuckValueReader {
    /// rust api, 和下面的c_duckdb_vector一比一对应
    pub vector_reader: VectorReader,
    /// c api, 和上面的reader一比一对应
    pub c_duckdb_vector: duckdb_vector,
    /// 基于上面的c_duckdb_vector的子reader
    pub child_reader: Vec<DuckValueReader>,
}

impl DuckValueReader {
    fn new_from_chunk(chunk: &DataChunk, column_index: usize) -> Self {
        let vector = unsafe { chunk.vector(column_index) };
        let size = chunk.size();
        Self::new_from_vector(vector, size)
    }
    fn new_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckValueReader {
            vector_reader: unsafe { VectorReader::from_vector(vector, size) },
            c_duckdb_vector: vector,
            child_reader: vec![],
        }
    }
}

pub struct DuckValueWriter {
    pub vector_writer: VectorWriter,
    pub c_duckdb_vector: duckdb_vector,
    pub child_writer: Vec<DuckValueWriter>,
    pub offset: usize,
    // pub list_builder:Option<ListBuilder>,
}
impl DuckValueWriter {
    fn new_from_vector(vector: duckdb_vector) -> Self {
        Self {
            vector_writer: unsafe { VectorWriter::new(vector) },
            c_duckdb_vector: vector,
            child_writer: vec![],
            offset: 0,
            // list_builder: None,
        }
    }
}

/// 映射规则：
/// - 如果 Rust 基础类型已经完整表达了业务语义，可以直接映射；
/// - 如果多个逻辑类型共享同一个物理表示，就应该 newtype 包装。
pub trait DuckValueType: Sized+Clone {
    fn type_info() -> DuckTypeInfo {
        DuckTypeInfo {
            type_id: Self::type_id(),
            logical_type: Self::logical_type(),
        }
    }
    fn type_id() -> TypeId;
    fn logical_type() -> LogicalType {
        LogicalType::new(Self::type_id())
    }

    // fn create_reader(chunk: &DataChunk, column_index: usize) -> DuckValueReader {
    //     DuckValueReader::new_from_chunk(chunk, column_index)
    // }
    fn create_reader(chunk: &DataChunk, column_index: usize) -> DuckValueReader {
        let vector = unsafe { chunk.vector(column_index) };
        let size = chunk.size();
        Self::create_reader_from_vector(vector, size)
    }
    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckValueReader::new_from_vector(vector, size)
    }

    fn read(reader: &DuckValueReader, row: usize) -> Option<Self> {
        if unsafe { reader.vector_reader.is_valid(row) } {
            Some(Self::read_valid(reader, row))
        } else {
            None
        }
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        Self::read_valid_by_vector_reader(&reader.vector_reader, row)
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        todo!("子类需要实现read_valid_by_vector_reader")
    }

    fn write_batch(output: duckdb_vector,output_vec: &[Option<Self>]) {
        let refs: Vec<Option<&Self>> =
            output_vec.iter().map(|v| v.as_ref()).collect();
        let mut writer = Self::create_writer_batch(output, &refs);
        for (idx, result) in output_vec.iter().enumerate() {
            Self::write(&mut writer, idx, result);
        }
        Self::write_finish(&mut writer);
    }

    // fn create_writer(vector: duckdb_vector) -> DuckValueWriter {
    //     DuckValueWriter::new_from_vector(vector)
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        // Self::create_writer(vector)
        DuckValueWriter::new_from_vector(vector)
    }


    fn write(writer: &mut DuckValueWriter, idx: usize, vo: &Option<Self>) {
        // Self::write_to_vector(&mut writer.vector_writer, idx, vo);
        match vo {
            None => unsafe { writer.vector_writer.set_null(idx) },

            Some(v) => Self::write_valid(writer, idx, v),
        }
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        Self::write_valid_to_vector_writer(&mut writer.vector_writer, idx, vo)
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        todo!("子类需要实现write_valid")
    }

    fn write_finish(writer: &mut DuckValueWriter) {}


    fn struct_field_reader(reader: &DuckValueReader, field_index: usize) -> DuckValueReader {
        let row_count = reader.vector_reader.row_count();
        let vector = reader.c_duckdb_vector;
        let field_vector = unsafe { StructVector::get_child(vector, field_index) };
        let field_reader = Self::create_reader_from_vector(field_vector, row_count);
        field_reader
    }
    // fn struct_field_writer(writer: &DuckValueWriter, field_index: usize) -> DuckValueWriter {
    //     let vector = writer.c_duckdb_vector;
    //     let field_vector = unsafe { StructVector::get_child(vector, field_index) };
    //     let field_writer = Self::create_writer(field_vector);
    //     field_writer
    // }
    fn struct_field_writer_batch(writer: &DuckValueWriter, field_index: usize,output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let vector = writer.c_duckdb_vector;
        let field_vector = unsafe { StructVector::get_child(vector, field_index) };
        let field_writer = Self::create_writer_batch(field_vector, output_vec);
        field_writer
    }
}
pub fn assert_impl_duck_value_type<T: DuckValueType>() {}


pub trait FieldNames: Sized+Clone {
    // const FIELD_NAMES: &'static [&'static str] = &["hello_count"];
    const FIELD_NAMES: &'static [&'static str];
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckStruct1<F0: DuckValueType, N: FieldNames> {
    pub f0: Option<F0>,
    pub field_names_type: PhantomData<N>,
}

impl<F0: DuckValueType, N: FieldNames> DuckValueType for DuckStruct1<F0, N> {
    fn type_id() -> TypeId {
        TypeId::Struct
    }
    fn logical_type() -> LogicalType {
        LogicalType::struct_type_from_logical(&vec![(N::FIELD_NAMES[0], F0::logical_type())])
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let f0_reader = F0::struct_field_reader(&reader, 0);
        reader.child_reader = vec![f0_reader];
        reader
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        Self{
            f0: F0::read(&reader.child_reader[0], row),
            field_names_type: PhantomData,
        }
    }
    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //     let f0_writer = F0::struct_field_writer(&writer, 0);
    //     writer.child_writer = vec![f0_writer];
    //     writer
    // }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);

        let f0_vec: Vec<Option<&F0>> = output_vec.iter().map(|x| x.as_ref().and_then(|v| v.f0.as_ref())).collect();
        let f0_writer = F0::struct_field_writer_batch(&writer, 0, &f0_vec);
        writer.child_writer = vec![f0_writer];

        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        F0::write(&mut writer.child_writer[0], idx, &vo.f0);
    }

}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckStruct2<F0: DuckValueType, F1: DuckValueType, N: FieldNames> {
    pub f0: Option<F0>,
    pub f1: Option<F1>,
    pub field_names_type: PhantomData<N>,
}
impl<F0: DuckValueType, F1: DuckValueType, N: FieldNames> DuckValueType for DuckStruct2<F0, F1, N> {
    fn type_id() -> TypeId {
        TypeId::Struct
    }
    fn logical_type() -> LogicalType {
        LogicalType::struct_type_from_logical(&vec![
            (N::FIELD_NAMES[0], F0::logical_type()),
            (N::FIELD_NAMES[1], F1::logical_type()),
        ])
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let f0_reader = F0::struct_field_reader(&reader, 0);
        let f1_reader = F1::struct_field_reader(&reader, 1);
        reader.child_reader = vec![f0_reader, f1_reader];
        reader
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        Self{
            f0: F0::read(&reader.child_reader[0], row),
            f1: F1::read(&reader.child_reader[1], row),
            field_names_type: PhantomData,
        }
    }
    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //     let f0_writer = F0::struct_field_writer(&writer, 0);
    //     let f1_writer = F1::struct_field_writer(&writer, 1);
    //     writer.child_writer = vec![f0_writer, f1_writer];
    //     writer
    // }

    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);

        let f0_vec: Vec<Option<&F0>> = output_vec.iter()
            .map(|x| x.as_ref().and_then(|v| v.f0.as_ref()))
            .collect();
        let f0_writer = F0::struct_field_writer_batch(&writer, 0, &f0_vec);

        let f1_vec: Vec<Option<&F1>> = output_vec.iter()
            .map(|x| x.as_ref().and_then(|v| v.f1.as_ref()))
            .collect();
        let f1_writer = F1::struct_field_writer_batch(&writer, 1, &f1_vec);
        writer.child_writer = vec![f0_writer, f1_writer];

        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        F0::write(&mut writer.child_writer[0], idx, &vo.f0);
        F1::write(&mut writer.child_writer[1], idx, &vo.f1);
    }
}


// TypeId::List
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuckList<T: DuckValueType> {
    pub value: Vec<Option<T>>,
}
impl<T: DuckValueType> DuckList<T> {}

impl<T: DuckValueType> DuckValueType for DuckList<T> {
    fn type_id() -> TypeId {
        TypeId::List
    }

    fn logical_type() -> LogicalType {
        LogicalType::list_from_logical(&T::logical_type())
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let child_vector = unsafe { ListVector::get_child(vector) };
        let child_size = unsafe { ListVector::get_size(vector) };
        let child_reader = T::create_reader_from_vector(child_vector, child_size);

        reader.child_reader = vec![child_reader];
        reader
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        let list_vec = reader.c_duckdb_vector;
        let entry = unsafe { ListVector::get_entry(list_vec, row) };

        // 之前是照着官方的写法写在这里的
        let child_reader = &reader.child_reader[0];
        let mut vec: Vec<Option<T>> = Vec::with_capacity(entry.length as usize);
        for i in 0..entry.length as usize {
            let idx = entry.offset as usize + i;
            vec.push(T::read(&child_reader, idx));
        }
        DuckList { value: vec }
    }

    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //
    //     let child_vector = unsafe { ListVector::get_child(output) };
    //
    //     let child_writer = T::create_writer(child_vector);
    //
    //     writer.child_writer.push(child_writer);
    //
    //     writer
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec.iter()
            .filter_map(|x| x.as_ref().map(|v| v.value.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| {
                list.value.iter().map(|x| x.as_ref())
            })
            .collect();

        let child_writer = T::create_writer_batch(child_vector,&vec);

        writer.child_writer.push(child_writer);

        writer
    }
    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     let mut writer = DuckValueWriter::new_from_vector(output);
    //     writer.list_builder = Some(unsafe{ ListBuilder::new(output) });
    //     writer
    // }

    // 尝试改为官方推荐的ListBuilder，失败，不支持递归嵌套
    // fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
    //     if let Some(builder) = &mut writer.list_builder {
    //         unsafe {
    //             builder.push_row(idx, vo.value.len(), move|writer, base| {
    //                 let mut child_writer = T::create_writer(writer.as_raw());
    //                 for (i, val) in vo.value.iter().enumerate() {
    //                     T::write(&mut child_writer, base + i, val);
    //                 }
    //                 T::write_finish(&mut child_writer);
    //             });
    //         }
    //     }
    // }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.value.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.value.iter().enumerate() {
            T::write(child_writer, offset + i, &value);
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(&mut writer.child_writer[0]);

        unsafe {
            ListVector::set_size(writer.c_duckdb_vector, writer.offset);
        }
    }
    // fn write_finish(writer: &mut DuckValueWriter) {
    //     unsafe {
    //         if let Some(builder) = writer.list_builder.take() {
    //            unsafe  { builder.finish(); }
    //         }
    //     }
    // }
}
impl<T: DuckValueType> DuckValueType for Vec<Option<T>> {
    fn type_id() -> TypeId {
        DuckList::<T>::type_id()
    }

    fn logical_type() -> LogicalType {
        DuckList::<T>::logical_type()
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckList::<T>::create_reader_from_vector(vector, size)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        DuckList::<T>::read_valid(reader, row).value
    }

    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     DuckList::<T>::create_writer(output)
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec.iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| {
                list.iter().map(|x| x.as_ref())
            })
            .collect();

        let child_writer = T::create_writer_batch(child_vector,&vec);

        writer.child_writer.push(child_writer);

        writer
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.iter().enumerate() {
            T::write(child_writer, offset + i, &value);
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        DuckList::<T>::write_finish(writer)
    }
}
impl<T: DuckValueType> DuckValueType for Vec<T> {
    fn type_id() -> TypeId {
        DuckList::<T>::type_id()
    }

    fn logical_type() -> LogicalType {
        DuckList::<T>::logical_type()
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        DuckList::<T>::create_reader_from_vector(vector, size)
    }
    fn read(reader: &DuckValueReader, row: usize) -> Option<Self> {
        // Option<Vec<Option<T>>> -> Option<Vec<T>>
        DuckList::<T>::read(reader, row)
            .map(|li| {li.value})
            .and_then(|v| v.into_iter().collect::<Option<Vec<_>>>())
    }

    // fn create_writer(output: duckdb_vector) -> DuckValueWriter {
    //     DuckList::<T>::create_writer(output)
    // }
    fn create_writer_batch(vector: duckdb_vector, output_vec: &[Option<&Self>]) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(vector);
        let total_elements: usize = output_vec.iter()
            .filter_map(|x| x.as_ref().map(|v| v.len()))
            .sum();
        unsafe { ListVector::reserve(vector, total_elements) };
        let child_vector = unsafe { ListVector::get_child(vector) };

        let vec: Vec<Option<&T>> = output_vec
            .iter()
            .filter_map(|x| x.as_ref().copied())
            .flat_map(|list| {
                list.iter().map(|x| Some(x))
            })
            .collect();

        let child_writer = T::create_writer_batch(child_vector,&vec);

        writer.child_writer.push(child_writer);

        writer
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        let offset = writer.offset;

        let len = v.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.iter().enumerate() {
            T::write_valid(child_writer, offset + i, value);
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        DuckList::<T>::write_finish(writer)
    }
}


/// TypeId::Boolean

impl DuckValueType for bool {
    fn type_id() -> TypeId {
        TypeId::Boolean
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_bool(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_bool(idx, *v) }
    }
}

/// TypeId::BigInt      // i64
impl DuckValueType for i64 {
    fn type_id() -> TypeId {
        TypeId::BigInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i64(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i64(idx, *v) }
    }
}
/// TypeId::TinyInt     // i8
impl DuckValueType for i8 {
    fn type_id() -> TypeId {
        TypeId::TinyInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i8(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i8(idx, *v) }
    }
}
/// TypeId::SmallInt    // i16
impl DuckValueType for i16 {
    fn type_id() -> TypeId {
        TypeId::SmallInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i16(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i16(idx, *v) }
    }
}

/// TypeId::Integer     // i32
impl DuckValueType for i32 {
    fn type_id() -> TypeId {
        TypeId::Integer
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i32(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i32(idx, *v) }
    }
}

/// TypeId::UTinyInt    // u8
impl DuckValueType for u8 {
    fn type_id() -> TypeId {
        TypeId::UTinyInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u8(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u8(idx, *v) }
    }
}

/// TypeId::USmallInt   // u16
impl DuckValueType for u16 {
    fn type_id() -> TypeId {
        TypeId::USmallInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u16(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u16(idx, *v) }
    }
}
/// TypeId::UInteger    // u32

/// TypeId::UBigInt     // u64
impl DuckValueType for u64 {
    fn type_id() -> TypeId {
        TypeId::UBigInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u64(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u64(idx, *v) }
    }
}

/// TypeId::HugeInt     // i128
impl DuckValueType for i128 {
    fn type_id() -> TypeId {
        TypeId::HugeInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i128(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_i128(idx, *v) }
    }
}

/// TypeId::UHugeInt    // u128 不考虑，没read_u128这个方法
impl DuckValueType for u128 {
    fn type_id() -> TypeId {
        TypeId::UHugeInt
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u128(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_u128(idx, *v) }
    }
}

/// TypeId::Float       // f32
impl DuckValueType for f32 {
    fn type_id() -> TypeId {
        TypeId::Float
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f32(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f32(idx, *v) }
    }
}

/// TypeId::Double      // f64
impl DuckValueType for f64 {
    fn type_id() -> TypeId {
        TypeId::Double
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f64(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f64(idx, *v) }
    }
}
///TypeId::Timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestamp {
    pub micros_since_epoch: i64,
}

impl DuckValueType for DuckTimestamp {
    fn type_id() -> TypeId {
        TypeId::Timestamp
    }

    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_epoch: unsafe { reader.read_timestamp(row) },
        }
    }

    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp(idx, v.micros_since_epoch) }
    }
}
// TypeId::TimestampTz
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampTz {
    pub millis_since_epoch: i64,
}
// pub const unsafe fn write_timestamp_ms(&mut self, idx: usize, millis_since_epoch: i64) {
impl DuckValueType for DuckTimestampTz {
    fn type_id() -> TypeId {
        TypeId::TimestampTz
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            millis_since_epoch: unsafe { reader.read_timestamp_tz(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_ms(idx, v.millis_since_epoch) }
    }
}
// TypeId::TimestampS
// pub const unsafe fn write_timestamp_s(&mut self, idx: usize, seconds_since_epoch: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampS {
    pub seconds_since_epoch: i64,
}

impl DuckValueType for DuckTimestampS {
    fn type_id() -> TypeId {
        TypeId::TimestampS
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            seconds_since_epoch: unsafe { reader.read_timestamp_s(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_s(idx, v.seconds_since_epoch) }
    }
}
// TypeId::TimestampMs
// pub const unsafe fn write_timestamp_ms(&mut self, idx: usize, millis_since_epoch: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampMs {
    pub millis_since_epoch: i64,
}
impl DuckValueType for DuckTimestampMs {
    fn type_id() -> TypeId {
        TypeId::TimestampMs
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            millis_since_epoch: unsafe { reader.read_timestamp_ms(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_ms(idx, v.millis_since_epoch) }
    }
}

// TypeId::TimestampNs
// pub const unsafe fn write_timestamp_ns(&mut self, idx: usize, nanos_since_epoch: i64) {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestampNs {
    pub nanos_since_epoch: i64,
}

impl DuckValueType for DuckTimestampNs {
    fn type_id() -> TypeId {
        TypeId::TimestampNs
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            nanos_since_epoch: unsafe { reader.read_timestamp_ns(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_timestamp_ns(idx, v.nanos_since_epoch) }
    }
}
// TypeId::TimeTz
// pub const unsafe fn write_time_tz(&mut self, idx: usize, bits: u64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimeTz {
    pub bits: u64,
}

impl DuckValueType for DuckTimeTz {
    fn type_id() -> TypeId {
        TypeId::TimeTz
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            bits: unsafe { reader.read_time_tz(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_time_tz(idx, v.bits) }
    }
}
// TypeId::Decimal
// pub const unsafe fn read_decimal(&self, idx: usize, WIDTH: u8) -> i128 {
// pub const unsafe fn write_decimal(&mut self, idx: usize, WIDTH: u8, unscaled: i128) {
pub trait DecimalShapeDef:Sized+Clone{
    const WIDTH: u8;
    const SCALE: u8;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DuckDecimal<T: DecimalShapeDef> {
    pub unscaled: i128,
    pub scale: u8,
    pub shape: PhantomData<T>,
}
impl<T: DecimalShapeDef> DuckValueType for DuckDecimal<T> {
    fn type_id() -> TypeId {
        TypeId::Decimal
    }
    fn logical_type() -> LogicalType {
        todo!("Decimal暂不可用：\
        可能Decimal有问题，但不清楚怎么处理，且我自己用不到decimal，后面再说；\
        输入的decimal指定类型不合适，可能就是处理decimal的函数；输出可指定类型、但也未必合适了；\
        或许可以分为两个类型、一个读一个写，读用获取到的类型、写用指定的类型");
        LogicalType::decimal(T::WIDTH, T::SCALE)
    }
    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        let logical = unsafe { quack_rs::vector::vector_get_column_type(reader.c_duckdb_vector) };
        let width = unsafe { logical.decimal_width() };
        let scale = unsafe { logical.decimal_scale() };
        Self {
            scale,
            shape: PhantomData::<T>,
            unscaled: unsafe { reader.vector_reader.read_decimal(row, width) },
        }
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: &Self) {
        let logical = unsafe { quack_rs::vector::vector_get_column_type(writer.c_duckdb_vector) };
        let width = unsafe { logical.decimal_width() };
        // let scale = unsafe { logical.decimal_scale() };
        unsafe { writer.vector_writer.write_decimal(idx, width, vo.unscaled) }
    }
}


// pub const unsafe fn write_date(&mut self, idx: usize, days_since_epoch: i32) {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckDate {
    pub days_since_epoch: i32,
}

impl DuckValueType for DuckDate {
    fn type_id() -> TypeId {
        TypeId::Date
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            days_since_epoch: unsafe { reader.read_date(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_date(idx, v.days_since_epoch) }
    }
}
// pub const unsafe fn write_time(&mut self, idx: usize, micros_since_midnight: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTime {
    pub micros_since_midnight: i64,
}

impl DuckValueType for DuckTime {
    fn type_id() -> TypeId {
        TypeId::Time
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_midnight: unsafe { reader.read_time(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_time(idx, v.micros_since_midnight) }
    }
}
///TypeId::Interval
impl DuckValueType for DuckInterval {
    fn type_id() -> TypeId {
        TypeId::Interval
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_interval(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_interval(idx, *v) }
    }
}

/// TypeId::Varchar
impl DuckValueType for String {
    fn type_id() -> TypeId {
        TypeId::Varchar
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_str(row).to_string() }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_str(idx, v.as_str()) }
    }
}
// pub unsafe fn read_blob(&self, idx: usize) -> &[u8] {
// pub unsafe fn write_blob(&mut self, idx: usize, value: &[u8]) {
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuckBlob {
    pub value: Vec<u8>,
}
impl DuckValueType for DuckBlob {
    fn type_id() -> TypeId {
        TypeId::Blob
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            value: unsafe { reader.read_blob(row).to_vec() },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_blob(idx, v.value.as_slice()) }
    }
}

// pub const unsafe fn write_uuid(&mut self, idx: usize, value: i128) {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckUuid {
    pub value: u128,
}

impl DuckValueType for DuckUuid {
    fn type_id() -> TypeId {
        TypeId::Uuid
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self {
            value: unsafe { reader.read_uuid(row) },
        }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_uuid(idx, v.value) }
    }
}

// ===================================================
// 这几个类型quack-rs没做read、write方法
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
// TypeId::Struct
// TypeId::Map
// TypeId::Array
//
//
//
//
//
// ===================================================
