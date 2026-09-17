//! 标量函数适配层：把 quack-rs 的向量级 C 回调拆成「逐行」的 Rust 代码。
//!
//! Scalar-function adapter: splits quack-rs' vector-level C callback into per-row Rust code.

use crate::duck_columns::DuckColumns;
use crate::utils::builder_with_params::BuilderWithParams;
use crate::value_types::duck_value_type::{DuckValueReader, DuckValueType};
use crate::{
    DuckExtraInfo, DuckOptionResult, DuckResult, duck_scalar_unwind, erased_extra_info,
    raw_extra_info, vec_option_to_ref,
};
use libduckdb_sys::{duckdb_connection, duckdb_data_chunk, duckdb_function_info, duckdb_vector};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{
    LogicalType, NullHandling, ScalarFunctionBuilder, ScalarFunctionInfo, ScalarOverloadBuilder,
};

/// 把「参数结构体 -> 输出值」的纯 Rust 函数注册成 DuckDB 标量函数。
///
/// 适配层把 quack-rs 的向量级回调拆成逐行的 Rust 代码：
///
/// - 用 [`Self::Args`]（实现 [`DuckColumns`]）从输入 `DataChunk` 逐行取参；
/// - 每行调用 [`Self::apply`]（或 NULL 时代替的 [`Self::apply_with_null`]，以及带附加数据的
///   [`Self::apply_with_extra`]）；
/// - 启用可变参数（[`Self::varargs_element_type`] 返回 `Some`）时改走
///   [`Self::apply_varargs`]，固定参数之后的列全部按该类型读成一个集合；
/// - 收集成 `Vec<Option<Self::Output>>` 后一次性写入输出向量。
///
/// 一般不用手写这个 impl，直接用 `#[duck_scalar_function]` 作用在普通函数上即可。
///
/// Registers a plain Rust function `Args -> Output` as a DuckDB scalar function. The adapter
/// splits quack-rs' vector-level callback into per-row Rust code: it reads arguments row by
/// row through [`Self::Args`] (a [`DuckColumns`] implementation), calls [`Self::apply`] (or
/// [`Self::apply_with_null`] for NULL rows, or [`Self::apply_with_extra`] when extra data is
/// attached), or [`Self::apply_varargs`] once variadic arguments are enabled
/// ([`Self::varargs_element_type`] returns `Some`), where every column past the fixed ones is
/// read as one element of the variadic element type. It finally writes the collected results into
/// the output vector in one batch. Usually you do not implement this manually: just annotate a
/// plain function with `#[duck_scalar_function]`.
pub trait ScalarFunctionAdapter: Sized + 'static {
    /// # Safety
    ///
    /// 由 DuckDB 回调，`info`/`input`/`output` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `info`, `input` and `output` are guaranteed valid by DuckDB.
    unsafe extern "C" fn scalar_function_wrapper(
        info: duckdb_function_info,
        input: duckdb_data_chunk,
        output: duckdb_vector,
    ) {
        let info: ScalarFunctionInfo = unsafe { ScalarFunctionInfo::new(info) };
        // SAFETY: 回调期间 info 有效；extra_info 由 builder 在注册时挂上（没挂时为 None）。
        //
        // SAFETY: `info` is valid during the callback; the extra info was attached by the builder at
        // registration time (None when nothing was attached).
        let extra = unsafe { erased_extra_info(&info) };
        duck_scalar_unwind(&info,|| {
            let chunk: DataChunk = unsafe { DataChunk::from_raw(input) };
            let mut readers = Self::Args::create_column_readers(&chunk);
            // 固定参数列数：可变参数（`varargs_element_type()` 为 `Some`）从这一列之后开始。
            //
            // Number of fixed-argument columns; variadic arguments (when `varargs_element_type()`
            // is `Some`) start right after them.
            let fixed_count = readers.len();
            let has_varargs = Self::varargs_element_type().is_some();
            if has_varargs {
                for column_index in fixed_count..chunk.column_count() {
                    readers.push(Self::varargs_create_reader(&chunk, column_index));
                }
            }
            let row_count = chunk.size();

            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(row_count);
            for row in 0..row_count {
                let result = if has_varargs {
                    Self::apply_varargs(&readers, row, fixed_count)
                } else {
                    Self::apply_with_extra(Self::Args::read_columns(&readers, row), extra)
                };
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

    /// 是否把函数标记为 volatile，默认 `false`。
    ///
    /// 返回 `true` 时注册期会调用 DuckDB 的 `duckdb_scalar_function_set_volatile`：DuckDB
    /// 不会缓存或复用相同参数的调用结果，每一行都会重新求值（`random()` 这类函数需要它）。
    /// 不开启时 DuckDB 可能把常量参数的调用折叠成只执行一次。
    ///
    /// 该开关走 quack-rs 的 `ScalarFunctionBuilder::volatile`，只有在 duckfn 打开
    /// `duckdb-1-5` feature（DuckDB 1.5.0+ 的 C API）时才真正生效；未开启时本方法被忽略。
    /// 函数集重载（[`Self::scalar_overload_builder`]）不支持该开关。
    ///
    /// Whether to mark the function volatile; defaults to `false`. Returning `true` makes the
    /// registration call DuckDB's `duckdb_scalar_function_set_volatile`, so DuckDB neither caches
    /// nor reuses the result of a call with the same arguments — every row is re-evaluated, which
    /// is what functions like `random()` need. Without it DuckDB may fold constant-argument calls
    /// into a single execution. The switch goes through quack-rs' `ScalarFunctionBuilder::volatile`
    /// and only takes effect when duckfn's `duckdb-1-5` feature (the DuckDB 1.5.0+ C API) is
    /// enabled; otherwise it is ignored. Function-set overloads ([`Self::scalar_overload_builder`])
    /// do not support it.
    fn volatile() -> bool {
        false
    }

    /// 可变参数的元素逻辑类型；默认 `None`（函数没有可变参数）。
    ///
    /// 返回 `Some(lt)` 表示函数带可变参数：注册期会调用 DuckDB 的
    /// `duckdb_scalar_function_set_varargs`（也就是 quack-rs 的
    /// `ScalarFunctionBuilder::varargs_logical`），调用期除去固定参数之外的每一列都按 `lt`
    /// 读成一个元素，交给 [`Self::apply_varargs`]。
    ///
    /// 元素可以是任意 [`DuckValueType`]，包括 `Option<T>`（元素可为 NULL）与 `Vec<T>`
    /// （即「可变参数本身是 LIST」，对应 `varargs_logical(LogicalType::list(...))`）。
    ///
    /// 该能力只在 duckfn 打开 `duckdb-1-5` feature（DuckDB 1.5.0+ 的 C API）时真正生效；
    /// 函数集重载（[`Self::scalar_overload_builder`]）不支持它。宏 `#[duck_scalar_function(varargs = true)]`
    /// 会从函数签名最后一个参数 `Vec<T>` 推断出 `T`。
    ///
    /// Variadic-argument element logical type; `None` (the default) means the function has no
    /// variadic arguments. `Some(lt)` makes registration call DuckDB's
    /// `duckdb_scalar_function_set_varargs` (quack-rs' `ScalarFunctionBuilder::varargs_logical`)
    /// and, at call time, reads every column after the fixed ones as one element of type `lt`
    /// handed to [`Self::apply_varargs`]. The element may be any [`DuckValueType`], `Option<T>`
    /// (nullable element) and `Vec<T>` (i.e. the variadic argument is itself a LIST, matching
    /// `varargs_logical(LogicalType::list(...))`) included. The capability only takes effect with
    /// duckfn's `duckdb-1-5` feature (the DuckDB 1.5.0+ C API) and is not supported for function-set
    /// overloads ([`Self::scalar_overload_builder`]). The macro
    /// `#[duck_scalar_function(varargs = true)]` infers the element type `T` from the last
    /// parameter `Vec<T>` of the signature.
    fn varargs_element_type() -> Option<LogicalType> {
        None
    }

    /// 为可变参数的第 `column_index` 列创建读取器。
    ///
    /// 只在 [`Self::varargs_element_type`] 返回 `Some` 时调用；默认实现直接 panic，因此手写
    /// impl 不需要实现它。
    ///
    /// Creates the reader for the `column_index`-th variadic column. Only called when
    /// [`Self::varargs_element_type`] returns `Some`; the default panics, so hand-written
    /// implementations do not need it.
    fn varargs_create_reader(_chunk: &DataChunk, _column_index: usize) -> DuckValueReader {
        unreachable!("varargs_create_reader is only used when varargs_element_type() returns Some")
    }

    /// 带可变参数时的一行求值。
    ///
    /// `readers` 覆盖固定参数与可变参数的全部列，`fixed_count` 是固定参数列数；实现方用
    /// `readers[..fixed_count]` 读固定参数、其余读成一个可变参数集合。`Ok(None)` 表示该行整体
    /// 输出 SQL NULL（任一非可空参数或元素为 NULL 时就应该这样短路）。
    ///
    /// 只在 [`Self::varargs_element_type`] 返回 `Some` 时调用；默认实现直接 panic，因此手写
    /// impl 不需要实现它。
    ///
    /// Evaluates one row when variadic arguments are enabled. `readers` covers both the fixed and
    /// the variadic columns and `fixed_count` is the number of fixed ones; the implementation reads
    /// the fixed arguments from `readers[..fixed_count]` and the rest as one variadic collection.
    /// `Ok(None)` makes the whole row SQL NULL — the right short-circuit when any non-nullable
    /// argument or element is NULL. Only called when [`Self::varargs_element_type`] returns `Some`;
    /// the default panics, so hand-written implementations do not need it.
    fn apply_varargs(
        _readers: &[DuckValueReader],
        _row: usize,
        _fixed_count: usize,
    ) -> DuckOptionResult<Self::Output> {
        unreachable!("apply_varargs is only used when varargs_element_type() returns Some")
    }

    /// 构造「独立函数」用的 builder（自带函数名）。
    ///
    /// Builds the builder for a standalone function (carrying its own name).
    fn scalar_function_builder() -> ScalarFunctionBuilder {
        let mut builder = ScalarFunctionBuilder::new(Self::NAME)
            .function(Self::scalar_function_wrapper)
            .null_handling(Self::null_handling())
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types());
        if Self::volatile() {
            builder = set_volatile(builder);
        }
        if let Some(varargs_type) = Self::varargs_element_type() {
            builder = set_varargs(builder, varargs_type);
        }
        if let Some((ptr, destroy)) = raw_extra_info(Self::extra_info()) {
            // SAFETY: ptr 由 `DuckExtraInfo::into_raw` 产生，destroy 与它配对；函数对象交给 DuckDB 后
            // 由 DuckDB 在销毁时调用 destroy。
            //
            // SAFETY: `ptr` comes from `DuckExtraInfo::into_raw` and `destroy` matches it; once the
            // function object is handed to DuckDB, DuckDB calls `destroy` on destruction.
            builder = unsafe { builder.extra_info(ptr, destroy) };
        }
        builder
    }

    /// 构造「函数集重载」用的 builder（不带函数名，由函数集决定）。
    ///
    /// 注意：quack-rs 的 [`ScalarOverloadBuilder`] 没有暴露 volatile / varargs 开关，因此
    /// [`Self::volatile`] 与 [`Self::varargs_element_type`] 对重载无效；需要它们时请注册成独立函数。
    ///
    /// Builds the builder for a function-set overload (no name; the set provides it). Note that
    /// quack-rs' [`ScalarOverloadBuilder`] exposes neither a volatile nor a varargs switch, so
    /// [`Self::volatile`] and [`Self::varargs_element_type`] have no effect on overloads; register
    /// the function standalone when they are required.
    fn scalar_overload_builder() -> ScalarOverloadBuilder {
        let mut builder = ScalarOverloadBuilder::new()
            .function(Self::scalar_function_wrapper)
            .null_handling(Self::null_handling())
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types());
        if let Some((ptr, destroy)) = raw_extra_info(Self::extra_info()) {
            // SAFETY: 同上；重载句柄最终由函数集持有，析构时机由 DuckDB 决定。
            //
            // SAFETY: as above; the overload handle ends up owned by the function set and DuckDB
            // decides when it is destroyed.
            builder = unsafe { builder.extra_info(ptr, destroy) };
        }
        builder
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

    /// 注册期附加的数据（DuckDB 的 `extra_info`）；默认不附加。
    ///
    /// 数据在函数对象销毁时由 DuckDB 调用析构回调释放，因此类型必须是 `Send + Sync + 'static`
    /// （函数对象可能被多线程、多查询共享，且应视为只读）。需要「每次查询一份」的状态请改用表函数的
    /// `with_state` 或 quack-rs 的 bind data。
    ///
    /// Function-level data attached at registration time (DuckDB's `extra_info`); nothing is
    /// attached by default. DuckDB frees it through the destructor when the function object is
    /// dropped, so the type must be `Send + Sync + 'static` (the function object may be shared
    /// across threads and queries, and must be treated as read-only). For per-query state use a
    /// table function's `with_state` or quack-rs' bind data instead.
    fn extra_info() -> Option<DuckExtraInfo> {
        None
    }

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

    /// 对一行参数求值，并带上函数级附加数据。
    ///
    /// 默认忽略 `extra` 并转调 [`Self::apply_with_null`]；需要读 `extra_info` 时重写本方法：
    ///
    /// ```ignore
    /// fn apply_with_extra(
    ///     args: Option<Self::Args>,
    ///     extra: Option<&duckfn::DuckExtraInfo>,
    /// ) -> duckfn::DuckOptionResult<Self::Output> {
    ///     let config = extra.and_then(|extra| extra.downcast_ref::<MyConfig>());
    ///     // ...
    /// }
    /// ```
    ///
    /// Evaluates one row of arguments together with the function-level extra data. By default it
    /// ignores `extra` and delegates to [`Self::apply_with_null`]; override it to read the
    /// `extra_info` (see the snippet above).
    fn apply_with_extra(
        args: Option<Self::Args>,
        extra: Option<&DuckExtraInfo>,
    ) -> DuckOptionResult<Self::Output> {
        let _ = extra;
        Self::apply_with_null(args)
    }

    /// 对一行非 NULL 参数求值；返回 `Ok(None)` 表示该行输出 SQL NULL。
    ///
    /// 启用可变参数（[`Self::varargs_element_type`] 返回 `Some`）时不会调用本方法，适配层改走
    /// [`Self::apply_varargs`]；`#[duck_scalar_function(varargs = true)]` 生成的实现因此只放一个
    /// 占位方法体。
    ///
    /// Evaluates one row of non-NULL arguments; returning `Ok(None)` makes this row SQL NULL. It
    /// is not called once variadic arguments are enabled ([`Self::varargs_element_type`] returns
    /// `Some`), where the adapter goes through [`Self::apply_varargs`] instead; the implementation
    /// generated by `#[duck_scalar_function(varargs = true)]` therefore only carries a placeholder
    /// body.
    fn apply(args: Self::Args) -> DuckOptionResult<Self::Output>;
}

/// 把标量函数标记为 volatile（[`ScalarFunctionAdapter::volatile`] 的实现细节）。
///
/// `ScalarFunctionBuilder::volatile` 只在 quack-rs 的 `duckdb-1-5` feature 下存在，因此没有该
/// feature 时这里原样返回 builder：开关被忽略，而不是让整个扩展编译失败。
///
/// Marks a scalar function volatile (the implementation detail behind
/// [`ScalarFunctionAdapter::volatile`]). `ScalarFunctionBuilder::volatile` only exists under
/// quack-rs' `duckdb-1-5` feature, so without it the builder is returned unchanged: the switch is
/// ignored rather than failing the whole extension build.
fn set_volatile(builder: ScalarFunctionBuilder) -> ScalarFunctionBuilder {
    #[cfg(feature = "duckdb-1-5")]
    {
        builder.volatile()
    }
    #[cfg(not(feature = "duckdb-1-5"))]
    {
        builder
    }
}

/// 给标量函数设置可变参数类型（[`ScalarFunctionAdapter::varargs_element_type`] 的实现细节）。
///
/// `ScalarFunctionBuilder::varargs_logical` 只在 quack-rs 的 `duckdb-1-5` feature 下存在，因此
/// 没有该 feature 时这里原样返回 builder：可变参数被忽略，而不是让整个扩展编译失败。
///
/// Sets the variadic-argument type (the implementation detail behind
/// [`ScalarFunctionAdapter::varargs_element_type`]). `ScalarFunctionBuilder::varargs_logical`
/// only exists under quack-rs' `duckdb-1-5` feature, so without it the builder is returned
/// unchanged: varargs are ignored rather than failing the whole extension build.
fn set_varargs(builder: ScalarFunctionBuilder, varargs_type: LogicalType) -> ScalarFunctionBuilder {
    #[cfg(feature = "duckdb-1-5")]
    {
        builder.varargs_logical(varargs_type)
    }
    #[cfg(not(feature = "duckdb-1-5"))]
    {
        let _ = varargs_type;
        builder
    }
}
