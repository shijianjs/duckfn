//! 聚合函数适配层：把有状态的 Rust 结构体注册成 DuckDB 聚合函数。
//!
//! Aggregate-function adapter: registers a stateful Rust struct as a DuckDB aggregate function.
//!
//! `catch_unwind` 的 panic 兜底**仅原生成立**；在 wasm/浏览器 (`wasm32-unknown-emscripten`) 上
//! 接不住 —— panic 无法跨 JS 边界展开，会变成 `Maximum call stack size exceeded` 栈溢出。
//! 报错请用 `Err(duck_error(..))` 而非 `panic!`（详见 `crate::utils::helpers` 模块文档）。
//!
//! The `catch_unwind` panic guard is native-only: on wasm a `panic!` cannot unwind across the JS
//! boundary and surfaces as a stack overflow. Report errors with `Err(duck_error(..))`, not `panic!`.

use crate::duck_columns::DuckColumns;
use crate::utils::builder_with_params::BuilderWithParams;
use crate::value_types::duck_value_type::DuckValueType;
use crate::{
    DuckExtraInfo, duck_aggregate_unwind, erased_extra_info, raw_extra_info, vec_option_to_ref,
    DuckOptionResult, DuckResult,
};
use libduckdb_sys::{
    duckdb_aggregate_state, duckdb_connection, duckdb_data_chunk, duckdb_function_info,
    duckdb_vector, idx_t,
};
use quack_rs::aggregate::{
    AggregateFunctionBuilder, AggregateOverloadBuilder, AggregateState, FfiState,
};
use quack_rs::data_chunk::DataChunk;
use quack_rs::prelude::{AggregateFunctionInfo, NullHandling};

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
    /// 生效范围：`aggregate_function_builder()`（独立聚合函数）与函数集重载
    /// （[`Self::aggregate_overload_builder`]，quack-rs 的 `AggregateOverloadBuilder` 同样有
    /// `extra_info`）。同一个函数集里的多个重载会各自调用本方法，注意别把同一个指针挂到两个
    /// 重载上（每个重载都会把自己的 destroy 交给 DuckDB）。
    ///
    /// Function-level data attached at registration time (DuckDB's `extra_info`); nothing is
    /// attached by default. DuckDB frees it through the destructor when the function object is
    /// dropped, so the type must be `Send + Sync + 'static` (the function object may be shared
    /// across threads and queries, and must be treated as read-only). It applies to
    /// `aggregate_function_builder()` (standalone aggregates) and function-set overloads
    /// ([`Self::aggregate_overload_builder`], since quack-rs' `AggregateOverloadBuilder` carries
    /// `extra_info` too). Every overload in one set calls this method, so beware of attaching the
    /// same pointer to two overloads (each hands its own destroy to DuckDB).
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

    /// 往「函数集重载」用的 [`AggregateOverloadBuilder`] 上补齐本签名的回调、参数表与返回类型。
    ///
    /// 返回类型取本签名的 `Self::Output`，因此同一函数集里的不同重载可以有不同的返回类型
    /// （DuckDB 只用参数类型与个数解析聚合重载，返回类型不参与）。
    ///
    /// Complements the given [`AggregateOverloadBuilder`] (used for function-set overloads) with
    /// this signature's callbacks, parameter list and return type. The return type comes from this
    /// signature's `Self::Output`, so overloads in one set may return different types (DuckDB
    /// resolves an aggregate overload from parameter types and arity only; the return type takes no
    /// part in it).
    fn aggregate_overload_builder(builder: AggregateOverloadBuilder) -> AggregateOverloadBuilder {
        let mut builder = builder
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
            // SAFETY: ptr 由 `DuckExtraInfo::into_raw` 产生，destroy 与它配对；重载句柄最终由函数集持有，
            // 析构时机由 DuckDB 决定。
            //
            // SAFETY: `ptr` comes from `DuckExtraInfo::into_raw` and `destroy` matches it; the overload
            // handle ends up owned by the function set and DuckDB decides when it is destroyed.
            builder = unsafe { builder.extra_info(ptr, destroy) };
        }
        builder
    }

    /// # Safety
    ///
    /// `con` 必须是由 DuckDB 提供的有效连接句柄。
    ///
    /// `con` must be a valid connection handle provided by DuckDB.
    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        unsafe { Self::aggregate_function_builder().register(con) }
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


