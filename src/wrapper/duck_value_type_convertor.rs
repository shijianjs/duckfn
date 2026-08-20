use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    DuckInterval, ListVector, LogicalType, StructVector, TypeId, VectorReader, VectorWriter,
};
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
}
impl DuckValueWriter {
    fn new_from_vector(vector: duckdb_vector) -> Self {
        Self {
            vector_writer: unsafe { VectorWriter::new(vector) },
            c_duckdb_vector: vector,
            child_writer: vec![],
            offset: 0,
        }
    }
}

/// 映射规则：
/// - 如果 Rust 基础类型已经完整表达了业务语义，可以直接映射；
/// - 如果多个逻辑类型共享同一个物理表示，就应该 newtype 包装。
pub trait DuckValueType: Sized {
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

    fn create_writer(output: duckdb_vector) -> DuckValueWriter {
        DuckValueWriter {
            vector_writer: unsafe { VectorWriter::new(output) },
            c_duckdb_vector: output,
            child_writer: vec![],
            offset: 0,
        }
    }

    fn write(writer: &mut DuckValueWriter, idx: usize, vo: Option<Self>) {
        // Self::write_to_vector(&mut writer.vector_writer, idx, vo);
        match vo {
            None => unsafe { writer.vector_writer.set_null(idx) },

            Some(v) => Self::write_valid(writer, idx, v),
        }
    }
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, vo: Self) {
        Self::write_valid_to_vector_writer(&mut writer.vector_writer, idx, vo)
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: Self) {
        todo!("子类需要实现write_valid")
    }

    fn write_finish(writer: &mut DuckValueWriter) {}

    fn struct_child_reader(reader: &DuckValueReader, field_index: usize) -> DuckValueReader {
        let row_count = reader.vector_reader.row_count();
        let vector = reader.c_duckdb_vector;
        let f1_vector = unsafe { StructVector::get_child(vector, field_index) };
        let f1_reader = Self::create_reader_from_vector(f1_vector, row_count);
        f1_reader
    }
}

pub trait FieldNames: Sized {
    // const FIELD_NAMES: &'static [&'static str] = &["hello_count"];
    const FIELD_NAMES: &'static [&'static str];
}
pub struct DuckStruct1<F0: DuckValueType, N: FieldNames> {
    pub f0: Option<F0>,
    pub field_names_type: PhantomData<N>,
}
impl<F0: DuckValueType, N: FieldNames> DuckStruct1<F0, N> {}
impl<F0: DuckValueType, N: FieldNames> DuckValueType for DuckStruct1<F0, N> {
    fn type_id() -> TypeId {
        TypeId::Struct
    }
    fn logical_type() -> LogicalType {
        LogicalType::struct_type_from_logical(&vec![(N::FIELD_NAMES[0], F0::logical_type())])
    }

    fn create_reader_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
        let mut reader = DuckValueReader::new_from_vector(vector, size);
        let f0_reader = F0::struct_child_reader(&reader,0);
        reader.child_reader = vec![f0_reader];
        reader
    }

    fn read_valid(reader: &DuckValueReader, row: usize) -> Self {
        Self{
            f0: F0::read(&reader.child_reader[0], row),
            field_names_type: PhantomData,
        }
    }
}
pub struct DuckStruct2<F0: DuckValueType, F1: DuckValueType, N: FieldNames> {
    pub f0: Option<F0>,
    pub f1: Option<F1>,
    pub field_names_type: PhantomData<N>,
}
impl<F0: DuckValueType, F1: DuckValueType, N: FieldNames> DuckStruct2<F0, F1, N> {}
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
        let f0_reader = F0::struct_child_reader(&reader, 0);
        let f1_reader = F1::struct_child_reader(&reader, 1);
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
}

impl<F0: DuckValueType, F1: DuckValueType, N: FieldNames> DuckStruct2<F0, F1, N> {
}

// TypeId::List
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

    fn create_writer(output: duckdb_vector) -> DuckValueWriter {
        let mut writer = DuckValueWriter::new_from_vector(output);

        let child_vector = unsafe { ListVector::get_child(output) };

        let child_writer = T::create_writer(child_vector);

        writer.child_writer.push(child_writer);

        writer
    }

    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: Self) {
        let offset = writer.offset;

        let len = v.value.len();

        unsafe {
            ListVector::set_entry(writer.c_duckdb_vector, idx, offset as u64, len as u64);
        }

        let child_writer = &mut writer.child_writer[0];

        for (i, value) in v.value.into_iter().enumerate() {
            T::write(child_writer, offset + i, value);
        }
        writer.offset += len;
    }

    fn write_finish(writer: &mut DuckValueWriter) {
        T::write_finish(&mut writer.child_writer[0]);

        unsafe {
            ListVector::set_size(writer.c_duckdb_vector, writer.offset);
        }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_bool(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i64(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i8(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i16(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i32(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_u8(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_u16(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_u64(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i128(row, v) }
    }
}

/// TypeId::UHugeInt    // u128 不考虑，没read_u128这个方法
// impl DuckValueType for u128 {
//     fn type_id() -> TypeId {
//         TypeId::UHugeInt
//     }
//     fn read_valid(reader: &VectorReader, row: usize) -> Self {
//         unsafe { reader.read_u128(row) }
//     }
//     fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
//         unsafe { writer.write_u128(row, v) }
//     }
// }

/// TypeId::Float       // f32
impl DuckValueType for f32 {
    fn type_id() -> TypeId {
        TypeId::Float
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f32(row) }
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_f32(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_f64(row, v) }
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

    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_timestamp(row, v.micros_since_epoch) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_date(row, v.days_since_epoch) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_time(row, v.micros_since_midnight) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_interval(row, v) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_str(row, v.as_str()) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_blob(row, v.value.as_slice()) }
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
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_uuid(row, v.value) }
    }
}

// ===================================================
// 这几个类型quack-rs没做read、write方法
//
// TypeId::TimestampTz
// TypeId::TimestampS
// TypeId::TimestampMs
// TypeId::TimestampNs
// TypeId::UHugeInt
// TypeId::TimeTz
// TypeId::Decimal
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
