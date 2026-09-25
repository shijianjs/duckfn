//! 运行时值：`DuckDynamicValue`。
//!
//! Runtime values: `DuckDynamicValue`.

use crate::value_types::vector_layout::{element_entry, map_entry};
use crate::{
    DuckResult, DuckValueReader, DuckValueType, DuckValueWriter, duck_error, duck_value_is_null,
};
use quack_rs::interval::DuckInterval;
use quack_rs::prelude::{TypeId, Value, VectorReader};

use super::type_desc::DuckTypeDesc;
use super::reader::child_reader_at;

/// 运行时动态值：携带数据、不携带类型；类型由 [`DuckTypeDesc`] 给出。
///
/// A runtime dynamic value: it carries data, not types; the type comes from [`DuckTypeDesc`].
///
/// 标量分支与 duckfn 已有的 [`DuckValueType`] 映射一一对应（`BOOLEAN` / 各宽度整数 / `FLOAT` /
/// `DOUBLE` / `VARCHAR` / `BLOB` / 日期时间 / `UUID` / `INTERVAL` / `DECIMAL`），嵌套分支对应
/// `LIST` / `STRUCT` / `MAP`。每个 `Option` 槽位表示 SQL NULL。
///
/// The scalar variants mirror duckfn's existing [`DuckValueType`] mappings (`BOOLEAN`, every
/// integer width, `FLOAT` / `DOUBLE`, `VARCHAR`, `BLOB`, the datetime wrappers, `UUID`,
/// `INTERVAL`, `DECIMAL`); the nested variants stand for `LIST` / `STRUCT` / `MAP`. Every `Option`
/// slot represents SQL NULL.
#[derive(Debug, Clone, PartialEq)]
pub enum DuckDynamicValue {
    /// `BOOLEAN`。
    ///
    /// `BOOLEAN`.
    Boolean(bool),
    /// `TINYINT`。
    ///
    /// `TINYINT`.
    TinyInt(i8),
    /// `SMALLINT`。
    ///
    /// `SMALLINT`.
    SmallInt(i16),
    /// `INTEGER`。
    ///
    /// `INTEGER`.
    Integer(i32),
    /// `BIGINT`。
    ///
    /// `BIGINT`.
    BigInt(i64),
    /// `HUGEINT`。
    ///
    /// `HUGEINT`.
    HugeInt(i128),
    /// `UTINYINT`。
    ///
    /// `UTINYINT`.
    UTinyInt(u8),
    /// `USMALLINT`。
    ///
    /// `USMALLINT`.
    USmallInt(u16),
    /// `UINTEGER`。
    ///
    /// `UINTEGER`.
    UInteger(u32),
    /// `UBIGINT`。
    ///
    /// `UBIGINT`.
    UBigInt(u64),
    /// `UHUGEINT`。
    ///
    /// `UHUGEINT`.
    UHugeInt(u128),
    /// `FLOAT`。
    ///
    /// `FLOAT`.
    Float(f32),
    /// `DOUBLE`。
    ///
    /// `DOUBLE`.
    Double(f64),
    /// `VARCHAR`。
    ///
    /// `VARCHAR`.
    Varchar(String),
    /// `BLOB`。
    ///
    /// `BLOB`.
    Blob(Vec<u8>),
    /// `DATE`（自 1970-01-01 起的天数）。
    ///
    /// `DATE` (days since 1970-01-01).
    Date(i32),
    /// `TIME`（自 00:00:00 起的微秒数）。
    ///
    /// `TIME` (microseconds since 00:00:00).
    Time(i64),
    /// `TIME WITH TIME ZONE`（位编码）。
    ///
    /// `TIME WITH TIME ZONE` (bit-packed).
    TimeTz(u64),
    /// `TIMESTAMP`（自纪元的微秒数）。
    ///
    /// `TIMESTAMP` (microseconds since the epoch).
    Timestamp(i64),
    /// `TIMESTAMP WITH TIME ZONE`（自纪元的微秒数，UTC）。
    ///
    /// `TIMESTAMP WITH TIME ZONE` (microseconds since the epoch, UTC).
    TimestampTz(i64),
    /// `TIMESTAMP_S`。
    ///
    /// `TIMESTAMP_S`.
    TimestampS(i64),
    /// `TIMESTAMP_MS`。
    ///
    /// `TIMESTAMP_MS`.
    TimestampMs(i64),
    /// `TIMESTAMP_NS`。
    ///
    /// `TIMESTAMP_NS`.
    TimestampNs(i64),
    /// `UUID`。
    ///
    /// `UUID`.
    Uuid(u128),
    /// `INTERVAL`。
    ///
    /// `INTERVAL`.
    Interval(DuckInterval),
    /// `DECIMAL(width, scale)`，值为未缩放整数。
    ///
    /// `DECIMAL(width, scale)`; the value is the unscaled integer.
    Decimal {
        /// 有效数字总位数（必须与列描述一致）。
        ///
        /// Total number of significant digits (must match the column description).
        width: u8,
        /// 小数点后的位数（必须与列描述一致）。
        ///
        /// Number of digits after the decimal point (must match the column description).
        scale: u8,
        /// 未缩放整数值；真实值是 `unscaled / 10^scale`。
        ///
        /// The unscaled integer; the real value is `unscaled / 10^scale`.
        unscaled: i128,
    },
    /// `LIST`：元素可为 NULL。
    ///
    /// `LIST`; elements may be NULL.
    List(Vec<Option<DuckDynamicValue>>),
    /// `STRUCT`：字段顺序与列描述的字段顺序一致。
    ///
    /// `STRUCT`; the field order matches the column description.
    Struct(Vec<Option<DuckDynamicValue>>),
    /// `MAP`：键不为 NULL，值可为 NULL。
    ///
    /// `MAP`; keys are never NULL, values may be.
    Map(Vec<(DuckDynamicValue, DuckDynamicValue)>),
}

