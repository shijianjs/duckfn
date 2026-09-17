//! 聚合函数适配层：把有状态的 Rust 结构体注册成 DuckDB 聚合函数。
//!
//! Aggregate-function adapter: registers a stateful Rust struct as a DuckDB aggregate function.

use crate::duck_columns::DuckColumns;
use crate::utils::builder_with_params::BuilderWithParams;
use crate::value_types::duck_value_type::DuckValueType;
use crate::{
    DuckExtraInfo, duck_aggregate_unwind, duck_error, erased_extra_info, raw_extra_info,
    vec_option_to_ref, DuckOptionResult, DuckResult,
};
use libduckdb_sys::{
    DuckDBSuccess, duckdb_add_aggregate_function_to_set, duckdb_aggregate_function,
    duckdb_aggregate_function_add_parameter, duckdb_aggregate_function_set,
    duckdb_aggregate_function_set_destructor, duckdb_aggregate_function_set_extra_info,
    duckdb_aggregate_function_set_functions, duckdb_aggregate_function_set_name,
    duckdb_aggregate_function_set_return_type, duckdb_aggregate_function_set_special_handling,
    duckdb_aggregate_state, duckdb_connection, duckdb_create_aggregate_function,
    duckdb_create_aggregate_function_set, duckdb_data_chunk, duckdb_destroy_aggregate_function,
    duckdb_destroy_aggregate_function_set, duckdb_function_info,
    duckdb_register_aggregate_function_set, duckdb_vector, idx_t,
};
use quack_rs::aggregate::builder::OverloadBuilder;
use quack_rs::aggregate::{AggregateFunctionBuilder, AggregateState, FfiState};
use quack_rs::data_chunk::DataChunk;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{AggregateFunctionInfo, NullHandling};
use std::ffi::CString;

