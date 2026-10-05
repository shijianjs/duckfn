//! 表函数适配层：把一个「返回迭代器」的 Rust 函数注册成 DuckDB 表函数。
//!
//! Table-function adapter: registers a Rust function that returns an iterator as a DuckDB
//! table function.
//!
//! `catch_unwind` 的 panic 兜底**仅原生成立**；在 wasm/浏览器 (`wasm32-unknown-emscripten`) 上
//! 接不住 —— panic 无法跨 JS 边界展开，会变成 `Maximum call stack size exceeded` 栈溢出。
//! 报错请用 `Err(duck_error(..))` 而非 `panic!`（详见 `crate::utils::helpers` 模块文档）。
//!
//! The `catch_unwind` panic guard is native-only: on wasm a `panic!` cannot unwind across the JS
//! boundary and surfaces as a stack overflow. Report errors with `Err(duck_error(..))`, not `panic!`.

use crate::{
    DuckColumns, DuckDynamicIterator, DuckDynamicRow, DuckDynamicTable, DuckOptionResult,
    DuckResult, DuckResultSchema, panic_to_duck_error, vec_option_to_ref,
};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    BindInfo, LogicalType,
    TableFunctionBuilder,
};
use quack_rs::vector::vector_size;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// 把一组 [`DuckBindArgs`] 的参数表（位置参数 / 命名参数）登记到 builder 上。
///
/// Registers the parameter list of a [`DuckBindArgs`] type (positional and named) on a builder.
///
/// 静态表函数与动态表函数共用的唯一一份实现：位置参数以 `param_logical` 登记，命名参数以
/// `named_param_logical` 登记（顺序即声明顺序，命名参数必须排在位置参数之后）。
///
/// The single implementation shared by static and dynamic table functions: positional parameters go
/// through `param_logical` and named ones through `named_param_logical` (in declaration order, with
/// named parameters after the positional ones).
pub fn config_bind_params<A: DuckBindArgs>(builder: TableFunctionBuilder) -> TableFunctionBuilder {
    let mut builder = builder;
    for (name, ty) in A::bind_param_logical() {
        if let Some(name) = name {
            builder = builder.named_param_logical(&name, ty);
        } else {
            builder = builder.param_logical(ty)
        }
    }
    builder
}

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
/// 表函数的生命周期分三段：
///
/// 1. **bind**（[`Self::bind_state`]）：声明输出列、解析参数，返回每次执行共用的 bind data（参数）；
/// 2. **init**（[`Self::init_state`]）：每次执行都从 bind data 重建扫描状态（行迭代器）；
/// 3. **scan**（[`Self::scan`]）：从状态里取一批行（至多 `vector_size()` 行）写入输出
///    chunk，并设置本批行数。
///
/// 一般不用手写这个 impl，直接用 `#[duck_table_function]` 作用在返回迭代器的函数上即可。
///
/// Registers a plain Rust function `Args -> row iterator` as a DuckDB table function. Its
/// life cycle has three phases: (1) **bind** ([`Self::bind_state`]), which declares the output
/// columns, parses the parameters and returns the bind data (the arguments) shared by every
/// execution; (2) **init** ([`Self::init_state`]), which rebuilds the scan state (the row iterator)
/// from that bind data for every execution; and (3) **scan** ([`Self::scan`]), which pulls a batch
/// of at most `vector_size()` rows from the state, writes them into the output chunk and sets the
/// batch size. Usually you do not implement this manually: annotate a function returning an iterator
/// with `#[duck_table_function]`.
///
/// 这里**没有** `extra_info` 钩子：本适配层走 quack-rs 的 typed 表函数 builder，而该 builder 把
/// `extra_info` 槽位用来存它自己的 bind/scan 闭包了。表函数本来就以 per-query 状态传数据
/// （[`Self::init_state`] 返回的迭代器），需要跨查询共享的只读数据用标准库的
/// `OnceLock` / `LazyLock` 即可。需要读 `extra_info` 又愿意自己接管注册的话，可以重写
/// [`Self::table_function_builder`]。
///
/// There is **no** `extra_info` hook here: this adapter goes through quack-rs' typed table-function
/// builder, which uses the `extra_info` slot for its own bind/scan closures. Table functions already
/// pass their data as per-query state (the iterator returned by [`Self::init_state`]); use the
/// standard library's `OnceLock` / `LazyLock` for read-only data shared across queries. If you need
/// `extra_info` and are willing to own the registration, override
/// [`Self::table_function_builder`].
pub trait TableFunctionAdapter: Sized + 'static
where
    Self::Args: Clone + Send + Sync + 'static,
{
    /// 构造表函数 builder，并挂上 bind/init/scan 三个闭包。
    ///
    /// Builds the table-function builder and attaches the bind/init/scan closures.
    fn table_function_builder() -> DuckResult<TableFunctionBuilder> {
        let mut builder = TableFunctionBuilder::new(Self::NAME);
        builder = Self::config_params(builder);
        // 1. bind closure: declare the output schema, read parameters, return the bind data.
        // 2. init closure: rebuild the per-execution scan state from the bind data.
        builder
            .with_bind_init(Self::bind_state, Self::init_state)
            // 3. scan closure: mutate state, write rows, set chunk size.
            // .scan(|state, chunk| Self::scan(state, chunk))
            .scan(Self::scan)
            .build()
    }

    /// 把 [`Self::Args`] 的参数表（位置参数/命名参数）登记到 builder 上。
    ///
    /// Registers the parameter list of [`Self::Args`] (positional and named) on the builder.
    fn config_params(builder: TableFunctionBuilder) -> TableFunctionBuilder {
        config_bind_params::<Self::Args>(builder)
    }

    /// bind 阶段：解析参数、声明输出列，并把参数作为 bind data 返回。
    ///
    /// 这里用 `with_bind_init` 而非 `with_state`：扫描状态是行迭代器，不可克隆，无法当作「每次执行
    /// 克隆一份」的模板；改为把参数存成 bind data，执行时在 [`Self::init_state`] 里重建迭代器。
    /// bind 用 `catch_unwind` 包住，panic 会被转成查询错误而不会跨 FFI 展开。
    ///
    /// Bind phase: parses the parameters, declares the result columns and returns the parameters as
    /// the bind data. `with_bind_init` is used rather than `with_state`: the scan state is a row
    /// iterator, which cannot be cloned and thus cannot be the "clone once per execution" template;
    /// the parameters are stored as bind data instead, and the iterator is rebuilt in
    /// [`Self::init_state`] per execution. The body is wrapped in `catch_unwind`, so a panic becomes
    /// a query error instead of unwinding across the FFI boundary.
    fn bind_state(bind: &BindInfo) -> DuckResult<Self::Args> {
        catch_unwind(|| {
            let args: Self::Args = Self::read_args(bind)?;
            Self::config_result_columns(bind, &args);
            Ok(args)
        })
        .map_err(panic_to_duck_error)? // 不用flatten以兼容1.86
    }

    /// init 阶段：从 bind data（参数）重建本次执行的行迭代器。
    ///
    /// 每次执行都会调用一次，所以 prepared statement 重复 `EXECUTE`、递归 CTE 里的重复扫描都从头
    /// 开始；同样用 `catch_unwind` 兜住 panic。
    ///
    /// Init phase: rebuilds this execution's row iterator from the bind data (the parameters). It runs
    /// once per execution, so repeated `EXECUTE` of a prepared statement and re-scans inside a
    /// recursive CTE start from the beginning; `catch_unwind` guards a panic as above.
    fn init_state(args: &Self::Args) -> DuckFullIteratorResult<Self::Output> {
        catch_unwind(AssertUnwindSafe(|| {
            let x: DuckFullIterator<Self::Output> =
                Box::new(Self::init_data_iterator(args.clone())?);
            Ok(x)
        }))
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

/// 动态表函数的 scan 状态：bind 阶段算出的 schema + 行迭代器。
///
/// The scan state of a dynamic table function: the schema computed during bind plus the row
/// iterator.
///
/// 两个字段都满足 `Send + 'static`（schema 走 [`DuckResultSchema`]，不含 `LogicalType` 句柄），
/// 因此可以直接作为 `with_bind_init` 的 `init` 返回的状态类型；迭代器不可克隆，所以不能走
/// `with_state`。
///
/// Both fields are `Send + 'static` (the schema is a [`DuckResultSchema`], holding no
/// `LogicalType` handle), so this works directly as the state type returned by `with_bind_init`'s
/// `init` closure; the iterator cannot be cloned, so `with_state` is not an option.
pub struct DuckDynamicState {
    /// 输出列定义（bind 阶段读外部元数据得出）。
    ///
    /// The output column definitions (derived during bind from external metadata).
    pub schema: DuckResultSchema,
    /// 行迭代器。
    ///
    /// The row iterator.
    pub rows: DuckDynamicIterator,
}

/// 把「参数结构体 -> 动态结果集」的 Rust 函数注册成输出 schema 在 bind 阶段才确定的表函数。
///
/// Registers a plain Rust function `args -> dynamic result table` as a table function whose output
/// schema is only decided during bind.
///
/// 与 [`TableFunctionAdapter`] 的关系：
///
/// - 静态版：输出列由 `Output: DuckColumns`（`#[derive(DuckStruct)]`）在编译期固定；
/// - 动态版（本 trait）：输出列由 [`Self::bind`] 在运行期返回的 [`DuckDynamicTable`] 给出 ——
///   列名与列类型都可以来自文件头、字典表、远端 schema 等外部信息。
///
/// 生命周期与静态版一致：bind 解析参数、调用 [`Self::bind`] 得到 `schema + 迭代器`，并按 schema
/// 调 `add_result_column_with_type` 声明结果列；每次执行再由 `init` 调一次 [`Self::bind`] 重建一份
/// 迭代器；scan 每批从迭代器取至多 `vector_size()` 行，按 schema 写进输出 chunk。各阶段都用
/// `catch_unwind` 包住，panic 会变成查询错误。
///
/// The life cycle matches the static flavour: bind parses the arguments, calls [`Self::bind`] to get
/// a `schema + iterator` pair, and declares the result columns through
/// `add_result_column_with_type`; for every execution `init` calls [`Self::bind`] once more to rebuild
/// the iterator; scan then pulls at most `vector_size()` rows per batch from the iterator and writes
/// them out according to the schema. Every phase is wrapped in `catch_unwind`, so a panic becomes a
/// query error.
///
/// 一般不需要手写 `table_function_builder()` / `bind_state` / `init_state` / `scan`，只实现
/// [`Self::bind`] 即可：
///
/// 这里**没有** `extra_info` 钩子，原因同 [`TableFunctionAdapter`]（typed builder 占用了该槽位）。
///
/// There is **no** `extra_info` hook here, for the same reason as [`TableFunctionAdapter`] (the
/// typed builder occupies that slot).
///
/// Usually only [`Self::bind`] has to be implemented; `table_function_builder()` / `bind_state` /
/// `init_state` / `scan` come with defaults:
///
/// ```ignore
/// struct MyDynamic;
///
/// impl duckfn::DynamicTableFunctionAdapter for MyDynamic {
///     const NAME: &'static str = "my_dynamic";
///     type Args = MyArgs;
///
///     fn bind(args: Self::Args) -> duckfn::DuckResult<duckfn::DuckDynamicTable> {
///         let schema = duckfn::DuckResultSchema::from_scalar_types([
///             ("id", duckfn::TypeId::BigInt),
///             ("name", duckfn::TypeId::Varchar),
///         ]);
///         let rows = (0..args.count).map(|i| {
///             Ok(Some(duckfn::DuckDynamicRow::new(vec![
///                 Some(duckfn::DuckDynamicValue::BigInt(i)),
///                 Some(duckfn::DuckDynamicValue::Varchar(format!("row_{i}"))),
///             ])))
///         });
///         Ok(duckfn::DuckDynamicTable::new(schema, Box::new(rows)))
///     }
/// }
/// ```
pub trait DynamicTableFunctionAdapter: Sized + 'static
where
    Self::Args: Clone + Send + Sync + 'static,
{
    /// 构造表函数 builder，并挂上 bind/init/scan 三个闭包（默认实现，通常不用改）。
    ///
    /// Builds the table-function builder and attaches the bind/init/scan closures (default
    /// implementation; usually left untouched).
    fn table_function_builder() -> DuckResult<TableFunctionBuilder> {
        let mut builder = TableFunctionBuilder::new(Self::NAME);
        builder = config_bind_params::<Self::Args>(builder);
        // 1. bind closure: declare the dynamic schema, read parameters, return the arguments.
        // 2. init closure: rebuild the per-execution scan state (schema + fresh row iterator).
        builder
            .with_bind_init(Self::bind_state, Self::init_state)
            // 3. scan closure: pull rows, write them by schema, set chunk size.
            .scan(Self::scan)
            .build()
    }

    /// 参数表登记（默认委托给 [`config_bind_params`]，需要特别顺序时再覆盖）。
    ///
    /// Parameter registration (delegates to [`config_bind_params`] by default; override only when a
    /// different order is needed).
    fn config_params(builder: TableFunctionBuilder) -> TableFunctionBuilder {
        config_bind_params::<Self::Args>(builder)
    }

    /// bind 阶段：解析参数、按 [`Self::bind`] 返回的动态结果集声明输出列，并把参数作为 bind data
    /// 返回。
    ///
    /// `Self::bind` 同时产出 schema 与迭代器；这里取 schema 声明结果列、丢弃迭代器（迭代器不可
    /// 克隆），scan 状态改由 [`Self::init_state`] 在每次执行时重建。
    ///
    /// Bind phase: parses the arguments, declares the result columns from the dynamic table returned
    /// by [`Self::bind`] and returns the arguments as the bind data. `Self::bind` yields both the
    /// schema and the iterator; the schema is used to declare the result columns and the iterator is
    /// dropped (it cannot be cloned), while the scan state is rebuilt by [`Self::init_state`] per
    /// execution.
    fn bind_state(bind: &BindInfo) -> DuckResult<Self::Args> {
        catch_unwind(|| {
            let args: Self::Args = Self::read_args(bind)?;
            let (schema, _rows) = Self::bind(args.clone())?.into_parts();
            schema.declare(bind);
            Ok(args)
        })
        .map_err(panic_to_duck_error)? // 不用flatten以兼容1.86
    }

    /// init 阶段：调 [`Self::bind`] 重建本次执行的动态结果集，拆成 scan 状态。
    ///
    /// 每次执行都会调用一次，所以 prepared statement 重复 `EXECUTE`、递归 CTE 里的重复扫描都从头
    /// 开始。
    ///
    /// Init phase: calls [`Self::bind`] again to rebuild this execution's dynamic result table and
    /// splits it into the scan state. It runs once per execution, so repeated `EXECUTE` of a prepared
    /// statement and re-scans inside a recursive CTE start from the beginning.
    fn init_state(args: &Self::Args) -> DuckResult<DuckDynamicState> {
        catch_unwind(AssertUnwindSafe(|| {
            let (schema, rows) = Self::bind(args.clone())?.into_parts();
            Ok(DuckDynamicState { schema, rows })
        }))
        .map_err(panic_to_duck_error)? // 不用flatten以兼容1.86
    }

    /// 从 bind 信息里解析参数，默认委托给 [`DuckBindArgs::read_bind_args`]。
    ///
    /// Parses the arguments from the bind info; by default it delegates to
    /// [`DuckBindArgs::read_bind_args`].
    fn read_args(bind: &BindInfo) -> DuckResult<Self::Args> {
        Self::Args::read_bind_args(bind)
    }

    /// scan 阶段：从迭代器取一批行，按 schema 写进 `chunk`，并设置本批实际行数。
    ///
    /// 迭代器耗尽（`None`）时提前结束并把 chunk 大小设为已写入的行数；用 `catch_unwind` 捕获
    /// panic 并转成查询错误。
    ///
    /// Scan phase: pulls one batch of rows from the iterator, writes them into `chunk` according to
    /// the schema and sets the actual batch size. When the iterator is exhausted (`None`) it stops
    /// early and sets the chunk size to the number of rows already written. Panics are caught and
    /// turned into query errors.
    fn scan(state: &mut DuckDynamicState, chunk: &DataChunk) -> DuckResult<()> {
        catch_unwind(AssertUnwindSafe(|| {
            let size = vector_size();
            let mut rows: Vec<Option<DuckDynamicRow>> = Vec::with_capacity(size as usize);

            let mut count = size;
            for i in 0..size {
                match state.rows.next() {
                    Some(value) => rows.push(value?),
                    None => {
                        count = i;
                        break;
                    }
                }
            }
            DuckDynamicRow::write_batch(chunk, &state.schema, &vec_option_to_ref(&rows))?;
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

    /// bind 阶段调用的真正业务逻辑：读外部元数据、算出动态 schema，并造出行迭代器。
    ///
    /// The actual business logic invoked during bind: read the external metadata, work out the
    /// dynamic schema and build the row iterator.
    fn bind(args: Self::Args) -> DuckResult<DuckDynamicTable>;
}
