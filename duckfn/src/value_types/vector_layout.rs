//! `LIST` / `MAP` / `STRUCT` 的物理布局写法：子向量、entry、NULL 行与收尾。
//!
//! Physical-layout writes for `LIST` / `MAP` / `STRUCT`: child vectors, entries, NULL rows and
//! finishing.
//!
//! 这些约定原先在 `duck_list.rs` / `duck_map.rs` / `duck_struct.rs`（静态类型通路）和
//! `DynColumnWriter`（动态列通路）里各写了一遍。集中到这里之后，那些容易写错的细节 ——
//! 「`LIST` / `MAP` 的 NULL 行要写显式空 entry」「`MAP` 与 `LIST` 物理同构」「`STRUCT` 子向量按
//! 字段下标取」—— 只有一个出处，两条通路共用同一份实现。
//!
//! These conventions used to be spelled out once in `duck_list.rs` / `duck_map.rs` /
//! `duck_struct.rs` (the static path) and again in `DynColumnWriter` (the dynamic one). Collecting
//! them here gives the error-prone details — "a `LIST` / `MAP` NULL row writes an explicit empty
//! entry", "`MAP` shares `LIST`'s physical layout", "a `STRUCT` child comes from its field index" —
//! a single home, shared by both paths.

// 这些 helper 都接收 DuckDB 的裸向量句柄：和解引用裸指针的既有代码一样，调用方负责保证句柄类型
// 与生命周期正确，这里不做重复的 unsafe 标注。
//
// These helpers all take raw DuckDB vector handles: as with the existing code that dereferences raw
// pointers, the caller guarantees the handle's type and lifetime, so this is not marked unsafe.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use crate::DuckValueWriter;
use libduckdb_sys::duckdb_vector;
use quack_rs::prelude::{ListVector, MapVector, StructVector};

/// `LIST` / `MAP` 的子向量（两者物理同构：`LIST(elem)` 与 `LIST(STRUCT(k, v))`）。
///
/// The child vector of a `LIST` / `MAP` (the two share their physical layout: `LIST(elem)` versus
/// `LIST(STRUCT(k, v))`).
///
/// # Safety
///
/// `vector` 必须是本行所在列的、有效的 `LIST` / `MAP` 向量。
///
/// `vector` must be a valid `LIST` / `MAP` vector of the column being written.
#[must_use]
pub(crate) fn element_child_vector(vector: duckdb_vector) -> duckdb_vector {
    // SAFETY: 由调用方保证 vector 是有效的 LIST / MAP 向量。
    //
    // SAFETY: the caller guarantees `vector` is a valid LIST / MAP vector.
    unsafe { ListVector::get_child(vector) }
}

/// 为子向量预留容量；`LIST` / `MAP` 批量写入前调用一次，传本批元素总数。
///
/// Reserves capacity in the child vector; call it once before a batch write, with the total number
/// of elements in the batch.
///
/// # Safety
///
/// 同 [`element_child_vector`]。
///
/// Same as [`element_child_vector`].
pub(crate) fn reserve_elements(vector: duckdb_vector, total_elements: usize) {
    // SAFETY: 由调用方保证 vector 是有效的 LIST / MAP 向量。
    unsafe { ListVector::reserve(vector, total_elements) }
}

/// 写第 `row` 行的 entry：子向量起点 `offset`、长度 `length`。
///
/// Writes the entry of row `row`: start `offset` in the child vector and `length` elements.
///
/// # Safety
///
/// 同 [`element_child_vector`]；`offset + length` 不能超过已预留的子向量容量。
///
/// Same as [`element_child_vector`]; `offset + length` must stay within the reserved capacity.
pub(crate) fn set_entry(vector: duckdb_vector, row: usize, offset: usize, length: usize) {
    // SAFETY: 由调用方保证 vector 有效且容量足够。
    unsafe { ListVector::set_entry(vector, row, offset as u64, length as u64) }
}

/// 收尾：把子向量长度收窄到已写入的元素数 `written`。
///
/// Finishes by clamping the child-vector length to the number of elements written (`written`).
///
/// # Safety
///
/// 同 [`element_child_vector`]。
pub(crate) fn finish_elements(vector: duckdb_vector, written: usize) {
    // SAFETY: 由调用方保证 vector 是有效的 LIST / MAP 向量。
    unsafe { ListVector::set_size(vector, written) }
}

/// `LIST` / `MAP` 的 NULL 行：父向量置空之外，写一个显式的空 entry `(0, 0)`。
///
/// A NULL row for `LIST` / `MAP`: besides marking the parent NULL, write an explicit empty entry
/// `(0, 0)`.
///
/// NULL 行的 `list_entry_t` 本来不会被写，留着未初始化的 offset/length。实测 DuckDB 的 list
/// 算子都会先查父向量 validity（子向量长度也被 [`finish_elements`] 收窄到已写范围），所以不写
/// entry 也安全；这里补上是为了让 duckfn 不依赖这一前提 —— ARRAY 的教训正是「不要假设对方一定
/// 先查父 validity」。
///
/// The entry of a NULL row would otherwise keep an uninitialised offset/length. DuckDB's list
/// operators do check the parent validity (and the child length is clamped by
/// [`finish_elements`]), so writing it is defensive rather than a fix — the ARRAY lesson being
/// exactly "do not assume the other side checks the parent validity".
pub(crate) fn write_null_row(writer: &mut DuckValueWriter, row: usize) {
    // SAFETY: 由调用方保证 writer 是当前列、当前行的写入器。
    unsafe {
        writer.vector_writer.set_null(row);
        ListVector::set_entry(writer.c_duckdb_vector, row, 0, 0);
    }
}

/// `MAP` 的键子向量（子 `STRUCT` 的第 0 个字段）。
///
/// The keys vector of a `MAP` (field 0 of its child `STRUCT`).
///
/// # Safety
///
/// `vector` 必须是本行所在列的、有效的 `MAP` 向量。
///
/// `vector` must be a valid `MAP` vector of the column being written.
#[must_use]
pub(crate) fn map_keys(vector: duckdb_vector) -> duckdb_vector {
    // SAFETY: 由调用方保证 vector 是有效的 MAP 向量。
    unsafe { MapVector::keys(vector) }
}

/// `MAP` 的值子向量（子 `STRUCT` 的第 1 个字段）。
///
/// The values vector of a `MAP` (field 1 of its child `STRUCT`).
///
/// # Safety
///
/// 同 [`map_keys`]。
pub(crate) fn map_values(vector: duckdb_vector) -> duckdb_vector {
    // SAFETY: 由调用方保证 vector 是有效的 MAP 向量。
    unsafe { MapVector::values(vector) }
}

/// `STRUCT` 的第 `index` 个子向量。
///
/// Child vector `index` of a `STRUCT`.
///
/// # Safety
///
/// `vector` 必须是本行所在列的、有效的 `STRUCT` 向量，且 `index` 在字段范围内。
///
/// `vector` must be a valid `STRUCT` vector of the column being written, with `index` in range.
#[must_use]
pub(crate) fn struct_field(vector: duckdb_vector, index: usize) -> duckdb_vector {
    // SAFETY: 由调用方保证 vector 是有效的 STRUCT 向量且 index 合法。
    unsafe { StructVector::get_child(vector, index) }
}
