use indexmap::IndexMap;
use easy_duckdb_extension::RegisterBuilder;
use easy_duckdb_extension::ScalarFunctionAdapter;
use easy_duckdb_extension::DuckList;
use easy_duckdb_extension::{DuckStruct1, DuckStruct2, FieldNames};
use libduckdb_sys::{
    duckdb_data_chunk, duckdb_data_chunk_get_vector, duckdb_function_info, duckdb_vector,
};
use quack_rs::connection::Connection;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{
    ListVector, LogicalType, MapVector, Registrar, ScalarFunctionBuilder, StructVector, TypeId,
    VectorReader, VectorWriter,
};
use tuple_transpose::TupleTranspose;
use easy_duckdb_extension::{duck_error, DuckOptionResult, DuckResult};
use easy_duckdb_extension_macro::{duck_scalar_function, DuckStruct};

///
///
/// ```sql
///   SELECT double_it5(3);
/// ```

#[duck_scalar_function]
pub fn double_it5(input:i64)->i64{
    input*2
}


#[duck_scalar_function]
pub fn first_word_tuple(input: Option<String>) -> Option<String> {
    input.map(|v| {
        v.as_str()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    })
}


#[duck_scalar_function]
pub fn add_it_tuple(input: Option<i64>, v2: i64) -> Option<i64> {
    input.map(|v| v + v2)
}


// ============================================================================
// Scalar: sum_list(LIST(BIGINT)) → BIGINT
// ============================================================================

unsafe extern "C" fn sum_list_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    let reader = unsafe { VectorReader::new(input, 0) };
    let mut writer = unsafe { VectorWriter::new(output) };
    let row_count = reader.row_count();
    let list_vec: duckdb_vector = unsafe { duckdb_data_chunk_get_vector(input, 0) };
    let child_reader = unsafe {
        VectorReader::from_vector(
            ListVector::get_child(list_vec),
            ListVector::get_size(list_vec),
        )
    };
    for row in 0..row_count {
        if !unsafe { reader.is_valid(row) } {
            unsafe { writer.set_null(row) };
            continue;
        }
        let entry = unsafe { ListVector::get_entry(list_vec, row) };

        let mut sum: i64 = 0;
        for i in 0..entry.length as usize {
            let idx = entry.offset as usize + i;
            if unsafe { child_reader.is_valid(idx) } {
                sum += unsafe { child_reader.read_i64(idx) };
            }
        }
        unsafe { writer.write_i64(row, sum) };
    }
}

/// ```sql
/// SELECT sum_list_w([1,2,3,4]);
/// ```
#[duck_scalar_function]
pub fn sum_list_w(li: Vec<i64>) -> i64 {
    li.iter().sum()
}

/// ```sql
/// SELECT sum_list_nest(v) from (values (
///   [[1,2],[3,null,4],null]),
///   ([[1],[3,null]])
/// ) t(v);
/// ```
#[duck_scalar_function]
pub fn sum_list_nest(input: Vec<Option<Vec<Option<i64>>>>) -> i64 {
    input
        .iter()
        .flatten()
        .map(|v| v.iter().flatten().sum::<i64>())
        .sum()
}

unsafe extern "C" fn make_list_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    // let mut writer = unsafe { VectorWriter::new(output) };

    let row_count = unsafe { libduckdb_sys::duckdb_data_chunk_get_size(input) } as usize;

    // output 是 LIST vector
    let list_vec = output;

    // 假设每行写 [1,2,3]
    let total_elements = row_count * 3;

    // 1. 预留 child 空间
    unsafe {
        ListVector::reserve(list_vec, total_elements);
    }

    // 2. 获取 child writer
    let mut child_writer = unsafe { ListVector::child_writer(list_vec) };

    let mut offset = 0usize;

    for row in 0..row_count {
        // 当前 row 对应 child 区间
        unsafe {
            ListVector::set_entry(list_vec, row, offset as u64, 3);
        }

        // 写 child
        unsafe {
            child_writer.write_i64(offset, 1);
            child_writer.write_i64(offset + 1, 2);
            child_writer.write_i64(offset + 2, 3);
        }
        println!("row {} offset {}", row, offset);

        offset += 3;
    }

    // 3. 告诉 DuckDB child vector 有多少元素
    unsafe {
        ListVector::set_size(list_vec, total_elements);
    }

    // 如果 list 本身有 null
    // writer.set_null(row)
    // 不要调用 set_entry
}