impl DuckDynamicValue {
    /// 构造一个 `LIST` 值。
    ///
    /// Builds a `LIST` value.
    #[must_use]
    pub fn list<I: IntoIterator<Item = Option<Self>>>(items: I) -> Self {
        Self::List(items.into_iter().collect())
    }

    /// 构造一个 `STRUCT` 值（字段顺序须与列描述一致）。
    ///
    /// Builds a `STRUCT` value (the field order must match the column description).
    #[must_use]
    pub fn struct_value<I: IntoIterator<Item = Option<Self>>>(fields: I) -> Self {
        Self::Struct(fields.into_iter().collect())
    }

    /// 构造一个 `MAP` 值。
    ///
    /// Builds a `MAP` value.
    #[must_use]
    pub fn map<I: IntoIterator<Item = (Self, Self)>>(pairs: I) -> Self {
        Self::Map(pairs.into_iter().collect())
    }

    /// 本值对应的 DuckDB 类型 id（嵌套值为 `LIST` / `STRUCT` / `MAP`）。
    ///
    /// The DuckDB type id of this value (`LIST` / `STRUCT` / `MAP` for nested ones).
    #[must_use]
    pub fn type_id(&self) -> TypeId {
        match self {
            Self::Boolean(_) => TypeId::Boolean,
            Self::TinyInt(_) => TypeId::TinyInt,
            Self::SmallInt(_) => TypeId::SmallInt,
            Self::Integer(_) => TypeId::Integer,
            Self::BigInt(_) => TypeId::BigInt,
            Self::HugeInt(_) => TypeId::HugeInt,
            Self::UTinyInt(_) => TypeId::UTinyInt,
            Self::USmallInt(_) => TypeId::USmallInt,
            Self::UInteger(_) => TypeId::UInteger,
            Self::UBigInt(_) => TypeId::UBigInt,
            Self::UHugeInt(_) => TypeId::UHugeInt,
            Self::Float(_) => TypeId::Float,
            Self::Double(_) => TypeId::Double,
            Self::Varchar(_) => TypeId::Varchar,
            Self::Blob(_) => TypeId::Blob,
            Self::Date(_) => TypeId::Date,
            Self::Time(_) => TypeId::Time,
            Self::TimeTz(_) => TypeId::TimeTz,
            Self::Timestamp(_) => TypeId::Timestamp,
            Self::TimestampTz(_) => TypeId::TimestampTz,
            Self::TimestampS(_) => TypeId::TimestampS,
            Self::TimestampMs(_) => TypeId::TimestampMs,
            Self::TimestampNs(_) => TypeId::TimestampNs,
            Self::Uuid(_) => TypeId::Uuid,
            Self::Interval(_) => TypeId::Interval,
            Self::Decimal { .. } => TypeId::Decimal,
            Self::List(_) => TypeId::List,
            Self::Struct(_) => TypeId::Struct,
            Self::Map(_) => TypeId::Map,
        }
    }