/// 把「参数结构体 -> 聚合状态」的有状态 Rust 类型注册成 DuckDB 聚合函数。
///
/// 适配层把 DuckDB 的聚合回调逐一接到 Rust 方法上：
///
/// - `state_size` / `init` / `destroy`：聚合状态的分配、初始化与释放（由 quack-rs 的
///   [`FfiState`] 处理）；
/// - `update`（[`Self::handle_row`]）：对每一行输入累加状态；
/// - `combine`（[`Self::combine`]）：并行执行时合并两个状态；
/// - `finalize`（[`Self::result`]）：输出最终结果。
///
/// 一般不用手写这个 impl，直接用 `#[duck_aggregate_function]` 作用在普通函数上即可
/// （函数里带一个 `&mut XxxState` 参数用来累积）。
///
/// Registers a stateful Rust type `Args -> aggregate state` as a DuckDB aggregate function.
/// The adapter wires each DuckDB aggregate callback to a Rust method: `state_size` / `init` /
/// `destroy` allocate, initialise and free the state (handled by quack-rs' [`FfiState`]);
/// `update` ([`Self::handle_row`]) accumulates one input row; `combine` ([`Self::combine`])
/// merges two states during parallel execution; and `finalize` ([`Self::result`]) emits the
/// final value. Usually you do not implement this manually: annotate an ordinary function
/// taking a `&mut XxxState` parameter with `#[duck_aggregate_function]`.
pub trait AggregateFunctionAdapter: AggregateState + Sized + 'static {
    /// # Safety
    ///
    /// 由 DuckDB 回调，`_info` 由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `_info` is guaranteed valid by DuckDB.
    unsafe extern "C" fn c_state_size(_info: duckdb_function_info) -> idx_t {
        unsafe { FfiState::<Self>::size_callback(_info) }
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`info`/`state` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `info` and `state` are guaranteed valid by DuckDB.
    unsafe extern "C" fn c_state_init(info: duckdb_function_info, state: duckdb_aggregate_state) {
        unsafe { FfiState::<Self>::init_callback(info, state) };
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`info`/`input`/`states` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `info`, `input` and `states` are guaranteed valid by DuckDB.
    unsafe extern "C" fn c_update(
        info: duckdb_function_info,
        input: duckdb_data_chunk,
        states: *mut duckdb_aggregate_state,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(info) };
        // SAFETY: 回调期间 info 有效；extra_info 由 builder 在注册时挂上（没挂时为 None）。
        //
        // SAFETY: `info` is valid during the callback; the extra info was attached by the builder
        // at registration time (None when nothing was attached).
        let extra = unsafe { erased_extra_info(&info) };
        duck_aggregate_unwind(&info, || {
            let chunk = unsafe { DataChunk::from_raw(input) };
            let readers = Self::Args::create_column_readers(&chunk);
            let row_count = chunk.size();
            for row in 0..row_count {
                let args = Self::Args::read_columns(&readers, row);
                let state_ptr = unsafe { *states.add(row) };
                if let Some(st) = unsafe { FfiState::<Self>::with_state_mut(state_ptr) } {
                    let result = st.handle_row_with_extra(args, extra);
                    if let Err(e) = result {
                        info.set_error(e.as_str());
                        return;
                    }
                }
            }
        });
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`source`/`target` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `source` and `target` are guaranteed valid by DuckDB.
    unsafe extern "C" fn c_combine(
        info: duckdb_function_info,
        source: *mut duckdb_aggregate_state,
        target: *mut duckdb_aggregate_state,
        count: idx_t,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(info) };
        // SAFETY: 回调期间 info 有效；extra_info 由 builder 在注册时挂上（没挂时为 None）。
        //
        // SAFETY: `info` is valid during the callback; the extra info was attached by the builder
        // at registration time (None when nothing was attached).
        let extra = unsafe { erased_extra_info(&info) };
        duck_aggregate_unwind(&info, || {
            for i in 0..count as usize {
                let src_ptr = unsafe { *source.add(i) };
                let tgt_ptr = unsafe { *target.add(i) };
                let src = unsafe { FfiState::<Self>::with_state(src_ptr) };
                let tgt = unsafe { FfiState::<Self>::with_state_mut(tgt_ptr) };
                if let (Some(s), Some(t)) = (src, tgt) {
                    let result = t.combine_with_extra(s, extra);
                    if let Err(e) = result {
                        info.set_error(e.as_str());
                        return;
                    }
                }
            }
        });
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`info`/`source`/`result` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `info`, `source` and `result` are guaranteed valid by DuckDB.
    unsafe extern "C" fn c_finalize(
        info: duckdb_function_info,
        source: *mut duckdb_aggregate_state,
        result: duckdb_vector,
        count: idx_t,
        offset: idx_t,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(info) };
        // SAFETY: 回调期间 info 有效；extra_info 由 builder 在注册时挂上（没挂时为 None）。
        //
        // SAFETY: `info` is valid during the callback; the extra info was attached by the builder
        // at registration time (None when nothing was attached).
        let extra = unsafe { erased_extra_info(&info) };
        duck_aggregate_unwind(&info, || {
            if offset != 0 {
                info.set_error(
                    format!(
                        "non-zero aggregate finalize result offset is not supported: {}",
                        offset
                    )
                    .as_str(),
                );
                return;
            }

            let mut output_vec: Vec<Option<Self::Output>> = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                let state_ptr = unsafe { *source.add(i) };
                match unsafe { FfiState::<Self>::with_state(state_ptr) } {
                    Some(st) => {
                        let result1 = st.result_with_extra(extra);
                        match result1 {
                            Ok(r) => output_vec.push(r),
                            Err(e) => {
                                info.set_error(e.as_str());
                                return;
                            }
                        };
                    }
                    None => output_vec.push(None),
                }
            }
            Self::Output::write_batch(result, &vec_option_to_ref(&output_vec));
        });
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`states` 由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `states` is guaranteed valid by DuckDB.
    unsafe extern "C" fn c_state_destroy(states: *mut duckdb_aggregate_state, count: idx_t) {
        unsafe { FfiState::<Self>::destroy_callback(states, count) };
    }

    /// NULL 处理策略，默认 [`NullHandling::DefaultNullHandling`]（NULL 行不进入回调）。
    ///
    /// Null-handling strategy; defaults to [`NullHandling::DefaultNullHandling`] (NULL rows
    /// never reach the callback).
    fn null_handling() -> NullHandling {
        NullHandling::DefaultNullHandling
    }

    /// 注册期附加的数据（DuckDB 的 `extra_info`）；默认不附加。
    ///
    /// 数据在函数对象销毁时由 DuckDB 调用析构回调释放，因此类型必须是 `Send + Sync + 'static`
    /// （函数对象可能被多线程、多查询共享，且应视为只读）。
    ///
    /// 生效范围：`aggregate_function_builder()`（独立聚合函数）与 duckfn 自建的聚合函数集
    /// （`aggregate_function_guard()`）；quack-rs 的 `OverloadBuilder` 没有该接口，走那条路时本方法无效。
    ///
    /// Function-level data attached at registration time (DuckDB's `extra_info`); nothing is
    /// attached by default. DuckDB frees it through the destructor when the function object is
    /// dropped, so the type must be `Send + Sync + 'static` (the function object may be shared
    /// across threads and queries, and must be treated as read-only). It applies to
    /// `aggregate_function_builder()` (standalone aggregates) and duckfn's own aggregate function
    /// set (`aggregate_function_guard()`); quack-rs' `OverloadBuilder` has no such interface, so this
    /// method has no effect on that path.
    fn extra_info() -> Option<DuckExtraInfo> {
        None
    }

    /// 构造「独立函数」用的 aggregate builder（自带函数名）。
    ///
    /// Builds the aggregate builder for a standalone function (carrying its own name).
    fn aggregate_function_builder() -> AggregateFunctionBuilder {
        let mut builder = AggregateFunctionBuilder::new(Self::NAME)
            .state_size(Self::c_state_size)
            .init(Self::c_state_init)
            .update(Self::c_update)
            .combine(Self::c_combine)
            .finalize(Self::c_finalize)
            .destructor(Self::c_state_destroy)
            .null_handling(Self::null_handling())
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types());
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

    /// 往「函数集重载」用的 [`OverloadBuilder`] 上补齐本签名的回调与参数表。
    ///
    /// Complements the given [`OverloadBuilder`] (used for function-set overloads) with this
    /// signature's callbacks and parameter list.
    ///
    /// 注意：quack-rs 的 `OverloadBuilder` **没有** `extra_info` 接口，因此 [`Self::extra_info`] 在这条
    /// 路径上不会生效（要用附加数据请走 duckfn 自建的函数集，或 `aggregate_function_builder`）。
    ///
    /// Note: quack-rs' `OverloadBuilder` has **no** `extra_info` interface, so [`Self::extra_info`] is
    /// ignored on this path (use duckfn's own function set, or `aggregate_function_builder`, if you
    /// need attached data).
    fn aggregate_overload_builder(builder: OverloadBuilder) -> OverloadBuilder {
        builder
            .state_size(Self::c_state_size)
            .init(Self::c_state_init)
            .update(Self::c_update)
            .combine(Self::c_combine)
            .finalize(Self::c_finalize)
            .destructor(Self::c_state_destroy)
            .null_handling(Self::null_handling())
            .with_params(Self::Args::column_types())
    }

    /// # Safety
    ///
    /// `con` 必须是由 DuckDB 提供的有效连接句柄。
    ///
    /// `con` must be a valid connection handle provided by DuckDB.
    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        unsafe { Self::aggregate_function_builder().register(con) }
    }

    /// 直接用 DuckDB C API 创建一个聚合函数句柄（不走 quack-rs 的 builder）。
    ///
    /// 逐个设置函数名、参数类型、返回类型、全部回调与析构函数，并在开启
    /// `SpecialNullHandling` 时额外调用 `duckdb_aggregate_function_set_special_handling`。
    /// 返回的句柄由 [`AggregateFunctionGuard`] 在 drop 时释放。
    ///
    /// Creates an aggregate-function handle directly through the DuckDB C API (bypassing
    /// quack-rs' builder): it sets the name, parameter types, return type, every callback and
    /// the destructor, and additionally calls `duckdb_aggregate_function_set_special_handling`
    /// when `SpecialNullHandling` is enabled. The returned handle is freed on drop by
    /// [`AggregateFunctionGuard`].
    fn create_aggregate_function_guard(name: &CString) -> AggregateFunctionGuard {
        let func = unsafe { duckdb_create_aggregate_function() };
        unsafe { duckdb_aggregate_function_set_name(func, name.as_ptr()) };
        for lt in Self::Args::column_types() {
            unsafe { duckdb_aggregate_function_add_parameter(func, lt.as_raw()) };
        }
        unsafe {
            duckdb_aggregate_function_set_return_type(func, Self::Output::logical_type().as_raw())
        };
        unsafe {
            duckdb_aggregate_function_set_functions(
                func,
                Some(Self::c_state_size),
                Some(Self::c_state_init),
                Some(Self::c_update),
                Some(Self::c_combine),
                Some(Self::c_finalize),
            )
        };

        unsafe { duckdb_aggregate_function_set_destructor(func, Some(Self::c_state_destroy)) };

        if Self::null_handling() == NullHandling::SpecialNullHandling {
            unsafe { duckdb_aggregate_function_set_special_handling(func) };
        }

        if let Some((ptr, destroy)) = raw_extra_info(Self::extra_info()) {
            // SAFETY: ptr 由 `DuckExtraInfo::into_raw` 产生，destroy 与它配对；func 是刚创建的有效句柄，
            // 交给 DuckDB 注册后由 DuckDB 在销毁句柄时调用 destroy。
            //
            // SAFETY: `ptr` comes from `DuckExtraInfo::into_raw` and `destroy` matches it; `func` is a
            // freshly created valid handle, and DuckDB calls `destroy` when it destroys the handle.
            unsafe { duckdb_aggregate_function_set_extra_info(func, ptr, destroy) };
        }

        AggregateFunctionGuard {
            name: Self::NAME.to_string(),
            c_agg: func,
        }
    }

    /// 为该签名创建一个可挂进函数集（DuckfnAggregateFunctionSetBuilder）的聚合函数句柄。
    ///
    /// 返回类型取自 `Self::Output`，每个重载各自设置，因此同一函数集里
    /// 的不同重载可以有不同的返回类型（quack_rs 的 AggregateFunctionSetBuilder
    /// 只能在函数集层面设一个统一的返回类型）。
    ///
    /// Creates an aggregate-function handle for this signature that can be attached to a
    /// function set ([`DuckfnAggregateFunctionSetBuilder`]). The return type comes from
    /// `Self::Output` and is set per overload, so different overloads in the same set may have
    /// different return types (quack_rs' builder can only set one return type per set).
    fn aggregate_function_guard() -> AggregateFunctionGuard {
        let name = CString::new(Self::NAME).expect("function name must not contain null bytes");
        Self::create_aggregate_function_guard(&name)
    }

    /// 回调名称，仅用于标识（SQL 里的函数名由 builder 决定）。
    ///
    /// Callback name, used for identification only (the SQL name comes from the builder).
    const NAME: &'static str;

    /// 参数结构体类型：实现 [`DuckColumns`]，负责按行读取各参数列
    /// （`cargo add tuple-transpose` 可快速处理多个 `Option` 参数）。
    ///
    /// The argument struct type: implements [`DuckColumns`] and reads each argument column row
    /// by row (`cargo add tuple-transpose` helps with many `Option` parameters).
    type Args: DuckColumns;
    /// 输出值类型：决定聚合结果的 DuckDB 逻辑类型。
    ///
    /// The output value type: determines the logical type of the aggregate result.
    type Output: DuckValueType;

    /// NULL 行的默认处理：入参为 `None` 时跳过该行（不更新状态）。
    ///
    /// Default handling of NULL rows: when the arguments are `None` the row is skipped and the
    /// state is not updated.
    fn handle_row_with_null(&mut self, args: Option<Self::Args>) -> DuckResult<()> {
        if let Some(args) = args {
            self.handle_row(args)?;
        }
        Ok(())
    }

    /// 用一行（可能为 NULL）输入更新状态，并带上函数级附加数据。
    ///
    /// 默认忽略 `extra` 并转调 [`Self::handle_row_with_null`]；需要读 `extra_info` 时重写本方法：
    ///
    /// ```ignore
    /// fn handle_row_with_extra(
    ///     &mut self,
    ///     args: Option<Self::Args>,
    ///     extra: Option<&duckfn::DuckExtraInfo>,
    /// ) -> duckfn::DuckResult<()> {
    ///     let config = extra.and_then(|extra| extra.downcast_ref::<MyConfig>());
    ///     // ...
    /// }
    /// ```
    ///
    /// Updates the state with one (possibly NULL) input row together with the function-level extra
    /// data. By default it ignores `extra` and delegates to [`Self::handle_row_with_null`]; override
    /// it to read the `extra_info`.
    fn handle_row_with_extra(
        &mut self,
        args: Option<Self::Args>,
        extra: Option<&DuckExtraInfo>,
    ) -> DuckResult<()> {
        let _ = extra;
        self.handle_row_with_null(args)
    }

    /// 用一行非 NULL 输入更新状态。
    ///
    /// Updates the state with one row of non-NULL input.
    fn handle_row(&mut self, args: Self::Args) -> DuckResult<()>;
    /// 合并另一个状态到自身（并行聚合时使用）。
    ///
    /// Merges another state into this one (used for parallel aggregation).
    fn combine(&mut self, other: &Self) -> DuckResult<()>;
    /// 输出当前聚合结果；`Ok(None)` 表示结果为空（SQL NULL）。
    ///
    /// Emits the current aggregate result; `Ok(None)` means an empty (SQL NULL) result.
    fn result(&self) -> DuckOptionResult<Self::Output>;

    /// 合并另一个状态到自身，并带上函数级附加数据。
    ///
    /// 默认忽略 `extra` 并转调 [`Self::combine`]；需要读 `extra_info` 时重写本方法。
    ///
    /// Merges another state into this one together with the function-level extra data. By default it
    /// ignores `extra` and delegates to [`Self::combine`]; override it to read the `extra_info`.
    fn combine_with_extra(
        &mut self,
        other: &Self,
        extra: Option<&DuckExtraInfo>,
    ) -> DuckResult<()> {
        let _ = extra;
        self.combine(other)
    }

    /// 输出当前聚合结果，并带上函数级附加数据。
    ///
    /// 默认忽略 `extra` 并转调 [`Self::result`]；需要读 `extra_info` 时重写本方法。
    ///
    /// Emits the current aggregate result together with the function-level extra data. By default it
    /// ignores `extra` and delegates to [`Self::result`]; override it to read the `extra_info`.
    fn result_with_extra(&self, extra: Option<&DuckExtraInfo>) -> DuckOptionResult<Self::Output> {
        let _ = extra;
        self.result()
    }
}

