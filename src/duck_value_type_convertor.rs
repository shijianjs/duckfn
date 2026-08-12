use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    AggregateFunctionBuilder, DuckInterval, LogicalType, ScalarFunctionBuilder,
    ScalarOverloadBuilder, TypeId, VectorReader, VectorWriter,
};

#[derive(Default)]
pub struct DuckTypeInfo {
    pub is_logical: bool,
    pub type_id: Option<TypeId>,
    pub logical_type: Option<LogicalType>,
}
impl DuckTypeInfo {
    pub fn new(type_id: TypeId) -> Self {
        DuckTypeInfo {
            is_logical: false,
            type_id: Some(type_id),
            logical_type: None,
        }
    }
    pub fn new_logical(logical_type: LogicalType) -> Self {
        DuckTypeInfo {
            is_logical: true,
            type_id: None,
            logical_type: Some(logical_type),
        }
    }
}
pub trait RegisterBuilder: Sized {
    fn param(self, type_id: TypeId) -> Self;
    fn param_logical(self, logical_type: LogicalType) -> Self;
    fn returns(self, type_id: TypeId) -> Self;
    fn returns_logical(self, logical_type: LogicalType) -> Self;
    //             let return_info = Self::Output::type_info();
    //         if let Some(t) = return_info.type_id {
    //             builder = builder.returns(t);
    //         } else if let Some(t) = return_info.logical_type {
    //             builder = builder.returns_logical(t);
    //         }
    //         for x in Self::Args::params() {
    //             if let Some(t) = x.type_id {
    //                 builder = builder.param(t);
    //             } else if let Some(t) = x.logical_type {
    //                 builder = builder.param_logical(t);
    //             }
    //         }
    fn with_return_type(self, return_info: DuckTypeInfo) -> Self {
        if let Some(t) = return_info.type_id {
            self.returns(t)
        } else if let Some(t) = return_info.logical_type {
            self.returns_logical(t)
        } else {
            self
        }
    }
    fn with_params(self, params: Vec<DuckTypeInfo>) -> Self {
        let mut builder = self;
        for param in params {
            if let Some(t) = param.type_id {
                builder = builder.param(t)
            } else if let Some(t) = param.logical_type {
                builder = builder.param_logical(t)
            }
        }
        builder
    }
}
impl RegisterBuilder for ScalarFunctionBuilder {
    fn param(self, type_id: TypeId) -> Self {
        self.param(type_id)
    }
    fn param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
    fn returns(self, type_id: TypeId) -> Self {
        self.returns(type_id)
    }
    fn returns_logical(self, logical_type: LogicalType) -> Self {
        self.returns_logical(logical_type)
    }
}
impl RegisterBuilder for ScalarOverloadBuilder {
    fn param(self, type_id: TypeId) -> Self {
        self.param(type_id)
    }
    fn param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
    fn returns(self, type_id: TypeId) -> Self {
        self.returns(type_id)
    }
    fn returns_logical(self, logical_type: LogicalType) -> Self {
        self.returns_logical(logical_type)
    }
}
impl RegisterBuilder for AggregateFunctionBuilder {
    fn param(self, type_id: TypeId) -> Self {
        self.param(type_id)
    }
    fn param_logical(self, logical_type: LogicalType) -> Self {
        self.param_logical(logical_type)
    }
    fn returns(self, type_id: TypeId) -> Self {
        self.returns(type_id)
    }
    fn returns_logical(self, logical_type: LogicalType) -> Self {
        self.returns_logical(logical_type)
    }
}

#[derive(Default)]
pub struct DuckValueReader {
    pub vector_reader: Option<VectorReader>,
    pub c_duckdb_vector: Option<duckdb_vector>,
}
impl DuckValueReader {
    pub fn new(vector_reader: VectorReader) -> Self {
        DuckValueReader {
            vector_reader: Some(vector_reader),
            c_duckdb_vector: None,
        }
    }
    pub fn new_logical(duckdb_vector: duckdb_vector) -> Self {
        DuckValueReader {
            vector_reader: None,
            c_duckdb_vector: Some(duckdb_vector),
        }
    }
}

/// 映射规则：
/// - 如果 Rust 基础类型已经完整表达了业务语义，可以直接映射；
/// - 如果多个逻辑类型共享同一个物理表示，就应该 newtype 包装。
pub trait DuckValueType: Sized {
    fn type_info() -> DuckTypeInfo {
        DuckTypeInfo {
            is_logical: Self::is_logical(),
            type_id: Self::type_id(),
            logical_type: Self::logical_type(),
        }
    }
    fn type_id() -> Option<TypeId> {
        None
    }
    fn logical_type() -> Option<LogicalType> {
        None
    }

    fn is_logical() -> bool {
        Self::logical_type().is_some()
    }