    /// 标量值的类型 id；嵌套值返回 `None`。
    ///
    /// The type id of a scalar value; `None` for nested values.
    #[must_use]
    pub(super) fn scalar_type_id(&self) -> Option<TypeId> {
        match self {
            Self::List(_) | Self::Struct(_) | Self::Map(_) => None,
            _ => Some(self.type_id()),
        }
    }

    /// 标量值的类型描述；嵌套值退化成 `Scalar(LIST/STRUCT/MAP)`（仅用于错误信息兜底）。
    ///
    /// The type description of a scalar value; nested values degrade to
    /// `Scalar(LIST/STRUCT/MAP)` (only as a fallback for error messages).
    #[must_use]
    pub(super) fn scalar_type_desc(&self) -> DuckTypeDesc {
        match self {
            Self::Decimal { width, scale, .. } => DuckTypeDesc::Decimal {
                width: *width,
                scale: *scale,
            },
            other => DuckTypeDesc::Scalar(other.type_id()),
        }
    }

    /// 从 DuckDB [`Value`]（bind 参数、外部自描述数据）读出动态值；NULL 返回 `Ok(None)`。
    ///
    /// Reads a dynamic value out of a DuckDB [`Value`] (bind arguments, self-describing external
    /// data); NULL yields `Ok(None)`.
    ///
    /// # Errors
    ///
    /// 值的实际类型与 `desc` 不一致、或 `desc` 是不支持从 `Value` 读取的形状时返回错误。
    ///
    /// Returns an error when the value's actual type does not match `desc`, or when `desc` is a
    /// shape that cannot be read from a `Value`.
    pub fn from_duck_value(value: &Value, desc: &DuckTypeDesc) -> DuckResult<Option<Self>> {
        // 不能只判 `value.is_null()`：SQL 里显式写 `arg = NULL` 时句柄并非空指针，
        // 只判句柄会漏掉，随后按类型取值会撞上 FFI 的 foreign exception。
        //
        // Do not rely on `value.is_null()` alone: an explicit `arg = NULL` in SQL yields a non-null
        // handle, and the type-specific read would then hit a foreign exception inside the FFI.
        if duck_value_is_null(value) {
            return Ok(None);
        }
        let dynamic = match desc {
            DuckTypeDesc::Scalar(TypeId::Boolean) => Self::Boolean(value.as_bool()),
            DuckTypeDesc::Scalar(TypeId::TinyInt) => Self::TinyInt(value.as_i8()),
            DuckTypeDesc::Scalar(TypeId::SmallInt) => Self::SmallInt(value.as_i16()),
            DuckTypeDesc::Scalar(TypeId::Integer) => Self::Integer(value.as_i32()),
            DuckTypeDesc::Scalar(TypeId::BigInt) => Self::BigInt(value.as_i64()),
            DuckTypeDesc::Scalar(TypeId::HugeInt) => Self::HugeInt(value.as_i128()),
            DuckTypeDesc::Scalar(TypeId::UTinyInt) => Self::UTinyInt(value.as_u8()),
            DuckTypeDesc::Scalar(TypeId::USmallInt) => Self::USmallInt(value.as_u16()),
            DuckTypeDesc::Scalar(TypeId::UInteger) => Self::UInteger(value.as_u32()),
            DuckTypeDesc::Scalar(TypeId::UBigInt) => Self::UBigInt(value.as_u64()),
            DuckTypeDesc::Scalar(TypeId::UHugeInt) => Self::UHugeInt(value.as_u128()),
            DuckTypeDesc::Scalar(TypeId::Float) => Self::Float(value.as_f32()),
            DuckTypeDesc::Scalar(TypeId::Double) => Self::Double(value.as_f64()),
            DuckTypeDesc::Scalar(TypeId::Varchar) => Self::Varchar(value.as_str()?),
            DuckTypeDesc::Scalar(TypeId::Blob) => Self::Blob(value.as_blob()?),
            DuckTypeDesc::Scalar(TypeId::Date) => Self::Date(value.as_date()),
            DuckTypeDesc::Scalar(TypeId::Time) => Self::Time(value.as_time()),
            DuckTypeDesc::Scalar(TypeId::TimeTz) => Self::TimeTz(value.as_time_tz()),
            DuckTypeDesc::Scalar(TypeId::Timestamp) => Self::Timestamp(value.as_timestamp()),
            DuckTypeDesc::Scalar(TypeId::TimestampTz) => Self::TimestampTz(value.as_timestamp_tz()),
            DuckTypeDesc::Scalar(TypeId::TimestampS) => Self::TimestampS(value.as_timestamp_s()),
            DuckTypeDesc::Scalar(TypeId::TimestampMs) => Self::TimestampMs(value.as_timestamp_ms()),
            DuckTypeDesc::Scalar(TypeId::TimestampNs) => Self::TimestampNs(value.as_timestamp_ns()),
            DuckTypeDesc::Scalar(TypeId::Uuid) => Self::Uuid(value.as_uuid()),
            DuckTypeDesc::Scalar(TypeId::Interval) => Self::Interval(value.as_interval()),
            DuckTypeDesc::Decimal { .. } => {
                let decimal = value.as_decimal();
                Self::Decimal {
                    width: decimal.width,
                    scale: decimal.scale,
                    unscaled: decimal.value,
                }
            }
            DuckTypeDesc::List(element) => {
                let mut items = Vec::new();
                for child in value.list_items() {
                    items.push(Self::from_duck_value(&child, element)?);
                }
                Self::List(items)
            }
            DuckTypeDesc::Struct(fields) => {
                let mut values = Vec::with_capacity(fields.len());
                for (index, (_, field_desc)) in fields.iter().enumerate() {
                    match value.struct_child(index) {
                        Some(child) => values.push(Self::from_duck_value(&child, field_desc)?),
                        None => values.push(None),
                    }
                }
                Self::Struct(values)
            }
            DuckTypeDesc::Map(key_desc, value_desc) => {
                let mut pairs = Vec::with_capacity(value.map_len());
                for index in 0..value.map_len() {
                    let key = value
                        .map_key(index)
                        .map(|key| Self::from_duck_value(&key, key_desc))
                        .transpose()?
                        .flatten()
                        .ok_or_else(|| duck_error("dynamic column: MAP key cannot be null"))?;
                    let map_value = value
                        .map_value(index)
                        .map(|map_value| Self::from_duck_value(&map_value, value_desc))
                        .transpose()?
                        .flatten()
                        .ok_or_else(|| duck_error("dynamic column: MAP value cannot be null"))?;
                    pairs.push((key, map_value));
                }
                Self::Map(pairs)
            }
            DuckTypeDesc::Scalar(other) => {
                return Err(duck_error(format!(
                    "dynamic column: DuckTypeDesc `{other:?}` is not supported by \
                     DuckDynamicValue::from_duck_value"
                )));
            }
        };
        Ok(Some(dynamic))
    }

