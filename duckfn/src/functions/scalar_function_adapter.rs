//! 标量函数适配层：把 quack-rs 的向量级 C 回调拆成「逐行」的 Rust 代码。
//!
//! Scalar-function adapter: splits quack-rs' vector-level C callback into per-row Rust code.

use crate::duck_columns::DuckColumns;
use crate::utils::builder_with_params::BuilderWithParams;
use crate::value_types::duck_value_type::DuckValueType;
use crate::{DuckOptionResult, DuckResult, duck_scalar_unwind, vec_option_to_ref};
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{NullHandling, ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder};

/// 把「参数结构体 -> 输出值」的纯 Rust 函数注册成 DuckDB 标量函数。
///
/// 适配层把 quack-rs 的向量级回调拆成逐行的 Rust 代码：
///
/// - 用 [`Self::Args`]（实现 [`DuckColumns`]）从输入 `DataChunk` 逐行取参；
/// - 每行调用 [`Self::apply`]（或 NULL 时代替的 [`Self::apply_with_null`]）；
/// - 收集成 `Vec<Option<Self::Output>>` 后一次性写入输出向量。
///
/// 一般不用手写这个 impl，直接用 `#[duck_scalar_function]` 作用在普通函数上即可。
///
/// Registers a plain Rust function `Args -> Output` as a DuckDB scalar function. The adapter
/// splits quack-rs' vector-level callback into per-row Rust code: it reads arguments row by
/// row through [`Self::Args`] (a [`DuckColumns`] implementation), calls [`Self::apply`] (or
/// [`Self::apply_with_null`] for NULL rows) and finally writes the collected results into the
/// output vector in one batch. Usually you do not implement this manually: just annotate a
/// plain function with `#[duck_scalar_function]`.
pub trait ScalarFunctionAdapter: Sized + 'static {
    /// # Safety
    ///
    /// 由 DuckDB 回调，`_info`/`input`/`output` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `_info`, `input` and `output` are guaranteed valid by DuckDB.
    unsafe extern "C" fn scalar_function_wrapper(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
        let info: ScalarFunctionInfo = unsafe { ScalarFunctionInfo::new(_info) };
        duck_scalar_unwind(&info,|| {
            let chunk: DataChunk = unsafe { DataChunk::from_raw(input) };
            let readers = Self::Args::create_column_readers(&chunk);
            let row_count = chunk.size();

            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(row_count);
            for row in 0..row_count {
                let args = Self::Args::read_columns(&readers, row);
                let result = Self::apply_with_null(args);
                match result {
                    Ok(r) => output_vec.push(r),
                    Err(e) => {
                        info.set_error(e.as_str());
                        return;
                    }
                }
            }
            Self::Output::write_batch(output, &vec_option_to_ref(&output_vec));
        });
    }

    /// NULL 处理策略，默认 [`NullHandling::DefaultNullHandling`]（NULL 行不进入回调）。
    ///
    /// Null-handling strategy; defaults to [`NullHandling::DefaultNullHandling`] (NULL rows
    /// never reach the callback).
    fn null_handling() -> NullHandling {
        NullHandling::DefaultNullHandling
    }

    /// 构造「独立函数」用的 builder（自带函数名）。
    ///
    /// Builds the builder for a standalone function (carrying its own name).
    fn scalar_function_builder() -> ScalarFunctionBuilder {
        ScalarFunctionBuilder::new(Self::NAME)
            .function(Self::scalar_function_wrapper)
            .null_handling(Self::null_handling())
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types())
    }

    /// 构造「函数集重载」用的 builder（不带函数名，由函数集决定）。
    ///
    /// Builds the builder for a function-set overload (no name; the set provides it).
    fn scalar_overload_builder() -> ScalarOverloadBuilder {
        ScalarOverloadBuilder::new()
            .function(Self::scalar_function_wrapper)
            .null_handling(Self::null_handling())
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types())
    }

    /// # Safety
    ///
    /// `con` 必须是有效的 DuckDB 连接句柄。
    ///
    /// `con` must be a valid DuckDB connection handle.
    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        unsafe { Self::scalar_function_builder().register(con) }
    }

    /// 回调名称，仅用于标识（SQL 里的函数名由 builder 决定）。
    ///
    /// Callback name, used for identification only (the SQL name comes from the builder).
    const NAME: &'static str;
    /// 参数结构体类型：实现 [`DuckColumns`]，负责按行读取各参数列。
    ///
    /// The argument struct type: implements [`DuckColumns`] and reads each argument column
    /// row by row.
    type Args: DuckColumns;
    /// 输出值类型：决定返回的 DuckDB 逻辑类型与写向量方式。
    ///
    /// The output value type: determines the returned DuckDB logical type and how values are
    /// written into the vector.
    type Output: DuckValueType;

    /// NULL 行的默认处理：入参为 `None`（本行有 NULL 且参数不可空）时直接输出 NULL。
    ///
    /// Default handling of NULL rows: when the arguments are `None` (a NULL in this row with
    /// non-nullable parameters), output NULL directly.
    fn apply_with_null(args_option: Option<Self::Args>) -> DuckOptionResult<Self::Output> {
        if let Some(args) = args_option {
            Self::apply(args)
        } else {
            Ok(None)
        }
    }

    /// 对一行非 NULL 参数求值；返回 `Ok(None)` 表示该行输出 SQL NULL。
    ///
    /// Evaluates one row of non-NULL arguments; returning `Ok(None)` makes this row SQL NULL.
    fn apply(args: Self::Args) -> DuckOptionResult<Self::Output>;
}
