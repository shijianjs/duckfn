//! 列写入器树：`DynColumnWriter` 及批量取值辅助。
//!
//! Column writer tree: `DynColumnWriter` and the batch-value helpers.

use crate::value_types::vector_layout::{
    element_child_vector, finish_elements, map_keys, map_values, reserve_elements, set_entry,
    struct_field, write_null_row,
};
use crate::{DuckResult, DuckValueWriter, duck_error};
use libduckdb_sys::duckdb_vector;

use super::dynamic_row::DuckDynamicRow;
use super::dynamic_value::DuckDynamicValue;
use super::type_desc::DuckTypeDesc;

/// 一列的（可能嵌套的）写入器树：按列描述构建，负责把一批动态值写进该列向量。
///
/// A (possibly nested) writer tree for one column: built from the column description, it writes a
/// batch of dynamic values into that column's vector.
///
/// 叶子（标量 / `DECIMAL`）只有 [`DuckValueWriter`]；`LIST` / `STRUCT` / `MAP` 额外挂子写入器，
/// 与 `duck_list.rs` / `duck_map.rs` / `duck_struct.rs` 的静态实现保持同一套物理写法。
///
/// A leaf (scalar / `DECIMAL`) holds just a [`DuckValueWriter`]; `LIST` / `STRUCT` / `MAP` also
/// carry child writers, mirroring the static implementations in `duck_list.rs` / `duck_map.rs` /
/// `duck_struct.rs`.
pub(super) struct DynColumnWriter {
    /// 本列的底层写入器。
    ///
    /// The underlying writer of this column.
    writer: DuckValueWriter,
    /// 本列的类型描述。
    ///
    /// The type description of this column.
    desc: DuckTypeDesc,
    /// 子写入器：`LIST` 一个、`STRUCT` 每个字段一个、`MAP` 键/值各一个。
    ///
    /// Child writers: one for `LIST`, one per `STRUCT` field, and one each for a `MAP`'s key and
    /// value.
    children: Vec<Self>,
    /// `LIST` / `MAP` 已写入的子元素个数（也即下一个元素的起始偏移）。
    ///
    /// The number of child elements already written for a `LIST` / `MAP` (also the start offset of
    /// the next one).
    offset: usize,
}

impl DynColumnWriter {
    /// 按列描述与整批值构建写入器树。
    ///
    /// Builds the writer tree from the column description and the whole batch of values.
    pub(super) fn prepare(
        vector: duckdb_vector,
        desc: &DuckTypeDesc,
        values: &[Option<&DuckDynamicValue>],
    ) -> Self {
        match desc {
            DuckTypeDesc::Scalar(_) | DuckTypeDesc::Decimal { .. } => Self {
                writer: DuckValueWriter::new_from_vector(vector),
                desc: desc.clone(),
                children: Vec::new(),
                offset: 0,
            },
            DuckTypeDesc::List(element) => {
                // LIST：先按本批元素总数 reserve 子向量，再递归构建元素写入器。
                //
                // LIST: reserve the child vector for the whole batch, then recurse.
                let total: usize = values
                    .iter()
                    .flatten()
                    .filter_map(|value| match value {
                        DuckDynamicValue::List(items) => Some(items.len()),
                        _ => None,
                    })
                    .sum();
                reserve_elements(vector, total);
                let child_vector = element_child_vector(vector);
                let child_values: Vec<Option<&DuckDynamicValue>> = values
                    .iter()
                    .flatten()
                    .filter_map(|value| match value {
                        DuckDynamicValue::List(items) => Some(items.as_slice()),
                        _ => None,
                    })
                    .flatten()
                    .map(Option::as_ref)
                    .collect();
                let child = Self::prepare(child_vector, element, &child_values);
                Self {
                    writer: DuckValueWriter::new_from_vector(vector),
                    desc: desc.clone(),
                    children: vec![child],
                    offset: 0,
                }
            }
            DuckTypeDesc::Struct(fields) => {
                let children = fields
                    .iter()
                    .enumerate()
                    .map(|(index, (_, field_desc))| {
                        let child_vector = struct_field(vector, index);
                        let child_values = column_struct_values(values, index);
                        Self::prepare(child_vector, field_desc, &child_values)
                    })
                    .collect();
                Self {
                    writer: DuckValueWriter::new_from_vector(vector),
                    desc: desc.clone(),
                    children,
                    offset: 0,
                }
            }
            DuckTypeDesc::Map(key_desc, value_desc) => {
                // MAP 物理上是 LIST(STRUCT(key, value))：外层 entry + 键/值两个子向量。
                //
                // A MAP is physically LIST(STRUCT(key, value)): an outer entry plus two child
                // vectors for keys and values.
                let total: usize = values
                    .iter()
                    .flatten()
                    .filter_map(|value| match value {
                        DuckDynamicValue::Map(pairs) => Some(pairs.len()),
                        _ => None,
                    })
                    .sum();
                reserve_elements(vector, total);
                let keys_vector = map_keys(vector);
                let values_vector = map_values(vector);
                let mut key_values: Vec<Option<&DuckDynamicValue>> = Vec::new();
                let mut entry_values: Vec<Option<&DuckDynamicValue>> = Vec::new();
                for value in values.iter().flatten() {
                    if let DuckDynamicValue::Map(pairs) = value {
                        for (key, map_value) in pairs {
                            key_values.push(Some(key));
                            entry_values.push(Some(map_value));
                        }
                    }
                }
                let key_writer = Self::prepare(keys_vector, key_desc, &key_values);
                let value_writer = Self::prepare(values_vector, value_desc, &entry_values);
                Self {
                    writer: DuckValueWriter::new_from_vector(vector),
                    desc: desc.clone(),
                    children: vec![key_writer, value_writer],
                    offset: 0,
                }
            }
        }
    }