    fn create_reader(chunk: &DataChunk, column_index: usize) -> DuckValueReader {
        if !Self::is_logical() {
            DuckValueReader::new(unsafe { chunk.reader(column_index) })
        } else {
            DuckValueReader::new_logical(unsafe { chunk.vector(column_index) })
        }
    }

    fn read(reader: &DuckValueReader, row: usize) -> Option<Self> {
        if let Some(vector_reader) = &reader.vector_reader {
            Self::read_by_vector_reader(vector_reader, row)
        } else if let Some(c_duckdb_vector) = &reader.c_duckdb_vector {
            Self::read_by_c_duckdb_vector(c_duckdb_vector, row)
        } else {
            None
        }
    }
    fn read_by_c_duckdb_vector(c_duckdb_vector: &duckdb_vector, row: usize) -> Option<Self> {
        None
    }
    fn read_by_vector_reader(reader: &VectorReader, row: usize) -> Option<Self> {
        if unsafe { reader.is_valid(row) } {
            Some(Self::read_valid(reader, row))
        } else {
            None
        }
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self;

    fn write(writer: &mut VectorWriter, row: usize, vo: Option<Self>) {
        match vo {
            None => unsafe { writer.set_null(row) },
            Some(v) => Self::write_valid(writer, row, v),
        }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self);
}

/// TypeId::Boolean

impl DuckValueType for bool {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Boolean)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_bool(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_bool(row, v) }
    }
}

/// TypeId::BigInt      // i64
impl DuckValueType for i64 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::BigInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i64(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i64(row, v) }
    }
}
/// TypeId::TinyInt     // i8
impl DuckValueType for i8 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::TinyInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i8(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i8(row, v) }
    }
}
/// TypeId::SmallInt    // i16
impl DuckValueType for i16 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::SmallInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i16(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i16(row, v) }
    }
}

/// TypeId::Integer     // i32
impl DuckValueType for i32 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Integer)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i32(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_i32(row, v) }
    }
}

/// TypeId::UTinyInt    // u8
impl DuckValueType for u8 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::UTinyInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u8(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_u8(row, v) }
    }
}

/// TypeId::USmallInt   // u16
impl DuckValueType for u16 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::USmallInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u16(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_u16(row, v) }
    }
}
/// TypeId::UInteger    // u32

/// TypeId::UBigInt     // u64
impl DuckValueType for u64 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::UBigInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_u64(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_u64(row, v) }
    }
}

/// TypeId::HugeInt     // i128
impl DuckValueType for i128 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::HugeInt)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_i128(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
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
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Float)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f32(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_f32(row, v) }
    }
}

/// TypeId::Double      // f64
impl DuckValueType for f64 {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Double)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_f64(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_f64(row, v) }
    }
}
///TypeId::Timestamp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTimestamp {
    pub micros_since_epoch: i64,
}

impl DuckValueType for DuckTimestamp {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Timestamp)
    }

    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_epoch: unsafe { reader.read_timestamp(row) },
        }
    }

    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_timestamp(row, v.micros_since_epoch) }
    }
}
// pub const unsafe fn write_date(&mut self, idx: usize, days_since_epoch: i32) {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckDate {
    pub days_since_epoch: i32,
}

impl DuckValueType for DuckDate {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Date)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        Self {
            days_since_epoch: unsafe { reader.read_date(row) },
        }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_date(row, v.days_since_epoch) }
    }
}
// pub const unsafe fn write_time(&mut self, idx: usize, micros_since_midnight: i64) {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckTime {
    pub micros_since_midnight: i64,
}

impl DuckValueType for DuckTime {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Time)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        Self {
            micros_since_midnight: unsafe { reader.read_time(row) },
        }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_time(row, v.micros_since_midnight) }
    }
}
///TypeId::Interval
impl DuckValueType for DuckInterval {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Interval)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_interval(row) }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_interval(row, v) }
    }
}

/// TypeId::Varchar
impl DuckValueType for String {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Varchar)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        unsafe { reader.read_str(row).to_string() }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
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
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Blob)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        Self {
            value: unsafe { reader.read_blob(row).to_vec() },
        }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
        unsafe { writer.write_blob(row, v.value.as_slice()) }
    }
}

// pub const unsafe fn write_uuid(&mut self, idx: usize, value: i128) {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuckUuid {
    pub value: i128,
}

impl DuckValueType for DuckUuid {
    fn type_id() -> Option<TypeId> {
        Some(TypeId::Uuid)
    }
    fn read_valid(reader: &VectorReader, row: usize) -> Self {
        Self {
            value: unsafe { reader.read_uuid(row) },
        }
    }
    fn write_valid(writer: &mut VectorWriter, row: usize, v: Self) {
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
// TypeId::List
// TypeId::Struct
// TypeId::Map
// TypeId::Array
//
//
//
//
//
// ===================================================
