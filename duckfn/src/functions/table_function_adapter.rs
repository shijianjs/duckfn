//! 表函数适配层：把一个「返回迭代器」的 Rust 函数注册成 DuckDB 表函数。
//!
//! Table-function adapter: registers a Rust function that returns an iterator as a DuckDB
//! table function.

use crate::{
    DuckColumns, DuckOptionResult, DuckResult, panic_to_duck_error, vec_option_to_ref,
};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    BindInfo, LogicalType,
    TableFunctionBuilder,
};
use quack_rs::vector::vector_size;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// 表函数的数据源：一个可发送的迭代器，逐行产出「可能失败、可能为空」的结果。
///
/// The data source of a table function: a `Send` iterator that yields, row by row, a value
/// that may fail or be NULL.
pub type DuckFullIterator<T> = Box<dyn Iterator<Item=DuckOptionResult<T>> + Send>;
/// [`DuckFullIterator`] 的构造结果：构建迭代器本身也可能失败（例如参数非法）。
///
/// The construction result of a [`DuckFullIterator`]: building the iterator itself may fail
/// (for example on invalid parameters).
pub type DuckFullIteratorResult<T> = DuckResult<DuckFullIterator<T>>;

/// 把「参数结构体 -> 行迭代器」的纯 Rust 函数注册成 DuckDB 表函数。
///
/// 表函数的生命周期分两段：
///
/// 1. **bind**（[`Self::with_state`]）：声明输出列、解析参数，返回扫描状态（即行迭代器）；
/// 2. **scan**（[`Self::scan`]）：从状态里取一批行（至多 `vector_size()` 行）写入输出
///    chunk，并设置本批行数。
///
/// 一般不用手写这个 impl，直接用 `#[duck_table_function]` 作用在返回迭代器的函数上即可。
///
/// Registers a plain Rust function `Args -> row iterator` as a DuckDB table function. Its
/// life cycle has two phases: (1) **bind** ([`Self::with_state`]), which declares the output
/// columns, parses the parameters and returns the scan state (the row iterator); and
/// (2) **scan** ([`Self::scan`]), which pulls a batch of at most `vector_size()` rows from the
/// state, writes them into the output chunk and sets the batch size. Usually you do not
/// implement this manually: annotate a function returning an iterator with
/// `#[duck_table_function]`.
pub trait TableFunctionAdapter: Sized + 'static {
    /// 构造表函数 builder，并挂上 bind/scan 两个闭包。
    ///
    /// Builds the table-function builder and attaches the bind/scan closures.
    fn table_function_builder() -> DuckResult<TableFunctionBuilder> {
        let mut builder = TableFunctionBuilder::new(Self::NAME);
        builder = Self::config_params(builder);
        // 1. bind closure: declare the output schema, read parameters,
        //    return the initial scan state.
        builder
            .with_state(Self::with_state)
            // 2. scan closure: mutate state, write rows, set chunk size.
            // .scan(|state, chunk| Self::scan(state, chunk))
            .scan(Self::scan)
            .build()
    }

    /// 把 [`Self::Args`] 的参数表（位置参数/命名参数）登记到 builder 上。
    ///
    /// Registers the parameter list of [`Self::Args`] (positional and named) on the builder.
    fn config_params(builder: TableFunctionBuilder) -> TableFunctionBuilder {
        let mut builder = builder;
        for (name, ty) in Self::Args::bind_param_logical() {
            if let Some(name) = name {
                builder = builder.named_param_logical(&name, ty);
            } else {
                builder = builder.param_logical(ty)
            }
        }
        builder
    }

    /// bind 阶段：解析参数、声明输出列，并构造初始扫描状态（行迭代器）。
    ///
    /// bind 用 `catch_unwind` 包住，panic 会被转成查询错误而不会跨 FFI 展开。
    ///
    /// Bind phase: parses the parameters, declares the result columns and builds the initial
    /// scan state (the row iterator). The body is wrapped in `catch_unwind`, so a panic
    /// becomes a query error instead of unwinding across the FFI boundary.
    fn with_state(
        bind: &BindInfo,
    ) -> DuckFullIteratorResult<Self::Output> {
        catch_unwind(|| {
            let args: Self::Args = Self::read_args(bind)?;
            Self::config_result_columns(bind, &args);
            let x: DuckFullIterator<Self::Output> =
                Box::new(Self::init_data_iterator(args)?);
            Ok(x)
        })
        .map_err(panic_to_duck_error)? // 不用flatten以兼容1.86
    }

    /// 把 [`Self::Output`] 的列名与逻辑类型登记为结果列。
    ///
    /// Registers the column names and logical types of [`Self::Output`] as result columns.
    fn config_result_columns(bind: &BindInfo, _args: &Self::Args) {
        for (name, ty) in Self::Output::named_column_types() {
            bind.add_result_column_with_type(&name, &ty);
        }
    }

    /// 从 bind 信息里解析参数，默认委托给 [`DuckBindArgs::read_bind_args`]。
    ///
    /// Parses the arguments from the bind info; by default it delegates to
    /// [`DuckBindArgs::read_bind_args`].
    fn read_args(bind: &BindInfo) -> DuckResult<Self::Args> {
        Self::Args::read_bind_args(bind)
    }

    /// scan 阶段：从状态里取一批行写入 `chunk`，并设置本批实际行数。
    ///
    /// 迭代器耗尽（`None`）时提前结束并把 chunk 大小设为已写入的行数；用
    /// `catch_unwind` 捕获 panic 并转成查询错误。
    ///
    /// Scan phase: pulls one batch of rows from the state into `chunk` and sets the actual
    /// batch size. When the iterator is exhausted (`None`) it stops early and sets the chunk
    /// size to the number of rows already written. Panics are caught and turned into query
    /// errors.
    fn scan(
        state: &mut DuckFullIterator<Self::Output>,
        chunk: &DataChunk,
    ) -> DuckResult<()> {
        catch_unwind(AssertUnwindSafe(|| {
            let size = vector_size();
            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(size as usize);

            let mut count = size;
            for i in 0..size {
                let option = state.next();
                if let Some(value) = option {
                    output_vec.push(value?);
                } else if option.is_none() {
                    count = i;
                    break;
                }
            }
            Self::Output::write_columns_batch(chunk, &vec_option_to_ref(&output_vec));
            unsafe { chunk.set_size(count as usize) };
            Ok(())
        }))
        .map_err(panic_to_duck_error)? // 不用flatten以兼容1.86
    }

    /// 回调名称，仅用于标识（SQL 里的函数名由 builder 决定）。
    ///
    /// Callback name, used for identification only (the SQL name comes from the builder).
    const NAME: &'static str;
    /// 表函数的参数结构体类型，同时承担参数声明与解析。
    ///
    /// The argument struct type of the table function; it declares and parses the parameters.
    type Args: DuckBindArgs;
    /// 每一行的输出结构体类型，决定结果列的列名与类型。
    ///
    /// The per-row output struct type; it determines the result column names and types.
    type Output: DuckColumns;

    /// 表函数入口：由参数构造行迭代器（bind 阶段调用的真正业务逻辑）。
    ///
    /// Table-function entry point: builds the row iterator from the arguments (the actual
    /// business logic invoked during bind).
    fn init_data_iterator(
        args: Self::Args,
    ) -> DuckFullIteratorResult<Self::Output>;
}

/// 表函数参数抽象：描述 bind 参数表，并从 [`BindInfo`] 里解析出参数值。
///
/// 一般由 `#[derive(DuckStruct)]` 为参数结构体自动实现。
///
/// Table-function argument abstraction: describes the bind parameter list and parses the
/// argument values from [`BindInfo`]. Usually implemented automatically for the argument
/// struct by `#[derive(DuckStruct)]`.
pub trait DuckBindArgs: Sized {
    /// 从 bind 信息里读取参数值（位置参数按序号、命名参数按名字）。
    ///
    /// Reads the argument values from the bind info (positional by index, named by name).
    fn read_bind_args(bind: &BindInfo) -> DuckResult<Self>;

    /// 返回参数表：`(Some(名字), 类型)` 表示命名参数，`(None, 类型)` 表示位置参数。
    ///
    /// Returns the parameter list: `(Some(name), ty)` for a named parameter and
    /// `(None, ty)` for a positional one.
    fn bind_param_logical() -> Vec<(Option<String>, LogicalType)>;
}
