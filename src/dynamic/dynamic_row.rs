//! 动态行：`DuckDynamicRow`。
//!
//! Dynamic rows: `DuckDynamicRow`.

use crate::{DuckResult, DuckValueReader, duck_error};
use quack_rs::prelude::DataChunk;

use super::dynamic_value::DuckDynamicValue;
use super::result_schema::DuckResultSchema;
use super::dyn_column_writer::{DynColumnWriter, column_values};
use super::reader::prepare_dynamic_reader;

/// 动态行：一行的各列值，顺序与 [`DuckResultSchema`] 一致。
///
/// A dynamic row: one value per column, in the same order as the [`DuckResultSchema`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DuckDynamicRow {
    /// 每列的值；`None` 表示该单元格是 SQL NULL。
    ///
    /// One value per column; `None` means that cell is SQL NULL.
    pub(super) values: Vec<Option<DuckDynamicValue>>,
}

impl DuckDynamicRow {
    /// 由各列值构造一行。
    ///
    /// Builds a row from one value per column.
    #[must_use]
    pub fn new(values: Vec<Option<DuckDynamicValue>>) -> Self {
        Self { values }
    }

    /// 各列值。
    ///
    /// The values of every column.
    #[must_use]
    pub fn values(&self) -> &[Option<DuckDynamicValue>] {
        &self.values
    }

    /// 取第 `index` 列的值。
    ///
    /// Returns the value of column `index`.
    #[must_use]
    pub fn value(&self, index: usize) -> Option<&DuckDynamicValue> {
        self.values.get(index).and_then(Option::as_ref)
    }

    /// 行内列数。
    ///
    /// The number of cells in the row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 是否是空行（没有列）。
    ///
    /// Whether the row has no cells at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// 把一批动态行按 `schema` 写进输出 `chunk`（表函数 scan 阶段的批量写出）。
    ///
    /// Writes a batch of dynamic rows into the output `chunk` according to `schema` (the batch
    /// write used by the table-function scan phase).
    ///
    /// 流程与静态结构体一致：
    /// 1. 先按 `schema` 构建每一列的写入器树（`LIST` / `MAP` 顺带 reserve 子向量）；
    /// 2. 逐行、逐列写值，NULL 单元格走 `set_null`（`STRUCT` 递归置空，`LIST` / `MAP` 额外写
    ///    空 entry）；
    /// 3. 最后 `finish`，给 `LIST` / `MAP` 的子向量 `set_size`。
    ///
    /// 写之前会按列描述递归校验值的类型：类型错配返回行级错误，而不是写坏向量。
    ///
    /// The flow matches the static-struct path: build a writer tree per column from `schema`
    /// (`LIST` / `MAP` reserve their child vectors along the way), write value by value with NULL
    /// cells going through `set_null` (`STRUCT` recurses into children, `LIST` / `MAP` also write
    /// an empty entry), then `finish` so `LIST` / `MAP` set their child-vector sizes. Values are
    /// validated against the column description first: a mismatch becomes a row-level error
    /// instead of a corrupted vector.
    ///
    /// # Errors
    ///
    /// 行的列数或某列的值类型与 `schema` 不一致时返回错误。
    ///
    /// Returns an error when a row's cell count or a cell's type does not match `schema`.
    pub fn write_batch(
        chunk: &DataChunk,
        schema: &DuckResultSchema,
        rows: &[Option<&Self>],
    ) -> DuckResult<()> {
        let columns = schema.columns();

        // 1) 先校验，再写向量：类型错配必须变成可读错误，而不是未定义行为。
        //
        // 1) Validate before touching the vectors: a type mismatch must become a readable error,
        //    not undefined behaviour.
        for (row_index, row) in rows.iter().enumerate() {
            let Some(row) = row else {
                continue;
            };
            if row.values.len() != columns.len() {
                return Err(duck_error(format!(
                    "dynamic column: row {row_index} has {} columns but the schema \
                     declares {}",
                    row.values.len(),
                    columns.len()
                )));
            }
            for (column_index, (name, desc)) in columns.iter().enumerate() {
                if let Some(value) = &row.values[column_index] {
                    if !desc.matches_value(value) {
                        return Err(duck_error(format!(
                            "dynamic column: row {row_index} column `{name}` expects \
                             {desc} but got a value of type {}",
                            value.scalar_type_desc()
                        )));
                    }
                }
            }
        }

        // 2) 按 schema 构建每列的写入器树；LIST / MAP 在这一步 reserve 子向量。
        //
        // 2) Build one writer tree per column from the schema; LIST / MAP reserve child vectors
        //    here.
        let mut writers: Vec<DynColumnWriter> = columns
            .iter()
            .enumerate()
            .map(|(column_index, (_, desc))| {
                let vector = unsafe { chunk.vector(column_index) };
                let values = column_values(rows, column_index);
                DynColumnWriter::prepare(vector, desc, &values)
            })
            .collect();

        // 3) 逐行写值。
        //
        // 3) Write every row.
        for (row_index, row) in rows.iter().enumerate() {
            let Some(row) = row else {
                for writer in &mut writers {
                    writer.write_null(row_index)?;
                }
                continue;
            };
            for (column_index, writer) in writers.iter_mut().enumerate() {
                match &row.values[column_index] {
                    Some(value) => writer.write_value(row_index, value)?,
                    None => writer.write_null(row_index)?,
                }
            }
        }

        // 4) 收尾：LIST / MAP 的子向量 set_size。
        //
        // 4) Finish: LIST / MAP set their child-vector sizes.
        for writer in &mut writers {
            writer.finish();
        }
        Ok(())
    }

