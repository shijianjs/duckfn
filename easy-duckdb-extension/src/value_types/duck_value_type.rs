use libduckdb_sys::duckdb_vector;
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{LogicalType, StructVector, TypeId, VectorReader, VectorWriter};

/// 映射规则：
/// - 如果 Rust 基础类型已经完整表达了业务语义，可以直接映射；
/// - 如果多个逻辑类型共享同一个物理表示，就应该 newtype 包装。
pub trait DuckValueType: Sized+Clone {
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

    /// 仅处理null，子类不能重写
    fn read(reader: &DuckValueReader, row: usize) -> Option<Self> {
        if unsafe { reader.vector_reader.is_valid(row) } {
            Self::read_valid(reader, row)
        } else {
            None
        }
    }
    
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Some(Self::read_valid_by_vector_reader(&reader.vector_reader, row))
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

    /// 仅处理null，子类不能重写、因为可能调不到
    fn write(writer: &mut DuckValueWriter, idx: usize, vo: &Option<Self>) {
        // Self::write_to_vector(&mut writer.vector_writer, idx, vo);
        match vo {
            None => unsafe { writer.vector_writer.set_null(idx) },

            Some(v) => Self::write_valid(writer, idx, v),
        }
    }
    /// 外部可以调用write_valid
    /// - 只要已经处理了null，就不需要管其他的
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

/// 用来给宏校验 DuckValueType 是否被类型实现
pub fn assert_impl_duck_value_type<T: DuckValueType>() {}


pub struct DuckValueReader {
    /// rust api, 和下面的c_duckdb_vector一比一对应
    pub vector_reader: VectorReader,
    /// c api, 和上面的reader一比一对应
    pub c_duckdb_vector: duckdb_vector,
    /// 基于上面的c_duckdb_vector的子reader
    pub child_reader: Vec<DuckValueReader>,
}

impl DuckValueReader {
    pub fn new_from_chunk(chunk: &DataChunk, column_index: usize) -> Self {
        let vector = unsafe { chunk.vector(column_index) };
        let size = chunk.size();
        Self::new_from_vector(vector, size)
    }

    pub fn new_from_vector(vector: duckdb_vector, size: usize) -> DuckValueReader {
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