    /// 从一个向量读取器里读出一个动态值；该槽位是 SQL NULL 时返回 `Ok(None)`。
    ///
    /// Reads one dynamic value out of a vector reader; `Ok(None)` means the slot is SQL NULL.
    ///
    /// `desc` 是唯一真相：读取完全由它驱动，叶子按 `VectorReader::read_*` 分派，`LIST` /
    /// `STRUCT` / `MAP` 递归到 [`DuckValueReader::child_reader`]。因此调用前必须先用
    /// `prepare_dynamic_reader` 把读取器的子读取器按同一份 `desc` 建好。
    ///
    /// `desc` is the single source of truth: the read is driven entirely by it — leaves dispatch to
    /// `VectorReader::read_*`, while `LIST` / `STRUCT` / `MAP` recurse into
    /// [`DuckValueReader::child_reader`]. The child readers must therefore have been built for the
    /// same `desc` by `prepare_dynamic_reader` beforehand.
    ///
    /// # Errors
    ///
    /// `desc` 描述的类型读不出来（例如 `ENUM` / `ARRAY` / `UNION` / `BIT`）、或子读取器缺失、
    /// 或 `MAP` 的键/值为 NULL 时返回错误。
    ///
    /// Returns an error when the described type cannot be read (`ENUM` / `ARRAY` / `UNION` / `BIT`),
    /// when a child reader is missing, or when a `MAP` key or value is NULL.
    pub fn read_cell(
        reader: &DuckValueReader,
        row: usize,
        desc: &DuckTypeDesc,
    ) -> DuckResult<Option<Self>> {
        // SAFETY: row 由调用方保证落在 `reader.vector_reader.row_count()` 之内。
        //
        // SAFETY: the caller guarantees `row` is within `reader.vector_reader.row_count()`.
        if !unsafe { reader.vector_reader.is_valid(row) } {
            return Ok(None);
        }

        let value = match desc {
            DuckTypeDesc::Scalar(type_id) => Self::read_scalar(&reader.vector_reader, row, *type_id)?,
            DuckTypeDesc::Decimal { width, scale } => Self::Decimal {
                width: *width,
                scale: *scale,
                unscaled: unsafe { reader.vector_reader.read_decimal(row, *width) },
            },
            DuckTypeDesc::List(element) => {
                let (offset, length) = element_entry(reader.c_duckdb_vector, row);
                let child = child_reader_at(reader, 0, "LIST element")?;
                let mut items = Vec::with_capacity(length);
                for index in 0..length {
                    items.push(Self::read_cell(child, offset + index, element)?);
                }
                Self::List(items)
            }
            DuckTypeDesc::Struct(fields) => {
                let mut values = Vec::with_capacity(fields.len());
                for (index, (name, field_desc)) in fields.iter().enumerate() {
                    let child = child_reader_at(reader, index, name)?;
                    values.push(Self::read_cell(child, row, field_desc)?);
                }
                Self::Struct(values)
            }
            DuckTypeDesc::Map(key_desc, value_desc) => {
                let (offset, length) = map_entry(reader.c_duckdb_vector, row);
                let keys = child_reader_at(reader, 0, "MAP key")?;
                let values = child_reader_at(reader, 1, "MAP value")?;
                let mut pairs = Vec::with_capacity(length);
                for index in 0..length {
                    let index = offset + index;
                    let key = Self::read_cell(keys, index, key_desc)?
                        .ok_or_else(|| duck_error("dynamic value: MAP key cannot be null"))?;
                    let value = Self::read_cell(values, index, value_desc)?
                        .ok_or_else(|| duck_error("dynamic value: MAP value cannot be null"))?;
                    pairs.push((key, value));
                }
                Self::Map(pairs)
            }
        };
        Ok(Some(value))
    }