/// 聚合状态的简化接口：用「自合并 + 自身求值」描述一个聚合函数。
///
/// 相比 [`AggregateFunctionAdapter`]，这里只需实现 [`Self::simple_combine`] 与
/// [`Self::simple_result`]，适合「合并就是把另一个状态并进来」和「结果就是自身某个值」
/// 这类简单聚合。
///
/// A simplified aggregate-state interface: describes an aggregate with "merge into self" plus
/// "evaluate self". Compared with [`AggregateFunctionAdapter`] you only need
/// [`Self::simple_combine`] and [`Self::simple_result`], which suits simple aggregates whose
/// merge is just folding the other state in and whose result is one of the state's values.
pub trait DuckAggregateState {
    /// 聚合的输出类型。
    ///
    /// The aggregate's output type.
    type Output: DuckValueType;
    /// 合并另一个状态，默认调用 [`Self::simple_combine`]。
    ///
    /// Merges another state; by default it delegates to [`Self::simple_combine`].
    fn combine(&mut self, other: &Self) -> DuckResult<()> {
        self.simple_combine(other);
        Ok(())
    }
    /// 把 `other` 的状态并入自身（默认实现是 `todo!()`，需要时重写）。
    ///
    /// Folds `other`'s state into `self` (the default implementation is a `todo!()` and must
    /// be overridden when needed).
    fn simple_combine(&mut self, _other: &Self) {
        todo!("simple_combine is not implemented")
    }
    /// 计算最终结果，默认包装 [`Self::simple_result`] 为 `Some`。
    ///
    /// Computes the final result; by default it wraps [`Self::simple_result`] in `Some`.
    fn result(&self) -> DuckOptionResult<Self::Output> {
        Ok(Some(self.simple_result()))
    }
    /// 由自身状态直接算出输出值（默认实现是 `todo!()`，需要时重写）。
    ///
    /// Derives the output value from the state directly (the default implementation is a
    /// `todo!()` and must be overridden when needed).
    fn simple_result(&self) -> Self::Output {
        todo!("simple_result is not implemented")
    }
}

