//! cast 函数适配层：把「源类型 -> 目标类型」的逐值转换注册成 DuckDB 的 cast 函数。
//!
//! Cast-function adapter: registers a value-wise `source -> target` conversion as a DuckDB
//! cast function.

use crate::value_types::duck_value_type::DuckValueType;
use crate::{DuckOptionResult, DuckResult, panic_to_string, vec_option_to_ref};
use libduckdb_sys::{duckdb_function_info, duckdb_vector, idx_t};
use quack_rs::prelude::{CastFunctionBuilder, CastFunctionInfo, CastMode, Connection, Registrar};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// 把「源类型 -> 目标类型」的逐值转换注册成 DuckDB 的 cast 函数。
///
/// 注册后 `CAST(x AS Target)` 会走这里的回调；如果设置了
/// [`CastFunctionAdapter::implicit_cost`]，DuckDB 还可能在需要隐式转换的表达式里
/// 自动使用它。同 `(source, target)` 已经存在内置转换时，注册的这个会覆盖/参与竞争。
///
/// 适配层把 quack-rs 的向量级回调（`unsafe extern "C" fn(info, count, input, output) -> bool`）
/// 拆成「逐行」的 Rust 代码：
///
/// - 从输入向量按行读 [`Self::Input`]（`None` 即 SQL NULL）；
/// - 交给 [`Self::apply_with_null`] 得到 [`Self::Output`]；
/// - 写完整个向量后返回。
///
/// 错误语义按 DuckDB 的 cast mode 分流：
///
/// - `CAST(...)`（[`CastMode::Normal`]）：`Err` 调用 `set_error` 并返回 `false`，整条查询失败；
/// - `TRY_CAST(...)`（[`CastMode::Try`]）：`Err` 调用 `set_row_error` 并把该行写成 NULL，
///   继续处理后面的行。
///
/// 函数体里的 panic 由 `catch_unwind` 捕获成查询错误，不会跨 FFI 展开。
/// 一般不用手写这个 impl，直接用 `#[duck_cast_function]` 作用在
/// `fn(from: Src) -> Target`（或 `Option<T>` / `DuckOptionResult<T>` 变体）上即可。
///
/// Registers a value-wise `source -> target` conversion as a DuckDB cast function. After
/// registration `CAST(x AS Target)` dispatches to this callback; when
/// [`CastFunctionAdapter::implicit_cost`] is set, DuckDB may also insert the conversion
/// implicitly. If a built-in cast for the same `(source, target)` pair exists, the registered
/// one overrides or competes with it.
///
/// The adapter splits quack-rs' vector-level callback
/// (`unsafe extern "C" fn(info, count, input, output) -> bool`) into per-row Rust code: it
/// reads [`Self::Input`] row by row from the input vector (`None` meaning SQL NULL), hands it
/// to [`Self::apply_with_null`] to obtain [`Self::Output`], and flushes the whole vector
/// afterwards.
///
/// Error semantics follow the DuckDB cast mode: for `CAST(...)` ([`CastMode::Normal`]) an
/// `Err` calls `set_error` and returns `false`, failing the whole query; for `TRY_CAST(...)`
/// ([`CastMode::Try`]) an `Err` calls `set_row_error`, writes NULL for that row and keeps
/// processing the remaining rows. Panics inside the body are caught by `catch_unwind` and
/// turned into query errors, never unwinding across FFI. Usually you do not implement this
/// manually: annotate `fn(from: Src) -> Target` (or its `Option<T>` / `DuckOptionResult<T>`
/// variants) with `#[duck_cast_function]`.
pub trait CastFunctionAdapter: Sized + 'static {
    /// 只用于标识回调（cast 函数本身没有 SQL 名字）。
    ///
    /// Used to identify the callback only (a cast function has no SQL name of its own).
    const NAME: &'static str;

    /// 源类型，决定注册时的 source logical type。
    ///
    /// Source type; determines the registered source logical type.
    type Input: DuckValueType;
    /// 目标类型，决定注册时的 target logical type。
    ///
    /// Target type; determines the registered target logical type.
    type Output: DuckValueType;

    /// 隐式转换代价：`Some(cost)` 时 DuckDB 可能自动插入该转换，值越小优先级越高；
    /// `None`（默认）表示只用于显式 `CAST`。
    ///
    /// Implicit-cast cost: with `Some(cost)` DuckDB may insert this conversion implicitly,
    /// and a smaller value means higher priority; `None` (the default) restricts it to
    /// explicit `CAST`.
    fn implicit_cost() -> Option<i64> {
        None
    }

    /// 逐值转换：`None` 表示输入是 SQL NULL。
    ///
    /// 入参写成 `T` 还是 `Option<T>` 由宏生成的代码处理：写 `T` 时 NULL 输入
    /// 直接输出 NULL（函数体不执行），写 `Option<T>` 时 NULL 以 `None` 进入函数体。
    ///
    /// Value-wise conversion; `None` means the input is SQL NULL. Whether `T` or `Option<T>`
    /// is used is handled by the macro-generated code: with `T` a NULL input yields NULL
    /// directly (the body is not executed); with `Option<T>` the NULL enters the body as
    /// `None`.
    fn apply_with_null(value: Option<Self::Input>) -> DuckOptionResult<Self::Output>;

    /// 构造 cast 函数 builder（含源/目标逻辑类型、回调与可选的隐式代价）。
    ///
    /// Builds the cast-function builder (source/target logical types, callback and optional
    /// implicit cost).
    fn cast_function_builder() -> CastFunctionBuilder {
        // 统一走 new_logical：简单类型 logical_type() 就是 LogicalType::new(type_id())，
        // 复杂类型（LIST / MAP / ARRAY / STRUCT）在各自的 DuckValueType 实现里重写过。
        let mut builder = CastFunctionBuilder::new_logical(
            Self::Input::logical_type(),
            Self::Output::logical_type(),
        )
        .function(Self::cast_function_wrapper);
        if let Some(cost) = Self::implicit_cost() {
            builder = builder.implicit_cost(cost);
        }
        builder
    }

    /// # Safety
    ///
    /// `c` 必须是有效的 DuckDB 连接。
    ///
    /// `c` must be a valid DuckDB connection.
    unsafe fn register(c: &Connection) -> DuckResult<()> {
        // SAFETY: 由调用方保证连接有效；builder 已设置好 source/target/callback。
        unsafe { c.register_cast(Self::cast_function_builder()) }
    }

    /// # Safety
    ///
    /// 由 DuckDB 回调，`info` / `input` / `output` 均由 DuckDB 保证有效。
    ///
    /// Called by DuckDB; `info`, `input` and `output` are guaranteed valid by DuckDB.
    unsafe extern "C" fn cast_function_wrapper(
        info: duckdb_function_info,
        count: idx_t,
        input: duckdb_vector,
        output: duckdb_vector,
    ) -> bool {
        // SAFETY: info 由 DuckDB 传入，在回调期间有效。
        let info = unsafe { CastFunctionInfo::new(info) };
        match catch_unwind(AssertUnwindSafe(|| {
            cast_batch::<Self>(&info, count as usize, input, output)
        })) {
            Ok(success) => success,
            Err(e) => {
                info.set_error(&panic_to_string(e));
                false
            }
        }
    }
}