    /// 读一个标量槽位（调用方已确认该行非 NULL）。
    ///
    /// Reads one scalar slot (the caller has already established that the row is not NULL).
    ///
    /// # Errors
    ///
    /// `type_id` 没有对应的读法时返回错误（`ENUM` / `ARRAY` / `UNION` / `BIT` 等）。
    ///
    /// Returns an error when `type_id` has no read path (`ENUM` / `ARRAY` / `UNION` / `BIT`, ...).
    fn read_scalar(raw: &VectorReader, row: usize, type_id: TypeId) -> DuckResult<Self> {
        // SAFETY: 每个 `read_*` 都要求「列类型与该方法一致」且该行非 NULL，两者分别由
        // type_id 分派与调用方的 is_valid 检查保证。
        //
        // SAFETY: every `read_*` requires the column type to match and the row to be non-NULL,
        // which the `type_id` dispatch and the caller's validity check guarantee.
        let value = match type_id {
            TypeId::Boolean => Self::Boolean(unsafe { raw.read_bool(row) }),
            TypeId::TinyInt => Self::TinyInt(unsafe { raw.read_i8(row) }),
            TypeId::SmallInt => Self::SmallInt(unsafe { raw.read_i16(row) }),
            TypeId::Integer => Self::Integer(unsafe { raw.read_i32(row) }),
            TypeId::BigInt => Self::BigInt(unsafe { raw.read_i64(row) }),
            TypeId::HugeInt => Self::HugeInt(unsafe { raw.read_i128(row) }),
            TypeId::UTinyInt => Self::UTinyInt(unsafe { raw.read_u8(row) }),
            TypeId::USmallInt => Self::USmallInt(unsafe { raw.read_u16(row) }),
            TypeId::UInteger => Self::UInteger(unsafe { raw.read_u32(row) }),
            TypeId::UBigInt => Self::UBigInt(unsafe { raw.read_u64(row) }),
            TypeId::UHugeInt => Self::UHugeInt(unsafe { raw.read_u128(row) }),
            TypeId::Float => Self::Float(unsafe { raw.read_f32(row) }),
            TypeId::Double => Self::Double(unsafe { raw.read_f64(row) }),
            TypeId::Varchar => Self::Varchar(unsafe { raw.read_str(row) }.to_owned()),
            TypeId::Blob => Self::Blob(unsafe { raw.read_blob(row) }.to_vec()),
            TypeId::Date => Self::Date(unsafe { raw.read_date(row) }),
            TypeId::Time => Self::Time(unsafe { raw.read_time(row) }),
            TypeId::TimeTz => Self::TimeTz(unsafe { raw.read_time_tz(row) }),
            TypeId::Timestamp => Self::Timestamp(unsafe { raw.read_timestamp(row) }),
            TypeId::TimestampTz => Self::TimestampTz(unsafe { raw.read_timestamp_tz(row) }),
            TypeId::TimestampS => Self::TimestampS(unsafe { raw.read_timestamp_s(row) }),
            TypeId::TimestampMs => Self::TimestampMs(unsafe { raw.read_timestamp_ms(row) }),
            TypeId::TimestampNs => Self::TimestampNs(unsafe { raw.read_timestamp_ns(row) }),
            TypeId::Uuid => Self::Uuid(unsafe { raw.read_uuid(row) }),
            TypeId::Interval => Self::Interval(unsafe { raw.read_interval(row) }),
            other => {
                return Err(duck_error(format!(
                    "dynamic value: DuckDB type `{}` cannot be read from a vector",
                    other.sql_name()
                )));
            }
        };
        Ok(value)
    }

