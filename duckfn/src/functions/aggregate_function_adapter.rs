use crate::duck_columns::DuckColumns;
use crate::utils::builder_with_params::BuilderWithParams;
use crate::value_types::duck_value_type::DuckValueType;
use crate::{DuckOptionResult, DuckResult, duck_aggregate_unwind, duck_error, vec_option_to_ref};
use libduckdb_sys::{
    DuckDBSuccess, duckdb_add_aggregate_function_to_set, duckdb_aggregate_function,
    duckdb_aggregate_function_add_parameter, duckdb_aggregate_function_set,
    duckdb_aggregate_function_set_destructor, duckdb_aggregate_function_set_functions,
    duckdb_aggregate_function_set_name, duckdb_aggregate_function_set_return_type,
    duckdb_aggregate_function_set_special_handling, duckdb_aggregate_state, duckdb_connection,
    duckdb_create_aggregate_function, duckdb_create_aggregate_function_set, duckdb_data_chunk,
    duckdb_destroy_aggregate_function, duckdb_destroy_aggregate_function_set, duckdb_function_info,
    duckdb_register_aggregate_function_set, duckdb_vector, idx_t,
};
use quack_rs::aggregate::builder::OverloadBuilder;
use quack_rs::aggregate::{AggregateFunctionBuilder, AggregateState, FfiState};
use quack_rs::data_chunk::DataChunk;
use quack_rs::error::ExtensionError;
use quack_rs::prelude::{AggregateFunctionInfo, LogicalType, NullHandling, TypeId};
use std::ffi::CString;