/// 聚合函数句柄的 RAII 包装：持有 `duckdb_aggregate_function`，drop 时自动销毁。
///
/// RAII wrapper around an aggregate-function handle: owns the
/// `duckdb_aggregate_function` and destroys it on drop.
pub struct AggregateFunctionGuard {
    /// 函数名，仅用于错误信息。
    ///
    /// Function name, used only for error messages.
    name: String,
    /// 裸的 DuckDB 聚合函数句柄。
    ///
    /// The raw DuckDB aggregate-function handle.
    c_agg: duckdb_aggregate_function,
}
impl AggregateFunctionGuard {
    /// 返回裸句柄，供注册到函数集等底层调用使用。
    ///
    /// Returns the raw handle, for low-level calls such as adding it to a function set.
    pub fn as_raw(&self) -> duckdb_aggregate_function {
        self.c_agg
    }
}
impl Drop for AggregateFunctionGuard {
    fn drop(&mut self) {
        unsafe { duckdb_destroy_aggregate_function(&raw mut self.c_agg) };
    }
}

/// 聚合函数集句柄的 RAII 包装：drop 时自动销毁函数集。
///
/// RAII wrapper around an aggregate-function-set handle: destroys the set on drop.
pub struct AggregateFunctionSetGuard {
    /// 裸的 DuckDB 聚合函数集句柄。
    ///
    /// The raw DuckDB aggregate-function-set handle.
    c_set: duckdb_aggregate_function_set,
}
impl AggregateFunctionSetGuard {
    /// 把一个重载加进函数集；失败时返回带函数名的错误。
    ///
    /// Adds one overload to the set; on failure it returns an error carrying the function name.
    fn add_overload(&mut self, func: AggregateFunctionGuard) -> DuckResult<()> {
        let result = unsafe { duckdb_add_aggregate_function_to_set(self.as_raw(), func.as_raw()) };

        if result != DuckDBSuccess {
            return Err(duck_error(format!(
                "duckdb_add_aggregate_function_to_set failed: {}",
                func.name
            )));
        }

        Ok(())
    }
    /// 返回裸句柄，供注册到连接时使用。
    ///
    /// Returns the raw handle, used when registering the set on a connection.
    pub fn as_raw(&self) -> duckdb_aggregate_function_set {
        self.c_set
    }
}
impl Drop for AggregateFunctionSetGuard {
    fn drop(&mut self) {
        unsafe { duckdb_destroy_aggregate_function_set(&raw mut self.c_set) };
    }
}