    /// 文本渲染（展示 / 诊断用）：嵌套结构用 schema 里的字段名，便于人读。
    ///
    /// Text rendering (for display / diagnostics): nested structures use the schema's field names so
    /// they read like themselves.
    ///
    /// **它不是一种无歧义的往返编码**：字符串原样输出、容器里的 NULL 写作 `NULL`，因此
    /// `['NULL', NULL]` 与 `['NULL', 'NULL']` 会渲染成同一个字符串。自定义文件格式应当按自己的
    /// 转义约定递归渲染（`duckfn-quack/src/extension/functions/tsv_format.rs` 就是一个例子），而不是直接拿这份
    /// 文本去解析。
    ///
    /// **This is not an unambiguous round-trip encoding**: strings are written verbatim and a NULL
    /// inside a container becomes `NULL`, so `['NULL', NULL]` and `['NULL', 'NULL']` render
    /// identically. A custom file format should render recursively with its own escaping rules (see
    /// `duckfn-quack/src/extension/functions/tsv_format.rs` for an example) rather than parse this text back.
    ///
    /// 约定：
    /// - `VARCHAR` 原样输出（不做引号 / 转义，格式实现若需要转义请自行处理）；
    /// - `BLOB` 输出 `\xHH`（大写十六进制）；
    /// - 浮点一定带小数点（`1.0` 而不是 `1`）；
    /// - `DATE` / `TIME` / `TIMESTAMP*` / `UUID` / `INTERVAL` / `DECIMAL` 输出其**物理整数**
    ///   （就是 `DuckDynamicValue` 各分支里存的那个值），不做日历 / 小数格式化；
    /// - `LIST` → `[a, b]`，`STRUCT`（用 `desc` 给的名字）→ `{'k': v}`，`MAP` → `{k=v}`；
    ///   元素 / 字段 / 值为 NULL 时写 `NULL`；空容器写 `[]` / `{}`。
    ///
    /// Conventions: `VARCHAR` verbatim, `BLOB` as `\xHH`, floats always with a decimal point, the
    /// datetime / `UUID` / `INTERVAL` / `DECIMAL` wrappers as their **physical integer** (the value
    /// the enum branch stores), `LIST` as `[a, b]`, `STRUCT` (named by `desc`) as `{'k': v}` and
    /// `MAP` as `{k=v}`; NULL elements / fields / values become `NULL`, empty containers `[]` / `{}`.
    #[must_use]
    pub fn to_text(&self, desc: &DuckTypeDesc) -> String {
        let mut out = String::new();
        self.render(Some(desc), &mut out);
        out
    }