pub trait AggregateFunctionAdapter: AggregateState + Sized + 'static {
    /// # Safety
    ///
    /// 由 DuckDB 回调，`_info` 由 DuckDB 保证有效。
    unsafe extern "C" fn c_state_size(_info: duckdb_function_info) -> idx_t {
        unsafe { FfiState::<Self>::size_callback(_info) }
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`info`/`state` 均由 DuckDB 保证有效。
    unsafe extern "C" fn c_state_init(info: duckdb_function_info, state: duckdb_aggregate_state) {
        unsafe { FfiState::<Self>::init_callback(info, state) };
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`_info`/`input`/`states` 均由 DuckDB 保证有效。
    unsafe extern "C" fn c_update(
        _info: duckdb_function_info,
        input: duckdb_data_chunk,
        states: *mut duckdb_aggregate_state,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(_info) };
        duck_aggregate_unwind(&info, || {
            let chunk = unsafe { DataChunk::from_raw(input) };
            let readers = Self::Args::create_column_readers(&chunk);
            let row_count = chunk.size();
            for row in 0..row_count {
                let args = Self::Args::read_columns(&readers, row);
                let state_ptr = unsafe { *states.add(row) };
                if let Some(st) = unsafe { FfiState::<Self>::with_state_mut(state_ptr) } {
                    let result = st.handle_row_with_null(args);
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
    unsafe extern "C" fn c_combine(
        _info: duckdb_function_info,
        source: *mut duckdb_aggregate_state,
        target: *mut duckdb_aggregate_state,
        count: idx_t,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(_info) };
        duck_aggregate_unwind(&info, || {
            for i in 0..count as usize {
                let src_ptr = unsafe { *source.add(i) };
                let tgt_ptr = unsafe { *target.add(i) };
                let src = unsafe { FfiState::<Self>::with_state(src_ptr) };
                let tgt = unsafe { FfiState::<Self>::with_state_mut(tgt_ptr) };
                if let (Some(s), Some(t)) = (src, tgt) {
                    let result = t.combine(s);
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
    /// 由 DuckDB 回调，`_info`/`source`/`result` 均由 DuckDB 保证有效。
    unsafe extern "C" fn c_finalize(
        _info: duckdb_function_info,
        source: *mut duckdb_aggregate_state,
        result: duckdb_vector,
        count: idx_t,
        offset: idx_t,
    ) {
        let info = unsafe { AggregateFunctionInfo::new(_info) };
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
                        let result1 = st.result();
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
    unsafe extern "C" fn c_state_destroy(states: *mut duckdb_aggregate_state, count: idx_t) {
        unsafe { FfiState::<Self>::destroy_callback(states, count) };
    }

    fn null_handling() -> NullHandling {
        NullHandling::DefaultNullHandling
    }

    fn aggregate_function_builder() -> AggregateFunctionBuilder {
        AggregateFunctionBuilder::new(Self::NAME)
            .state_size(Self::c_state_size)
            .init(Self::c_state_init)
            .update(Self::c_update)
            .combine(Self::c_combine)
            .finalize(Self::c_finalize)
            .destructor(Self::c_state_destroy)
            .null_handling(Self::null_handling())
            .returns_logical(Self::Output::logical_type())
            .with_params(Self::Args::column_types())
    }
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
    unsafe fn register(con: duckdb_connection) -> DuckResult<()> {
        unsafe { Self::aggregate_function_builder().register(con) }
    }

    fn create_aggregate_function_guard(name: &CString) -> AggregateFunctionGuard {
        let mut func = unsafe { duckdb_create_aggregate_function() };
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

        // unsafe { duckdb_add_aggregate_function_to_set(c_set, func) };

        unsafe { duckdb_destroy_aggregate_function(&raw mut func) };

        AggregateFunctionGuard {
            name: Self::NAME.to_string(),
            c_agg: func,
        }
    }

    const NAME: &'static str;

    /// cargo add tuple-transpose
    /// 使用这个工具包可以快速处理多个Option参数
    type Args: DuckColumns;
    type Output: DuckValueType;

    fn handle_row_with_null(&mut self, args: Option<Self::Args>) -> DuckResult<()> {
        if let Some(args) = args {
            self.handle_row(args)?;
        }
        Ok(())
    }
    fn handle_row(&mut self, args: Self::Args) -> DuckResult<()>;
    fn combine(&mut self, other: &Self) -> DuckResult<()>;
    fn result(&self) -> DuckOptionResult<Self::Output>;
}

pub trait DuckAggregateState {
    type Output: DuckValueType;
    fn combine(&mut self, other: &Self) -> DuckResult<()> {
        self.simple_combine(other);
        Ok(())
    }
    fn simple_combine(&mut self, _other: &Self) {
        todo!("simple_combine is not implemented")
    }
    fn result(&self) -> DuckOptionResult<Self::Output> {
        Ok(Some(self.simple_result()))
    }
    fn simple_result(&self) -> Self::Output {
        todo!("simple_result is not implemented")
    }
}

pub struct AggregateFunctionGuard {
    name: String,
    c_agg: duckdb_aggregate_function,
}
impl AggregateFunctionGuard {
    pub fn as_raw(&self) -> duckdb_aggregate_function {
        self.c_agg
    }
}
impl Drop for AggregateFunctionGuard {
    fn drop(&mut self) {
        unsafe { duckdb_destroy_aggregate_function(&raw mut self.c_agg) };
    }
}
pub struct AggregateFunctionSetGuard {
    c_set: duckdb_aggregate_function_set,
}
impl AggregateFunctionSetGuard {
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
    pub fn as_raw(&self) -> duckdb_aggregate_function_set {
        self.c_set
    }
}
impl Drop for AggregateFunctionSetGuard {
    fn drop(&mut self) {
        unsafe { duckdb_destroy_aggregate_function_set(&raw mut self.c_set) };
    }
}
pub struct DuckfnAggregateFunctionSetBuilder {
    pub name: CString,
    pub overloads: Vec<AggregateFunctionGuard>,
}
impl DuckfnAggregateFunctionSetBuilder {
    /// Creates a new builder for a function set with the given name.
    ///
    /// # Panics
    ///
    /// Panics if `name` contains an interior null byte.
    pub fn new(name: &str, overloads: Vec<AggregateFunctionGuard>) -> Self {
        Self {
            name: CString::new(name).expect("function name must not contain null bytes"),
            overloads,
        }
    }

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
