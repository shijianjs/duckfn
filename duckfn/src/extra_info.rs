//! 函数级附加数据：DuckDB C API 的 `extra_info`。
//!
//! Function-level extra data: the DuckDB C API's `extra_info`.
//!
//! 每个 DuckDB 函数对象都可以挂一份「擦除指针」：注册时通过
//! `duckdb_<kind>_function_set_extra_info` 挂上，回调里用
//! `duckdb_<kind>_function_get_extra_info` 取回，函数对象销毁时 DuckDB 调用我们注册的析构回调
//! 释放它。它属于**函数对象**（跨查询共享、应视为只读），不是「每次查询一份」的状态 —— 后者请用
//! 表函数的 [`with_state`](crate::TableFunctionAdapter::with_state) 或 quack-rs 的 bind data。
//!
//! Every DuckDB function object can carry one erased pointer: it is attached at registration time
//! through `duckdb_<kind>_function_set_extra_info`, retrieved inside callbacks through
//! `duckdb_<kind>_function_get_extra_info`, and freed by the destructor we registered when DuckDB
//! drops the function object. It belongs to the **function object** (shared across queries and to be
//! treated as read-only), not to a single query — for per-query state use a table function's
//! [`with_state`](crate::TableFunctionAdapter::with_state) or quack-rs' bind data.
//!
//! 哪些适配层暴露了这个钩子（宏不涉及 `extra_info`，只有手写适配器会用）：
//!
//! Which adapters expose this hook (the macros never touch `extra_info`; only hand-written adapters
//! use it):
//!
//! | 适配层 / adapter | 钩子 / hook | 说明 / notes |
//! | --- | --- | --- |
//! | [`ScalarFunctionAdapter`](crate::ScalarFunctionAdapter) | [`ScalarFunctionAdapter::extra_info`] | 独立函数与函数集重载都支持 / both standalone and set overloads |
//! | [`CastFunctionAdapter`](crate::CastFunctionAdapter) | [`CastFunctionAdapter::extra_info`] | |
//! | [`AggregateFunctionAdapter`](crate::AggregateFunctionAdapter) | [`AggregateFunctionAdapter::extra_info`] | 独立函数与 duckfn 自建的聚合函数集 / standalone and duckfn's own aggregate set |
//! | [`ReplacementScanAdapter`](crate::ReplacementScanAdapter) | [`ReplacementScanAdapter::extra_info`] | 走注册参数 `extra_data` / through the `extra_data` registration argument |
//! | `CopyFromFunctionAdapter` | `extra_info` | 挂在 reader 表函数上（需要 `duckdb-1-5`）/ attached to the reader table function (`duckdb-1-5`) |
//! | 表函数 / `COPY TO` | 无 / none | 见下 / see below |
//!
//! 表函数、`COPY TO` 为什么没有：quack-rs 的 typed 表函数 builder 把 `extra_info` 槽位**自己用掉了**
//! （存它的 bind/scan 闭包），而 `CopyFunctionBuilder` 压根不暴露该接口；这两个槽位都要改 quack-rs
//! 或绕开它的 builder 才能拿到，所以 duckfn 这一层不提供（相对 quack-rs 的 builder 能力并无损失）。
//!
//! Why table functions and `COPY TO` have none: quack-rs' typed table-function builder **occupies**
//! the `extra_info` slot itself (it stores its bind/scan closures there), and `CopyFunctionBuilder`
//! does not expose the interface at all. Both would require changing quack-rs or bypassing its
//! builder, so duckfn does not offer them (no capability is lost relative to quack-rs' builders).
//!
//! # 示例 / Example
//!
//! ```ignore
//! struct MyFn;
//! struct MyConfig {
//!     scale: i64,
//! }
//!
//! impl duckfn::ScalarFunctionAdapter for MyFn {
//!     // ... NAME / Args / Output ...
//!
//!     fn extra_info() -> Option<duckfn::DuckExtraInfo> {
//!         Some(duckfn::DuckExtraInfo::new(MyConfig { scale: 2 }))
//!     }
//!
//!     fn apply_with_extra(
//!         args: Option<Self::Args>,
//!         extra: Option<&duckfn::DuckExtraInfo>,
//!     ) -> duckfn::DuckOptionResult<Self::Output> {
//!         let scale = extra
//!             .and_then(|extra| extra.downcast_ref::<MyConfig>())
//!             .map_or(1, |config| config.scale);
//!         // ...
//!         # todo!()
//!     }
//! }
//! ```