/// ```sql
/// SELECT make_list_scalar_w(range) from range(10);
/// ```
#[duck_scalar_function]
fn make_list_scalar_w(v: i64) -> Vec<Option<i64>> {
    vec![Some(v + 1), Some(v * 2), None]
}


/// ```sql
/// SELECT nest_list_scalar_w(range) from range(10);
/// ```
#[duck_scalar_function]
fn nest_list_scalar_w(v: i64) -> Vec<Option<Vec<Option<i64>>>> {
    vec![
        Some(vec![Some(v + 1), Some(v * 2), None]),
        Some((0..v).map(|x| Some(x)).collect()),
        None,
    ]
}



/// ```sql
/// SELECT nest_vec_no_null_scalar_w(range) from range(10);
/// ```
#[duck_scalar_function]
pub fn nest_vec_no_null_scalar_w(v: i64) -> Vec<Vec<i64>> {
    vec![vec![v + 1, v * 2], (0..v).collect()]
}

#[derive(DuckStruct, Clone, Default, Debug)]
pub struct StructScalarWrapperArg1{
    hello_count: i64,
}
/// ```sql
/// SELECT struct_scalar_w({hello_count:15})
/// ```
#[duck_scalar_function]
pub fn struct_scalar_w(arg1:StructScalarWrapperArg1)->i64{
    arg1.hello_count + 10
}


#[derive(DuckStruct, Clone, Default, Debug)]
pub struct NestOuter{
    structf:NestInner,
    list:Vec<Option<i64>>
}
#[derive(DuckStruct, Clone, Default, Debug)]
pub struct NestInner{
    hello_count: i64,
}
/// ```sql
/// SELECT struct_nest_scalar_w({struct:{hello_count:15},list:[1,null,2]});
/// ```
#[duck_scalar_function]
pub fn struct_nest_scalar_w(arg1: NestOuter) -> i64 {
    arg1.structf.hello_count + arg1.list.iter().flatten().sum::<i64>()
}


/// ```sql
/// SELECT struct_nest_output_scalar_w(10)
/// ```
#[duck_scalar_function]
pub fn struct_nest_output_scalar_w(arg1:i32)->NestOuter{
    NestOuter {
        structf: NestInner {
            hello_count: 100 + arg1 as i64,
        },
        list: (0..arg1).map(|x| Some(x as i64)).collect(),
    }
}



/// ```sql
/// SELECT error_scalar_demo(10);
/// SELECT error_scalar_demo(20);
/// SELECT error_scalar_demo(30);
/// SELECT error_scalar_demo(40);
/// ```
#[duck_scalar_function]
pub fn error_scalar_demo(i:i64) -> DuckOptionResult<i64> {
    if i==10 {
        return Err(duck_error("error: input is 10"));
    }
    if i==20 {
        panic!("panic: input is 20")
    }
    if i==30 {
        panic!()
    }
    Ok(Some(i *2))
}


// ============================================================================
// Scalar: make_pair(VARCHAR, INTEGER) → STRUCT(key VARCHAR, value INTEGER)
// ============================================================================

unsafe extern "C" fn make_pair_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    let key_reader = unsafe { VectorReader::new(input, 0) };
    let val_reader = unsafe { VectorReader::new(input, 1) };
    let row_count = key_reader.row_count();

    let mut key_writer = unsafe { StructVector::field_writer(output, 0) };
    let mut val_writer = unsafe { StructVector::field_writer(output, 1) };

    for row in 0..row_count {
        let key_valid = unsafe { key_reader.is_valid(row) };
        let val_valid = unsafe { val_reader.is_valid(row) };
        if !key_valid || !val_valid {
            let mut parent_writer = unsafe { VectorWriter::new(output) };
            unsafe { parent_writer.set_null(row) };
            continue;
        }
        let k = unsafe { key_reader.read_str(row) };
        let v = unsafe { val_reader.read_i32(row) };
        unsafe { key_writer.write_varchar(row, k) };
        unsafe { val_writer.write_i32(row, v) };
    }
}

