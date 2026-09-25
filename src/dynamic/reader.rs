//! 读取器树的构建辅助：`prepare_dynamic_reader` / `child_reader_at`。
//!
//! Reader-tree construction helpers: `prepare_dynamic_reader` / `child_reader_at`.

use crate::value_types::vector_layout::{
    element_child_size, element_child_vector, map_keys, map_total_entries, map_values, struct_field,
};
use crate::{DuckResult, DuckValueReader, duck_error};

use super::type_desc::DuckTypeDesc;

/// 取第 `index` 个子读取器；缺失时报错（说明读取器与类型描述不同构）。
///
/// Returns child reader `index`, or an error when it is missing (the reader and the type
/// description are not isomorphic).
pub(super) fn child_reader_at<'a>(
    reader: &'a DuckValueReader,
    index: usize,
    what: &str,
) -> DuckResult<&'a DuckValueReader> {
    reader.child_reader.get(index).ok_or_else(|| {
        duck_error(format!(
            "dynamic value: no child reader for {what} at index {index}; was the reader prepared \
             for this type description?"
        ))
    })
}

/// 按类型描述为读取器准备子读取器（`LIST` 元素 / `STRUCT` 字段 / `MAP` 键值），递归。
///
/// Prepares a reader's child readers (`LIST` elements, `STRUCT` fields, `MAP` keys and values) from
/// a type description, recursively.
///
/// [`DuckDynamicValue::read_cell`](super::dynamic_value::DuckDynamicValue::read_cell) 完全由
/// `desc` 驱动递归读取，因此读取器树必须与描述严格同构；这里是那棵树唯一的构建入口 —— 静态侧
/// `DuckValueType::create_reader_from_vector` 的动态对应物。
///
/// [`DuckDynamicValue::read_cell`](super::dynamic_value::DuckDynamicValue::read_cell) reads
/// recursively, driven entirely by `desc`, so the reader tree must mirror that description exactly:
/// this is the single place it is built — the dynamic counterpart of the static
/// `DuckValueType::create_reader_from_vector`.
///
/// 标量（含 `DECIMAL`）不需要子读取器，因此这里不校验它们的可读性：`DuckTypeDesc` 里正常的标量
/// 分支都来自 [`DuckTypeDesc::from_logical_type`]，本就只有可读的类型。手写出的不可读标量
/// （例如 `Scalar(TypeId::Bit)`）会在第一次读该列时报错。
///
/// Scalars (including `DECIMAL`) need no child reader, so their readability is not checked here:
/// every scalar branch a `DuckTypeDesc` normally carries comes from
/// [`DuckTypeDesc::from_logical_type`], which only admits readable types. A hand-built unreadable
/// scalar (say `Scalar(TypeId::Bit)`) reports its error on the first read of that column instead.
///
/// # Errors
///
/// 子读取器树无法按 `desc` 建出来时返回错误。
///
/// Returns an error when the child-reader tree cannot be built from `desc`.
pub(super) fn prepare_dynamic_reader(
    reader: &mut DuckValueReader,
    desc: &DuckTypeDesc,
) -> DuckResult<()> {
    let row_count = reader.vector_reader.row_count();
    match desc {
        DuckTypeDesc::Scalar(_) | DuckTypeDesc::Decimal { .. } => Ok(()),
        DuckTypeDesc::List(element) => {
            let vector = element_child_vector(reader.c_duckdb_vector);
            let size = element_child_size(reader.c_duckdb_vector);
            let mut child = DuckValueReader::new_from_vector(vector, size);
            prepare_dynamic_reader(&mut child, element)?;
            reader.child_reader = vec![child];
            Ok(())
        }
        DuckTypeDesc::Struct(fields) => {
            let mut children = Vec::with_capacity(fields.len());
            for (index, (_, field_desc)) in fields.iter().enumerate() {
                let vector = struct_field(reader.c_duckdb_vector, index);
                let mut child = DuckValueReader::new_from_vector(vector, row_count);
                prepare_dynamic_reader(&mut child, field_desc)?;
                children.push(child);
            }
            reader.child_reader = children;
            Ok(())
        }
        DuckTypeDesc::Map(key_desc, value_desc) => {
            let total = map_total_entries(reader.c_duckdb_vector);
            let mut key_reader =
                DuckValueReader::new_from_vector(map_keys(reader.c_duckdb_vector), total);
            prepare_dynamic_reader(&mut key_reader, key_desc)?;
            let mut value_reader =
                DuckValueReader::new_from_vector(map_values(reader.c_duckdb_vector), total);
            prepare_dynamic_reader(&mut value_reader, value_desc)?;
            reader.child_reader = vec![key_reader, value_reader];
            Ok(())
        }
    }
}