    /// 递归渲染实现；`desc` 为 `None` 时退化成「无 schema」渲染（`STRUCT` 只写位置）。
    ///
    /// The recursive rendering; with `desc` as `None` it degrades to a schema-less rendering (a
    /// `STRUCT` prints positions only).
    fn render(&self, desc: Option<&DuckTypeDesc>, out: &mut String) {
        match self {
            Self::Boolean(value) => out.push_str(if *value { "true" } else { "false" }),
            Self::TinyInt(value) => out.push_str(&value.to_string()),
            Self::SmallInt(value) => out.push_str(&value.to_string()),
            Self::Integer(value) => out.push_str(&value.to_string()),
            Self::BigInt(value) => out.push_str(&value.to_string()),
            Self::HugeInt(value) => out.push_str(&value.to_string()),
            Self::UTinyInt(value) => out.push_str(&value.to_string()),
            Self::USmallInt(value) => out.push_str(&value.to_string()),
            Self::UInteger(value) => out.push_str(&value.to_string()),
            Self::UBigInt(value) => out.push_str(&value.to_string()),
            Self::UHugeInt(value) => out.push_str(&value.to_string()),
            Self::Float(value) => push_float(*value as f64, out),
            Self::Double(value) => push_float(*value, out),
            Self::Varchar(value) => out.push_str(value),
            Self::Blob(value) => {
                for byte in value {
                    out.push_str(&format!("\\x{byte:02X}"));
                }
            }
            Self::Date(value) => out.push_str(&value.to_string()),
            Self::Time(value) => out.push_str(&value.to_string()),
            Self::TimeTz(value) => out.push_str(&value.to_string()),
            Self::Timestamp(value) => out.push_str(&value.to_string()),
            Self::TimestampTz(value) => out.push_str(&value.to_string()),
            Self::TimestampS(value) => out.push_str(&value.to_string()),
            Self::TimestampMs(value) => out.push_str(&value.to_string()),
            Self::TimestampNs(value) => out.push_str(&value.to_string()),
            Self::Uuid(value) => out.push_str(&value.to_string()),
            Self::Interval(value) => {
                out.push_str(&format!("{} {} {}", value.months, value.days, value.micros));
            }
            Self::Decimal { unscaled, .. } => out.push_str(&unscaled.to_string()),
            Self::List(items) => {
                let element = match desc {
                    Some(DuckTypeDesc::List(element)) => Some(element.as_ref()),
                    _ => None,
                };
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    match item {
                        Some(value) => value.render(element, out),
                        None => out.push_str("NULL"),
                    }
                }
                out.push(']');
            }
            Self::Struct(values) => {
                let fields = match desc {
                    Some(DuckTypeDesc::Struct(fields)) => Some(fields.as_slice()),
                    _ => None,
                };
                out.push('{');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    let (name, field_desc) = match fields.and_then(|fields| fields.get(index)) {
                        Some((name, field_desc)) => (Some(name.as_str()), Some(field_desc)),
                        None => (None, None),
                    };
                    if let Some(name) = name {
                        out.push('\'');
                        out.push_str(name);
                        out.push_str("': ");
                    }
                    match value {
                        Some(value) => value.render(field_desc, out),
                        None => out.push_str("NULL"),
                    }
                }
                out.push('}');
            }
            Self::Map(pairs) => {
                let (key_desc, value_desc) = match desc {
                    Some(DuckTypeDesc::Map(key, value)) => {
                        (Some(key.as_ref()), Some(value.as_ref()))
                    }
                    _ => (None, None),
                };
                out.push('{');
                for (index, (key, value)) in pairs.iter().enumerate() {
                    if index > 0 {
                        out.push_str(", ");
                    }
                    key.render(key_desc, out);
                    out.push('=');
                    value.render(value_desc, out);
                }
                out.push('}');
            }
        }
    }

    /// 把一个标量值写进向量；嵌套值由
    /// [`DynColumnWriter`](super::dyn_column_writer::DynColumnWriter) 处理，这里返回错误。
    ///
    /// Writes one scalar value into a vector; nested values are handled by
    /// [`DynColumnWriter`](super::dyn_column_writer::DynColumnWriter) and report an error here.
    pub(super) fn write_scalar(&self, writer: &mut DuckValueWriter, idx: usize) -> DuckResult<()> {
        // 标量分支直接复用各基础类型的 `DuckValueType` 写入实现，
        // 保证与静态表函数走的物理写入完全一致。
        //
        // Scalar branches reuse the existing `DuckValueType` write implementations, so the
        // physical write matches the static table-function path exactly.
        match self {
            Self::Boolean(value) => bool::write_valid_to_vector_writer(
                &mut writer.vector_writer,
                idx,
                value,
            ),
            Self::TinyInt(value) => {
                i8::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::SmallInt(value) => {
                i16::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::Integer(value) => {
                i32::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::BigInt(value) => {
                i64::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::HugeInt(value) => {
                i128::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::UTinyInt(value) => {
                u8::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::USmallInt(value) => {
                u16::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::UInteger(value) => {
                u32::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::UBigInt(value) => {
                u64::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::UHugeInt(value) => {
                u128::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::Float(value) => {
                f32::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::Double(value) => {
                f64::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::Varchar(value) => {
                String::write_valid_to_vector_writer(&mut writer.vector_writer, idx, value)
            }
            Self::Blob(value) => unsafe { writer.vector_writer.write_blob(idx, value.as_slice()) },
            Self::Date(value) => unsafe { writer.vector_writer.write_date(idx, *value) },
            Self::Time(value) => unsafe { writer.vector_writer.write_time(idx, *value) },
            Self::TimeTz(value) => unsafe { writer.vector_writer.write_time_tz(idx, *value) },
            Self::Timestamp(value) => unsafe { writer.vector_writer.write_timestamp(idx, *value) },
            Self::TimestampTz(value) => unsafe {
                writer.vector_writer.write_timestamp_tz(idx, *value)
            },
            Self::TimestampS(value) => unsafe {
                writer.vector_writer.write_timestamp_s(idx, *value)
            },
            Self::TimestampMs(value) => unsafe {
                writer.vector_writer.write_timestamp_ms(idx, *value)
            },
            Self::TimestampNs(value) => unsafe {
                writer.vector_writer.write_timestamp_ns(idx, *value)
            },
            Self::Uuid(value) => unsafe { writer.vector_writer.write_uuid(idx, *value) },
            Self::Interval(value) => unsafe { writer.vector_writer.write_interval(idx, *value) },
            Self::Decimal { width, unscaled, .. } => unsafe {
                writer.vector_writer.write_decimal(idx, *width, *unscaled)
            },
            Self::List(_) | Self::Struct(_) | Self::Map(_) => {
                return Err(duck_error(
                    "dynamic column: a nested value must be written through its column \
                     writer, not as a scalar",
                ));
            }
        }
        Ok(())
    }
}

impl From<bool> for DuckDynamicValue {
    /// `bool` → `BOOLEAN`。
    ///
    /// `bool` → `BOOLEAN`.
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl From<i16> for DuckDynamicValue {
    /// `i16` → `SMALLINT`。
    ///
    /// `i16` → `SMALLINT`.
    fn from(value: i16) -> Self {
        Self::SmallInt(value)
    }
}

impl From<i32> for DuckDynamicValue {
    /// `i32` → `INTEGER`。
    ///
    /// `i32` → `INTEGER`.
    fn from(value: i32) -> Self {
        Self::Integer(value)
    }
}

impl From<i64> for DuckDynamicValue {
    /// `i64` → `BIGINT`。
    ///
    /// `i64` → `BIGINT`.
    fn from(value: i64) -> Self {
        Self::BigInt(value)
    }
}

impl From<f64> for DuckDynamicValue {
    /// `f64` → `DOUBLE`。
    ///
    /// `f64` → `DOUBLE`.
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<String> for DuckDynamicValue {
    /// `String` → `VARCHAR`。
    ///
    /// `String` → `VARCHAR`.
    fn from(value: String) -> Self {
        Self::Varchar(value)
    }
}

impl From<&str> for DuckDynamicValue {
    /// `&str` → `VARCHAR`。
    ///
    /// `&str` → `VARCHAR`.
    fn from(value: &str) -> Self {
        Self::Varchar(value.to_owned())
    }
}

impl std::fmt::Display for DuckDynamicValue {
    /// 无 schema 的文本渲染（诊断 / 展示用）。
    ///
    /// The schema-less text rendering (for diagnostics / display).
    ///
    /// 与 [`DuckDynamicValue::to_text`] 的唯一区别是 `STRUCT`：这里没有 schema，只能用位置表示
    /// （`{1, a}`）而不是字段名（`{'id': 1, 'name': a}`）。需要可读的嵌套结构时请用 `to_text`。
    ///
    /// The only difference from [`DuckDynamicValue::to_text`] is `STRUCT`: without a schema it can
    /// only print positions (`{1, a}`) rather than field names (`{'id': 1, 'name': a}`). Use
    /// `to_text` whenever a nested structure should look like itself.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        self.render(None, &mut out);
        f.write_str(&out)
    }
}

/// 把浮点数写成「一定带小数点」的文本（`1` → `1.0`），`inf` / `NaN` 原样保留。
///
/// Writes a float so that it always carries a decimal point (`1` → `1.0`), leaving `inf` / `NaN`
/// alone.
fn push_float(value: f64, out: &mut String) {
    let text = value.to_string();
    out.push_str(&text);
    if !text.contains(['.', 'e', 'E']) && !text.contains("inf") && !text.contains("NaN") {
        out.push_str(".0");
    }
}