use std::any::Any;
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use libduckdb_sys::duckdb_delete_callback_t;
use quack_rs::aggregate::AggregateFunctionInfo;
use quack_rs::prelude::{CastFunctionInfo, ScalarFunctionInfo};
use quack_rs::table::{BindInfo, FunctionInfo, InitInfo};

/// 函数级附加数据：一个擦除指针的安全外壳。
///
/// Function-level extra data: a safe shell around one erased pointer.
///
/// 用 [`Self::new`] 把任意 `Send + Sync + 'static` 的值挂到函数对象上，之后在回调/业务代码里用
/// [`Self::downcast_ref`] 取回。`Send + Sync` 是硬要求：DuckDB 的函数对象可能被多个线程、多个
/// 查询共享。
///
/// Attach any `Send + Sync + 'static` value to the function object with [`Self::new`] and retrieve it
/// with [`Self::downcast_ref`]. `Send + Sync` is required: DuckDB may share a function object across
/// threads and queries.
pub struct DuckExtraInfo(Box<dyn Any + Send + Sync>);

// `extra_info` 只会以 `&` 交给业务代码，且构造时就要求 `Send + Sync`，因此把它视作「panic 时也安全」
// 是有依据的；这样适配层里的 `UnwindSafe` 边界不必为它开洞。
//
// Extra data is only ever handed to business code by `&`, and `new` already requires `Send + Sync`,
// so treating it as unwind-safe is justified — and it keeps the adapters' `UnwindSafe` bounds intact.
impl std::panic::UnwindSafe for DuckExtraInfo {}
impl std::panic::RefUnwindSafe for DuckExtraInfo {}

impl DuckExtraInfo {
    /// 用给定值构造附加数据。
    ///
    /// Wraps the given value as extra data.
    pub fn new<T: Any + Send + Sync>(value: T) -> Self {
        Self(Box::new(value))
    }

    /// 取回具体类型的引用；类型不符时返回 `None`。
    ///
    /// Returns a reference to the concrete type, or `None` when the type does not match.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref::<T>()
    }

    /// 附加数据是否为 `T`。
    ///
    /// Whether the extra data holds a value of type `T`.
    #[must_use]
    pub fn is<T: Any>(&self) -> bool {
        self.0.is::<T>()
    }

    /// 转成交给 DuckDB 的裸指针（薄指针，指向本结构体本身）。
    ///
    /// Turns this into the raw pointer handed to DuckDB (a thin pointer to this struct).
    pub(crate) fn into_raw(self) -> *mut c_void {
        Box::into_raw(Box::new(self)).cast::<c_void>()
    }

    /// 从裸指针还原引用；指针为 null 时返回 `None`。
    ///
    /// Restores a reference from the raw pointer; `None` when the pointer is null.
    ///
    /// # Safety
    ///
    /// `ptr` 必须由 [`Self::into_raw`] 产生，或为 null；且只能在 DuckDB 调用析构回调之前使用。
    ///
    /// `ptr` must come from [`Self::into_raw`] or be null, and must be used before DuckDB calls the
    /// destructor.
    pub(crate) unsafe fn from_raw<'a>(ptr: *mut c_void) -> Option<&'a Self> {
        // SAFETY: 调用方保证 ptr 来自 into_raw（DuckDB 在函数对象存活期间一直持有它），
        // null 由 as_ref 处理。
        unsafe { ptr.cast::<Self>().as_ref() }
    }
}

impl std::fmt::Debug for DuckExtraInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 具体类型被擦除了，只能报告「有数据」。
        //
        // The concrete type is erased, so all we can report is that data is present.
        f.debug_struct("DuckExtraInfo").finish_non_exhaustive()
    }
}

/// DuckDB 的 `extra_info` 析构回调：注册时交给 DuckDB，函数对象销毁时由 DuckDB 调用。
///
/// The `extra_info` destructor callback handed to DuckDB at registration time; DuckDB calls it when
/// the function object is destroyed.
///
/// # Safety
///
/// `ptr` 必须由 [`DuckExtraInfo::into_raw`] 产生，或为 null；DuckDB 保证每个指针只调用一次。
///
/// `ptr` must come from [`DuckExtraInfo::into_raw`] or be null; DuckDB guarantees exactly one call
/// per pointer.
pub(crate) unsafe extern "C" fn destroy_extra_info(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    // 用户类型的 `Drop` 理论上可能 panic，而 panic 跨 FFI 展开是未定义行为，所以照样兜一层。
    //
    // A user type's `Drop` could in principle panic, and unwinding across FFI is undefined behaviour,
    // so it is caught here as well.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: ptr 由 into_raw 产生，且 DuckDB 只会调用一次。
        unsafe { drop(Box::from_raw(ptr.cast::<DuckExtraInfo>())) };
    }));
}