// ============================================================================
// Scalar: make_kv_map(VARCHAR, INTEGER) → MAP(VARCHAR, INTEGER)
//
// Demonstrates MapVector, LogicalType::map(), and map write workflow.
// Creates a single-entry map {key: k, value: v} per row.
// ============================================================================

unsafe extern "C" fn make_kv_map_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    let key_reader = unsafe { VectorReader::new(input, 0) };
    let val_reader = unsafe { VectorReader::new(input, 1) };
    let row_count = key_reader.row_count();

    // Reserve space in the MAP child vector for row_count entries (1 per row)
    unsafe { MapVector::reserve(output, row_count) };

    // Get the key and value child vectors
    let keys_vec = unsafe { MapVector::keys(output) };
    let vals_vec = unsafe { MapVector::values(output) };
    let mut key_writer = unsafe { VectorWriter::from_vector(keys_vec) };
    let mut val_writer = unsafe { VectorWriter::from_vector(vals_vec) };

    let mut entry_offset: u64 = 0;
    for row in 0..row_count {
        let key_valid = unsafe { key_reader.is_valid(row) };
        let val_valid = unsafe { val_reader.is_valid(row) };
        if !key_valid || !val_valid {
            // Empty map for NULL inputs
            unsafe { MapVector::set_entry(output, row, entry_offset, 0) };
            continue;
        }
        let k = unsafe { key_reader.read_str(row) };
        let v = unsafe { val_reader.read_i32(row) };

        let child_idx = entry_offset as usize;
        unsafe { key_writer.write_varchar(child_idx, k) };
        unsafe { val_writer.write_i32(child_idx, v) };

        unsafe { MapVector::set_entry(output, row, entry_offset, 1) };
        entry_offset += 1;
    }

    unsafe { MapVector::set_size(output, entry_offset as usize) };
}

/// ```sql
/// SELECT create_map_demo(range) from range(10);
/// ```
#[duck_scalar_function]
pub fn create_map_demo(i:i64)->IndexMap<String, Option<Vec<i64>>>{
    let map = (0..i).map(|x| (format!("key {x}"), if x == 5 { None } else { Some((0..x).collect()) })).collect();
    // println!("{:?}", map);
    map
}


pub unsafe fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let builders = vec![
        // DoubleIt::register_builder(),
        double_it5::scalar_function_builder(),
        first_word_tuple::scalar_function_builder(),
        add_it_tuple::scalar_function_builder(),
        sum_list_w::scalar_function_builder(),
        sum_list_nest::scalar_function_builder(),
        make_list_scalar_w::scalar_function_builder(),
        nest_list_scalar_w::scalar_function_builder(),
        ScalarFunctionBuilder::new("sum_list")
            .param_logical(LogicalType::list(TypeId::BigInt))
            .returns(TypeId::BigInt)
            .function(sum_list_scalar),
        ScalarFunctionBuilder::new("make_list_scalar")
            .param_logical(LogicalType::new(TypeId::BigInt))
            .returns_logical(LogicalType::list(TypeId::BigInt))
            .function(make_list_scalar),
        ScalarFunctionBuilder::new("make_pair")
            .param(TypeId::Varchar)
            .param(TypeId::Integer)
            .returns_logical(LogicalType::struct_type(&[
                ("key", TypeId::Varchar),
                ("value", TypeId::Integer),
            ]))
            .function(make_pair_scalar),
        ScalarFunctionBuilder::new("make_kv_map")
            .param(TypeId::Varchar)
            .param(TypeId::Integer)
            .returns_logical(LogicalType::map(TypeId::Varchar, TypeId::Integer))
            .function(make_kv_map_scalar),
        struct_scalar_w::scalar_function_builder(),
        struct_nest_scalar_w::scalar_function_builder(),
        struct_nest_output_scalar_w::scalar_function_builder(),
        nest_vec_no_null_scalar_w::scalar_function_builder(),
        error_scalar_demo::scalar_function_builder(),
        create_map_demo::scalar_function_builder(),
    ];
    for builder in builders {
        unsafe { connection.register_scalar(builder) }?;
    }
    Ok(())
}