    /// 写一个非 NULL 值。
    ///
    /// Writes one non-NULL value.
    pub(super) fn write_value(&mut self, idx: usize, value: &DuckDynamicValue) -> DuckResult<()> {
        match value {
            DuckDynamicValue::List(items) => {
                if self.children.len() != 1 {
                    return Err(duck_error(
                        "dynamic column: LIST value written through a non-LIST column writer",
                    ));
                }
                let offset = self.offset;
                set_entry(self.writer.c_duckdb_vector, idx, offset, items.len());
                for (index, item) in items.iter().enumerate() {
                    match item {
                        Some(item) => self.children[0].write_value(offset + index, item)?,
                        None => self.children[0].write_null(offset + index)?,
                    }
                }
                self.offset += items.len();
                Ok(())
            }
            DuckDynamicValue::Struct(values) => {
                if self.children.len() != values.len() {
                    return Err(duck_error(format!(
                        "dynamic column: STRUCT has {} fields but the column declares {}",
                        values.len(),
                        self.children.len()
                    )));
                }
                for (index, value) in values.iter().enumerate() {
                    match value {
                        Some(value) => self.children[index].write_value(idx, value)?,
                        None => self.children[index].write_null(idx)?,
                    }
                }
                Ok(())
            }
            DuckDynamicValue::Map(pairs) => {
                if self.children.len() != 2 {
                    return Err(duck_error(
                        "dynamic column: MAP value written through a non-MAP column writer",
                    ));
                }
                let offset = self.offset;
                set_entry(self.writer.c_duckdb_vector, idx, offset, pairs.len());
                for (index, (key, map_value)) in pairs.iter().enumerate() {
                    self.children[0].write_value(offset + index, key)?;
                    self.children[1].write_value(offset + index, map_value)?;
                }
                self.offset += pairs.len();
                Ok(())
            }
            scalar => scalar.write_scalar(&mut self.writer, idx),
        }
    }

    /// 写一个 NULL 值。
    ///
    /// Writes one NULL value.
    ///
    /// - 标量 / `DECIMAL`：只把本向量置空；
    /// - `STRUCT`：本向量置空之外递归把子字段置空（`struct_extract` 不检查父 validity）；
    /// - `LIST` / `MAP`：本向量置空之外写一个显式空 entry `(0, 0)`（与静态实现一致）。
    ///
    /// - scalars / `DECIMAL`: mark this vector NULL only;
    /// - `STRUCT`: mark this vector NULL and recurse into the children (`struct_extract` does not
    ///   check the parent validity);
    /// - `LIST` / `MAP`: mark this vector NULL and write an explicit empty entry `(0, 0)`, matching
    ///   the static implementation.
    pub(super) fn write_null(&mut self, idx: usize) -> DuckResult<()> {
        match &self.desc {
            DuckTypeDesc::Scalar(_) | DuckTypeDesc::Decimal { .. } => unsafe {
                self.writer.vector_writer.set_null(idx);
            },
            DuckTypeDesc::Struct(_) => {
                unsafe { self.writer.vector_writer.set_null(idx) };
                for child in &mut self.children {
                    child.write_null(idx)?;
                }
            }
            DuckTypeDesc::List(_) | DuckTypeDesc::Map(_, _) => {
                write_null_row(&mut self.writer, idx);
            }
        }
        Ok(())
    }

    /// 收尾：`LIST` / `MAP` 把子向量长度收窄到已写元素数。
    ///
    /// Finishes: `LIST` / `MAP` clamp their child-vector sizes to the number of elements written.
    pub(super) fn finish(&mut self) {
        match &self.desc {
            DuckTypeDesc::Scalar(_) | DuckTypeDesc::Decimal { .. } => {}
            DuckTypeDesc::Struct(_) => {
                for child in &mut self.children {
                    child.finish();
                }
            }
            DuckTypeDesc::List(_) => {
                self.children[0].finish();
                finish_elements(self.writer.c_duckdb_vector, self.offset);
            }
            DuckTypeDesc::Map(_, _) => {
                self.children[0].finish();
                self.children[1].finish();
                finish_elements(self.writer.c_duckdb_vector, self.offset);
            }
        }
    }
}

/// 取一批行在第 `column_index` 列上的值（用于构建该列的写入器）。
///
/// Extracts the values of column `column_index` across a batch (used to build that column's
/// writer).
pub(super) fn column_values<'a>(
    rows: &'a [Option<&'a DuckDynamicRow>],
    column_index: usize,
) -> Vec<Option<&'a DuckDynamicValue>> {
    rows.iter()
        .map(|row| row.and_then(|row| row.values.get(column_index)).and_then(Option::as_ref))
        .collect()
}

/// 取一批行在第 `field_index` 个 `STRUCT` 字段上的值。
///
/// Extracts the values of `STRUCT` field `field_index` across a batch.
fn column_struct_values<'a>(
    values: &[Option<&'a DuckDynamicValue>],
    field_index: usize,
) -> Vec<Option<&'a DuckDynamicValue>> {
    values
        .iter()
        .map(|value| {
            value.and_then(|value| match value {
                DuckDynamicValue::Struct(fields) => {
                    fields.get(field_index).and_then(Option::as_ref)
                }
                _ => None,
            })
        })
        .collect()
}