/// 把 [`DuckExtraInfo`] 转成 builder / C API 需要的一对参数 `(指针, 析构回调)`。
///
/// Turns [`DuckExtraInfo`] into the `(pointer, destructor)` pair expected by the builders and the C
/// API.
///
/// 返回 `None` 表示「没有附加数据」，此时调用方**不要**调用 `set_extra_info`：挂一个空指针配上
/// 析构回调只会让 DuckDB 白跑一趟。
///
/// `None` means "nothing attached"; the caller must then **not** call `set_extra_info` — attaching a
/// null pointer with a destructor would only make DuckDB run a no-op.
pub(crate) fn raw_extra_info(
    extra: Option<DuckExtraInfo>,
) -> Option<(*mut c_void, duckdb_delete_callback_t)> {
    extra.map(|extra| {
        (
            extra.into_raw(),
            Some(destroy_extra_info as unsafe extern "C" fn(*mut c_void)),
        )
    })
}

/// 能取回函数级附加数据的回调信息（`duckdb_*_get_extra_info` 的薄封装）。
///
/// Callback info that can retrieve function-level extra data (a thin wrapper over
/// `duckdb_*_get_extra_info`).
///
/// duckfn 已为 quack-rs 的各 `*Info` 类型实现该 trait，因此在自己重写的回调里可以直接
/// `unsafe { duckfn::extra_info_ref::<MyConfig>(&info) }`。
///
/// duckfn implements this for quack-rs' `*Info` types, so an overridden callback can call
/// `unsafe { duckfn::extra_info_ref::<MyConfig>(&info) }` directly.
pub trait DuckExtraInfoSource {
    /// 取回原始附加数据指针（没有挂时为 null）。
    ///
    /// Returns the raw extra-info pointer (null when nothing was attached).
    ///
    /// # Safety
    ///
    /// 返回值只在当前回调期间有效；`self` 必须来自挂了该数据的那个函数的回调。
    ///
    /// The returned pointer is only valid during the current callback; `self` must come from a
    /// callback of the function the data was attached to.
    unsafe fn raw_extra_info(&self) -> *mut c_void;
}

impl DuckExtraInfoSource for ScalarFunctionInfo {
    unsafe fn raw_extra_info(&self) -> *mut c_void {
        // SAFETY: 由调用方保证在回调期间调用。
        unsafe { self.get_extra_info() }
    }
}

impl DuckExtraInfoSource for AggregateFunctionInfo {
    unsafe fn raw_extra_info(&self) -> *mut c_void {
        // SAFETY: 由调用方保证在回调期间调用。
        unsafe { self.get_extra_info() }
    }
}

impl DuckExtraInfoSource for CastFunctionInfo {
    unsafe fn raw_extra_info(&self) -> *mut c_void {
        // SAFETY: 由调用方保证在回调期间调用。
        unsafe { self.get_extra_info() }
    }
}

impl DuckExtraInfoSource for BindInfo {
    unsafe fn raw_extra_info(&self) -> *mut c_void {
        // SAFETY: 由调用方保证在回调期间调用。
        unsafe { self.get_extra_info() }
    }
}

impl DuckExtraInfoSource for InitInfo {
    unsafe fn raw_extra_info(&self) -> *mut c_void {
        // SAFETY: 由调用方保证在回调期间调用。
        unsafe { self.get_extra_info() }
    }
}

impl DuckExtraInfoSource for FunctionInfo {
    unsafe fn raw_extra_info(&self) -> *mut c_void {
        // SAFETY: 由调用方保证在回调期间调用。
        unsafe { self.get_extra_info() }
    }
}

/// COPY 函数的四个回调信息都支持取回附加数据（`duckdb-1-5` 才有的 C API）。
///
/// All four copy-function callback infos can retrieve extra data (a `duckdb-1-5`-only C API).
#[cfg(feature = "duckdb-1-5")]
mod copy_sources {
    use super::{DuckExtraInfoSource, c_void};
    use quack_rs::copy_function::{CopyBindInfo, CopyFinalizeInfo, CopyGlobalInitInfo, CopySinkInfo};

    impl DuckExtraInfoSource for CopyBindInfo {
        unsafe fn raw_extra_info(&self) -> *mut c_void {
            // SAFETY: 由调用方保证在回调期间调用。
            unsafe { self.get_extra_info() }
        }
    }