/// 逐行转换：读输入向量、调用 [`CastFunctionAdapter::apply_with_null`]、写输出向量。
///
/// 私有函数（不是 trait 方法），这样 `output` 这个裸指针参数的解引用不会被
/// `clippy::not_unsafe_ptr_arg_deref` 挂在公开 API 上。
///
/// Row-by-row conversion: reads the input vector, calls
/// [`CastFunctionAdapter::apply_with_null`] and writes the output vector. It is a private
/// function (not a trait method) so that dereferencing the raw-pointer argument `output` does
/// not surface `clippy::not_unsafe_ptr_arg_deref` on the public API.
fn cast_batch<A: CastFunctionAdapter>(
    info: &CastFunctionInfo,
    count: usize,
    input: duckdb_vector,
    output: duckdb_vector,
) -> bool {
    let reader = A::Input::create_reader_from_vector(input, count);
    let try_mode = info.cast_mode() == CastMode::Try;
    let mut results: Vec<Option<A::Output>> = Vec::with_capacity(count);

    for row in 0..count {
        let value = A::Input::read(&reader, row);
        match A::apply_with_null(value) {
            Ok(v) => results.push(v),
            Err(e) => {
                if try_mode {
                    // TRY_CAST：这一行失败 -> 记下行级错误并把该行写成 NULL
                    // SAFETY: output 就是 DuckDB 传给本回调的输出向量。
                    unsafe { info.set_row_error(e.as_str(), row as idx_t, output) };
                    results.push(None);
                } else {
                    // CAST：整条查询失败
                    info.set_error(e.as_str());
                    return false;
                }
            }
        }
    }

    A::Output::write_batch(output, &vec_option_to_ref(&results));
    true
}