    /// 按 `schema` 把一个数据块读成动态行 —— [`Self::write_batch`] 的逆向操作。
    ///
    /// Reads a whole data chunk into dynamic rows according to `schema` — the inverse of
    /// [`Self::write_batch`].
    ///
    /// `COPY ... TO` 的 sink 阶段用它把每个数据块交给用户；凡是能从 schema 描述出来的列
    /// （标量、`DECIMAL`、`LIST`、`STRUCT`、`MAP`）都能读，嵌套任意深度。每列只建一次读取器
    /// （子读取器按同一份描述预先建好），随后按行复用，因此不会每行重复分配。
    ///
    /// The sink phase of `COPY ... TO` uses it to hand each data chunk to the user. Any column the
    /// schema can describe (scalars, `DECIMAL`, `LIST`, `STRUCT`, `MAP`) is readable, nested to any
    /// depth. One reader is built per column (with its child readers prepared from the same
    /// description) and then reused for every row, so nothing is re-allocated per row.
    ///
    /// # Errors
    ///
    /// 数据块的列数少于 schema、或某列的类型读不出来时返回错误。
    ///
    /// Returns an error when the chunk has fewer columns than the schema, or when a column's type
    /// cannot be read.
    pub fn read_batch(
        chunk: &DataChunk,
        schema: &DuckResultSchema,
    ) -> DuckResult<Vec<DuckDynamicRow>> {
        let columns = schema.columns();
        let size = chunk.size();
        if chunk.column_count() < columns.len() {
            return Err(duck_error(format!(
                "dynamic value: the chunk has {} columns but the schema declares {}",
                chunk.column_count(),
                columns.len()
            )));
        }

        // 1) 每列建一次读取器（含子读取器）。
        //
        // 1) Build one reader per column (child readers included).
        let mut readers = Vec::with_capacity(columns.len());
        for (index, (_, desc)) in columns.iter().enumerate() {
            let vector = unsafe { chunk.vector(index) };
            let mut reader = DuckValueReader::new_from_vector(vector, size);
            prepare_dynamic_reader(&mut reader, desc)?;
            readers.push(reader);
        }

        // 2) 逐行读。
        //
        // 2) Read every row.
        let mut rows = Vec::with_capacity(size);
        for row in 0..size {
            let mut cells = Vec::with_capacity(columns.len());
            for (index, reader) in readers.iter().enumerate() {
                cells.push(DuckDynamicValue::read_cell(reader, row, &columns[index].1)?);
            }
            rows.push(Self::new(cells));
        }
        Ok(rows)
    }
}

impl From<Vec<Option<DuckDynamicValue>>> for DuckDynamicRow {
    /// `Vec<Option<DuckDynamicValue>>` → 动态行。
    ///
    /// `Vec<Option<DuckDynamicValue>>` → dynamic row.
    fn from(values: Vec<Option<DuckDynamicValue>>) -> Self {
        Self::new(values)
    }
}