    impl DuckExtraInfoSource for CopyGlobalInitInfo {
        unsafe fn raw_extra_info(&self) -> *mut c_void {
            // SAFETY: 由调用方保证在回调期间调用。
            unsafe { self.get_extra_info() }
        }
    }

    impl DuckExtraInfoSource for CopySinkInfo {
        unsafe fn raw_extra_info(&self) -> *mut c_void {
            // SAFETY: 由调用方保证在回调期间调用。
            unsafe { self.get_extra_info() }
        }
    }

    impl DuckExtraInfoSource for CopyFinalizeInfo {
        unsafe fn raw_extra_info(&self) -> *mut c_void {
            // SAFETY: 由调用方保证在回调期间调用。
            unsafe { self.get_extra_info() }
        }
    }
}

/// 读取附加数据外壳；没挂（指针为 null）时返回 `None`。
///
/// Returns the extra-data shell; `None` when nothing was attached (the pointer is null).
///
/// 适配层用它把 `extra_info` 传给业务方法（`apply_with_extra` 等）而不必知道具体类型。
///
/// The adapters use it to hand `extra_info` to business methods (`apply_with_extra`, ...) without
/// having to know the concrete type.
///
/// # Safety
///
/// 只能在对应回调期间调用：返回的引用不超出 `source` 所在回调的生命周期。
///
/// Only call this during the matching callback: the returned reference does not outlive the callback
/// `source` belongs to.
pub unsafe fn erased_extra_info(source: &impl DuckExtraInfoSource) -> Option<&DuckExtraInfo> {
    // SAFETY: 由调用方保证在回调期间调用。
    let ptr = unsafe { source.raw_extra_info() };
    // SAFETY: ptr 要么是 null（from_raw 返回 None），要么来自 DuckExtraInfo::into_raw。
    unsafe { DuckExtraInfo::from_raw(ptr) }
}

/// 读取附加数据并直接转成 `T`；没挂或类型不符时返回 `None`。
///
/// Reads the extra data and downcasts it to `T`; `None` when nothing was attached or the type does
/// not match.
///
/// # Safety
///
/// 同 [`erased_extra_info`]：只能在对应回调期间调用。
///
/// Same as [`erased_extra_info`]: only call it during the matching callback.
pub unsafe fn extra_info_ref<T: Any>(source: &impl DuckExtraInfoSource) -> Option<&T> {
    // SAFETY: 由调用方保证在回调期间调用。
    unsafe { erased_extra_info(source) }?.downcast_ref::<T>()
}

#[cfg(test)]
mod tests {
    use super::{DuckExtraInfo, destroy_extra_info, raw_extra_info};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn downcast_matches_the_original_type() {
        let info = DuckExtraInfo::new(41_i32);
        assert!(info.is::<i32>());
        assert_eq!(info.downcast_ref::<i32>(), Some(&41));
        assert!(!info.is::<u8>());
        assert_eq!(info.downcast_ref::<u8>(), None);
        assert!(!format!("{info:?}").is_empty());
    }

    #[test]
    fn raw_round_trip_keeps_the_value() {
        let ptr = DuckExtraInfo::new(String::from("cfg")).into_raw();
        assert!(!ptr.is_null());
        // SAFETY: ptr 来自 into_raw，且尚未交给 DuckDB。
        let back = unsafe { DuckExtraInfo::from_raw(ptr) }.expect("non-null pointer");
        assert_eq!(back.downcast_ref::<String>().map(String::as_str), Some("cfg"));
        // SAFETY: ptr 来自 into_raw，只销毁一次。
        unsafe { destroy_extra_info(ptr) };
    }

    #[test]
    fn null_pointer_yields_none_and_destruction_is_a_noop() {
        // SAFETY: null 是 from_raw 允许的输入。
        assert!(unsafe { DuckExtraInfo::from_raw(std::ptr::null_mut()) }.is_none());
        // SAFETY: null 是 destroy_extra_info 允许的输入。
        unsafe { destroy_extra_info(std::ptr::null_mut()) };
    }

    #[test]
    fn nothing_attached_produces_no_raw_pair() {
        assert!(raw_extra_info(None).is_none());
    }

    #[test]
    fn destructor_runs_the_value_drop_once() {
        struct Tracker(Arc<AtomicUsize>);
        impl Drop for Tracker {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let (ptr, destroy) = raw_extra_info(Some(DuckExtraInfo::new(Tracker(Arc::clone(&counter)))))
            .expect("extra info was attached");
        assert!(destroy.is_some());
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        // SAFETY: ptr 来自 into_raw，且只销毁一次。
        unsafe { destroy_extra_info(ptr) };
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