/// 聚合函数集 builder：把若干签名（重载）合并成一个同名函数集再注册。
///
/// 由 `duckfn::register_all_aggregate_overload` 使用（见 `#[duck_aggregate_function(overloads_name = "xxx")]`）。
///
/// Aggregate function-set builder: merges several signatures (overloads) into one set with a
/// shared name and registers it. Used by `duckfn::register_all_aggregate_overload` (see
/// `#[duck_aggregate_function(overloads_name = "xxx")]`).
pub struct DuckfnAggregateFunctionSetBuilder {
    /// 函数集名字（C 字符串，注册时传给 DuckDB）。
    ///
    /// The set name (a C string passed to DuckDB at registration time).
    pub name: CString,
    /// 组成该函数集的所有重载句柄。
    ///
    /// All overload handles that make up the set.
    pub overloads: Vec<AggregateFunctionGuard>,
}
impl DuckfnAggregateFunctionSetBuilder {
    /// 用给定名字和重载列表创建一个函数集 builder。
    ///
    /// Creates a new builder for a function set with the given name.
    ///
    /// # Panics
    ///
    /// `name` 含内部 NUL 字节时 panic。
    ///
    /// Panics if `name` contains an interior null byte.
    pub fn new(name: &str, overloads: Vec<AggregateFunctionGuard>) -> Self {
        Self {
            name: CString::new(name).expect("function name must not contain null bytes"),
            overloads,
        }
    }

    /// # Safety
    ///
    /// `con` 必须是有效的 DuckDB 连接句柄；函数集会在注册后被 DuckDB 接管。
    ///
    /// `con` must be a valid DuckDB connection handle; the set is taken over by DuckDB after
    /// registration.
    ///
    /// 建函数集 -> 逐个添加重载 -> 注册到 `con`。
    ///
    /// Creates the set, adds every overload, then registers it on `con`.
    pub unsafe fn register(self, con: duckdb_connection) -> DuckResult<()> {
        let mut set = AggregateFunctionSetGuard {
            c_set: unsafe { duckdb_create_aggregate_function_set(self.name.as_ptr()) },
        };

        for x in self.overloads {
            set.add_overload(x)?;
        }

        let result = unsafe { duckdb_register_aggregate_function_set(con, set.as_raw()) };

        if result != DuckDBSuccess {
            return Err(ExtensionError::new(format!(
                "duckdb_register_aggregate_function_set failed for '{}'",
                self.name.to_string_lossy()
            )));
        }

        Ok(())
    }
}
