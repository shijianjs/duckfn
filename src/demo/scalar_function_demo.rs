use crate::wrapper::duck_value_type_convertor::DuckList;
use crate::wrapper::scalar_function_wrapper::ScalarFunctionAdapter;
use libduckdb_sys::{
    duckdb_data_chunk, duckdb_data_chunk_get_vector, duckdb_function_info, duckdb_vector,
};
use quack_rs::connection::Connection;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{ListVector, LogicalType, MapVector, Registrar, ScalarFunctionBuilder, StructVector, TypeId, VectorReader, VectorWriter};
use tuple_transpose::TupleTranspose;
use crate::wrapper::duck_register_builder::RegisterBuilder;

///
///
/// ```shell
/// cargo duckdb-ext build; duckdb -unsigned -c "
///   LOAD './target/debug/rusty_quack.duckdb_extension';
///   SELECT double_it5(3);
///   ";
/// ```
pub struct DoubleIt;

impl ScalarFunctionAdapter for DoubleIt {
    const NAME: &'static str = "double_it5";
    type Args = (Option<i64>,);
    type Output = i64;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| v * 2)
    }
}

pub struct FirstWordTuple;

impl ScalarFunctionAdapter for FirstWordTuple {
    const NAME: &'static str = "first_word_tuple";
    type Args = (Option<String>,);
    type Output = String;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| {
            v.as_str()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string()
        })
    }
}

pub struct AddItTuple;

impl ScalarFunctionAdapter for AddItTuple {
    const NAME: &'static str = "add_it_tuple";
    type Args = (Option<i64>, Option<i64>);
    type Output = i64;
    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v, v2)| v + v2)
    }
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

struct SumListWrapper;
impl ScalarFunctionAdapter for SumListWrapper {
    const NAME: &'static str = "sum_list_w";
    type Args = (Option<DuckList<i64>>,);
    type Output = i64;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| v.value.iter().flatten().sum())
    }
}
struct SumListNest;
impl ScalarFunctionAdapter for SumListNest {
    const NAME: &'static str = "sum_list_nest";
    type Args = (Option<DuckList<DuckList<i64>>>,);
    type Output = i64;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| {
            v.value
                .iter()
                .flatten()
                .map(|v| v.value.iter().flatten().sum::<i64>())
                .sum()
        })
    }
}



unsafe extern "C" fn make_list_scalar(
    _info: duckdb_function_info,
    input: duckdb_data_chunk,
    output: duckdb_vector,
) {
    // let mut writer = unsafe { VectorWriter::new(output) };

    let row_count = unsafe {
        libduckdb_sys::duckdb_data_chunk_get_size(input)
    } as usize;


    // output 是 LIST vector
    let list_vec = output;


    // 假设每行写 [1,2,3]
    let total_elements = row_count * 3;


    // 1. 预留 child 空间
    unsafe {
        ListVector::reserve(list_vec, total_elements);
    }


    // 2. 获取 child writer
    let mut child_writer = unsafe {
        ListVector::child_writer(list_vec)
    };


    let mut offset = 0usize;


    for row in 0..row_count {

        // 当前 row 对应 child 区间
        unsafe {
            ListVector::set_entry(
                list_vec,
                row,
                offset as u64,
                3,
            );
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
        ListVector::set_size(
            list_vec,
            total_elements,
        );
    }


    // 如果 list 本身有 null
    // writer.set_null(row)
    // 不要调用 set_entry
}
struct MakeListScalarWrapper;
impl ScalarFunctionAdapter for MakeListScalarWrapper {
    const NAME: &'static str = "make_list_scalar_w";
    type Args = (Option<i64>,);
    type Output = DuckList<i64>;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| DuckList{ value: vec![Some(v+1), Some(v*2), None]})
    }
}
struct NestListScalarWrapper;
impl ScalarFunctionAdapter for NestListScalarWrapper {
    const NAME: &'static str = "nest_list_scalar_w";
    type Args = (Option<i64>,);
    type Output = DuckList<DuckList<i64>>;

    fn apply(args: Self::Args) -> Option<Self::Output> {
        args.transpose().map(|(v,)| DuckList{ value: vec![
            Some(DuckList{ value: vec![Some(v+1), Some(v*2), None]}),
            Some(DuckList{ value: vec![Some(v+10), Some(v*10), None]}),
            None]})
    }
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

pub unsafe fn register(connection: &Connection) -> Result<(), ExtensionError> {
    let builders = vec![
        DoubleIt::register_builder(),
        FirstWordTuple::register_builder(),
        AddItTuple::register_builder(),
        SumListWrapper::register_builder(),
        SumListNest::register_builder(),
        MakeListScalarWrapper::register_builder(),
        NestListScalarWrapper::register_builder(),
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
                ("key",   TypeId::Varchar),
                ("value", TypeId::Integer),
            ]))
            .function(make_pair_scalar),
        ScalarFunctionBuilder::new("make_kv_map")
            .param(TypeId::Varchar)
            .param(TypeId::Integer)
            .returns_logical(LogicalType::map(TypeId::Varchar, TypeId::Integer))
            .function(make_kv_map_scalar),
    ];
    for builder in builders {
        unsafe { connection.register_scalar(builder) }?;
    }
    Ok(())
}
