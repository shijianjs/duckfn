//! 运行时动态列：把「列名 + 列类型」从编译期搬到 bind 阶段。
//!
//! Runtime dynamic columns: moving column names and column types from compile time to the bind
//! phase.
//!
//! 普通表函数的输出 schema 由 `#[derive(DuckStruct)]` 结构体在编译期固定
//! （见 [`TableFunctionAdapter`](crate::TableFunctionAdapter)）。当列本身要在 bind 阶段读
//! 外部元数据（文件头、字典表、远端 schema）才确定时，就需要这条动态通路：
//!
//! - [`DuckTypeDesc`]：可跨线程保存的递归类型描述。它能在 bind 里转成 DuckDB 逻辑类型用来
//!   声明输出列，也能从外部逻辑类型反推，因此「按外部元数据构造 schema」是直接的；
//! - [`DuckDynamicValue`]：运行时值枚举，覆盖标量与 `LIST` / `STRUCT` / `MAP`，并携带 SQL NULL；
//!   值只带数据、不带类型 —— 类型一律来自 schema，避免两处真相不一致；
//! - [`DuckResultSchema`] / [`DuckDynamicRow`] / [`DuckDynamicTable`]：动态 schema、动态行与
//!   「schema + 行迭代器」的结果集，供
//!   [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter) 在 bind / scan 两阶段使用。
//!
//! The output schema of an ordinary table function is fixed at compile time by its
//! `#[derive(DuckStruct)]` row struct. When the columns can only be known at bind time — after
//! reading external metadata such as a file header, a dictionary table or a remote schema — this
//! dynamic path is what you need. [`DuckTypeDesc`] is a `Send`-friendly recursive type
//! description that converts to a DuckDB logical type (to declare result columns) and back (to
//! build a schema from external logical types). [`DuckDynamicValue`] is a runtime value enum
//! covering scalars plus `LIST` / `STRUCT` / `MAP` with SQL NULL; values carry data only, never
//! types — the schema is the single source of truth. [`DuckResultSchema`], [`DuckDynamicRow`] and
//! [`DuckDynamicTable`] then carry that schema and a row iterator through the bind/scan phases.

use crate::value_types::vector_layout::{
    element_child_vector, finish_elements, map_keys, map_values, reserve_elements, set_entry,
    struct_field, write_null_row,
};
use crate::{
    DuckOptionResult, DuckResult, DuckValueType, DuckValueWriter, duck_error, duck_value_is_null,
};
use libduckdb_sys::duckdb_vector;
use quack_rs::interval::DuckInterval;
use quack_rs::prelude::{BindInfo, DataChunk, LogicalType, TypeId, Value};

// ============================================================================
// 运行时类型描述
// Runtime type description
// ============================================================================

/// 运行时列类型描述：`Send` 友好的递归逻辑类型。
///
/// A runtime column type description: a `Send`-friendly recursive logical type.
///
/// 为什么不用 [`LogicalType`] 本身？因为表函数 scan 状态必须 `Send + 'static`，而
/// `LogicalType` 持有裸句柄、不是 `Send`。`DuckTypeDesc` 只含 `TypeId` / `String` / `Box`，
/// 因此可以安全地跨阶段保存；`LogicalType` 只在 bind 内临时构造，声明完列即释放。
///
/// Why not [`LogicalType`] itself? The table-function scan state must be `Send + 'static`, while
/// `LogicalType` owns a raw handle and is not `Send`. `DuckTypeDesc` only holds `TypeId` /
/// `String` / `Box`, so it can be kept across phases; a `LogicalType` is built temporarily inside
/// bind, used to declare the columns and dropped right after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuckTypeDesc {
    /// 基础类型 / 包装类型（`BIGINT`、`VARCHAR`、`DATE`、`TIMESTAMP`、`UUID` …）。
    ///
    /// A primitive or wrapper type (`BIGINT`, `VARCHAR`, `DATE`, `TIMESTAMP`, `UUID`, ...).
    ///
    /// 只用于能用 [`LogicalType::new`] 直接重建的类型；参数化类型请用
    /// [`DuckTypeDesc::Decimal`] / [`DuckTypeDesc::List`] / [`DuckTypeDesc::Struct`] /
    /// [`DuckTypeDesc::Map`]，不要写成 `Scalar(TypeId::List)`。
    ///
    /// Only for types that [`LogicalType::new`] can rebuild directly. Parameterised types use
    /// [`DuckTypeDesc::Decimal`] / [`DuckTypeDesc::List`] / [`DuckTypeDesc::Struct`] /
    /// [`DuckTypeDesc::Map`] — never `Scalar(TypeId::List)`.
    Scalar(TypeId),
    /// `DECIMAL(width, scale)`：宽度与精度是逻辑类型的一部分，必须显式带着。
    ///
    /// `DECIMAL(width, scale)`: width and scale are part of the logical type, so they are carried
    /// explicitly.
    Decimal {
        /// 有效数字总位数。
        ///
        /// Total number of significant digits.
        width: u8,
        /// 小数点后的位数。
        ///
        /// Number of digits after the decimal point.
        scale: u8,
    },
    /// `LIST(element)`。
    ///
    /// `LIST(element)`.
    List(Box<DuckTypeDesc>),
    /// `STRUCT(name type, ...)`：字段名即列名，顺序即子向量下标。
    ///
    /// `STRUCT(name type, ...)`: the field name is the child name and the order is the child-vector
    /// index.
    Struct(Vec<(String, DuckTypeDesc)>),
    /// `MAP(key, value)`。
    ///
    /// `MAP(key, value)`.
    Map(Box<DuckTypeDesc>, Box<DuckTypeDesc>),
}

impl DuckTypeDesc {
    /// 由 [`TypeId`] 构造标量描述。
    ///
    /// Builds a scalar description from a [`TypeId`].
    #[must_use]
    pub fn scalar(type_id: TypeId) -> Self {
        Self::Scalar(type_id)
    }

    /// 由元素描述构造 `LIST`。
    ///
    /// Builds a `LIST` from the element description.
    #[must_use]
    pub fn list(element: Self) -> Self {
        Self::List(Box::new(element))
    }

    /// 由 `(字段名, 字段类型)` 列表构造 `STRUCT`。
    ///
    /// Builds a `STRUCT` from a list of `(field name, field type)` pairs.
    #[must_use]
    pub fn struct_type<I: IntoIterator<Item = (String, Self)>>(fields: I) -> Self {
        Self::Struct(fields.into_iter().collect())
    }

    /// 由键/值描述构造 `MAP`。
    ///
    /// Builds a `MAP` from the key and value descriptions.
    #[must_use]
    pub fn map(key: Self, value: Self) -> Self {
        Self::Map(Box::new(key), Box::new(value))
    }

    /// 本描述对应的 DuckDB 类型 id。
    ///
    /// The DuckDB type id this description maps onto.
    #[must_use]
    pub fn type_id(&self) -> TypeId {
        match self {
            Self::Scalar(type_id) => *type_id,
            Self::Decimal { .. } => TypeId::Decimal,
            Self::List(_) => TypeId::List,
            Self::Struct(_) => TypeId::Struct,
            Self::Map(_, _) => TypeId::Map,
        }
    }

    /// 转成 DuckDB 逻辑类型，用于 `bind.add_result_column_with_type`。
    ///
    /// Converts to a DuckDB logical type, ready for `bind.add_result_column_with_type`.
    ///
    /// 返回的句柄由调用方持有（离开作用域时自动释放），不要跨阶段保存 —— 它会在创建它的
    /// DuckDB 上下文之外失效。
    ///
    /// The returned handle is owned by the caller (released when it goes out of scope); do not keep
    /// it across phases — it is only valid inside the DuckDB context that created it.
    #[must_use]
    pub fn to_logical_type(&self) -> LogicalType {
        match self {
            Self::Scalar(type_id) => LogicalType::new(*type_id),
            Self::Decimal { width, scale } => LogicalType::decimal(*width, *scale),
            Self::List(element) => LogicalType::list_from_logical(&element.to_logical_type()),
            Self::Struct(fields) => {
                let children: Vec<(&str, LogicalType)> = fields
                    .iter()
                    .map(|(name, desc)| (name.as_str(), desc.to_logical_type()))
                    .collect();
                LogicalType::struct_type_from_logical(&children)
            }
            Self::Map(key, value) => {
                LogicalType::map_from_logical(&key.to_logical_type(), &value.to_logical_type())
            }
        }
    }

    /// 从外部逻辑类型反推描述（bind 阶段读文件/元数据 schema 时使用）。
    ///
    /// Reconstructs a description from an external logical type (used at bind time when the schema
    /// comes from a file or other metadata).
    ///
    /// 支持基础类型、包装类型、`DECIMAL`、`LIST`、`STRUCT`、`MAP`，可任意嵌套。`ENUM`、`ARRAY`、
    /// `UNION`、`BIT` 等形状没有稳定的「重建」方式（它们的参数并不在 [`TypeId`] 里），会返回
    /// 错误而不是给出一个不完整的描述。
    ///
    /// Primitives, wrappers, `DECIMAL`, `LIST`, `STRUCT` and `MAP` are supported and may nest
    /// arbitrarily. Shapes such as `ENUM`, `ARRAY`, `UNION` and `BIT` have no reliable
    /// reconstruction (their parameters are not part of [`TypeId`]), so they produce an error
    /// instead of an incomplete description.
    ///
    /// # Errors
    ///
    /// 逻辑类型无法用 [`DuckTypeDesc`] 表达时返回错误。
    ///
    /// Returns an error when the logical type cannot be expressed as a [`DuckTypeDesc`].
    pub fn from_logical_type(logical_type: &LogicalType) -> DuckResult<Self> {
        // SAFETY: 句柄由调用方持有且在 DuckDB 运行时内有效。
        //
        // SAFETY: the handle is owned by the caller and valid inside the DuckDB runtime.
        let raw = unsafe { libduckdb_sys::duckdb_get_type_id(logical_type.as_raw()) };
        let Some(type_id) = TypeId::try_from_duckdb_type(raw) else {
            return Err(duck_error(format!(
                "dynamic table function: unknown DuckDB type id {raw}; cannot build a \
                 DuckTypeDesc from this logical type"
            )));
        };
        match type_id {
            TypeId::List => Ok(Self::list(Self::from_logical_type(&unsafe {
                logical_type.list_child_type()
            })?)),
            TypeId::Map => {
                let key = Self::from_logical_type(&unsafe { logical_type.map_key_type() })?;
                let value = Self::from_logical_type(&unsafe { logical_type.map_value_type() })?;
                Ok(Self::map(key, value))
            }
            TypeId::Struct => {
                let count = unsafe { logical_type.struct_child_count() };
                let mut fields = Vec::with_capacity(count as usize);
                for index in 0..count {
                    fields.push((
                        unsafe { logical_type.struct_child_name(index) },
                        Self::from_logical_type(&unsafe { logical_type.struct_child_type(index) })?,
                    ));
                }
                Ok(Self::Struct(fields))
            }
            TypeId::Decimal => Ok(Self::Decimal {
                width: unsafe { logical_type.decimal_width() },
                scale: unsafe { logical_type.decimal_scale() },
            }),
            other => Self::from_scalar_type_id(other).ok_or_else(|| {
                duck_error(format!(
                    "dynamic table function: the DuckDB logical type `{other:?}` cannot be turned \
                     into a DuckTypeDesc (its parameters are not expressible); use a supported \
                     scalar, DECIMAL, LIST, STRUCT or MAP instead"
                ))
            }),
        }
    }

    /// 把（能用 [`LogicalType::new`] 重建的）标量类型 id 转成描述；不支持的形状返回 `None`。
    ///
    /// Turns a scalar type id that [`LogicalType::new`] can rebuild into a description; returns
    /// `None` for shapes that cannot be rebuilt.
    fn from_scalar_type_id(type_id: TypeId) -> Option<Self> {
        let supported = matches!(
            type_id,
            TypeId::Boolean
                | TypeId::TinyInt
                | TypeId::SmallInt
                | TypeId::Integer
                | TypeId::BigInt
                | TypeId::UTinyInt
                | TypeId::USmallInt
                | TypeId::UInteger
                | TypeId::UBigInt
                | TypeId::HugeInt
                | TypeId::UHugeInt
                | TypeId::Float
                | TypeId::Double
                | TypeId::Varchar
                | TypeId::Blob
                | TypeId::Uuid
                | TypeId::Date
                | TypeId::Time
                | TypeId::TimeTz
                | TypeId::Timestamp
                | TypeId::TimestampTz
                | TypeId::TimestampS
                | TypeId::TimestampMs
                | TypeId::TimestampNs
                | TypeId::Interval
        );
        supported.then_some(Self::Scalar(type_id))
    }

    /// 由值反推描述：方便「先有数据、再推 schema」的用法。
    ///
    /// Infers a description from a value, for the "data first, schema second" workflow.
    ///
    /// 注意：空 `LIST` / 空 `MAP` 无法从值本身看出元素类型，会退化成
    /// `LIST(VARCHAR)` / `MAP(VARCHAR, VARCHAR)`。有真实 schema 时请直接构造描述，别用推断。
    ///
    /// Note: an empty `LIST` / `MAP` has no element type to observe and falls back to
    /// `LIST(VARCHAR)` / `MAP(VARCHAR, VARCHAR)`. Prefer building the description directly whenever
    /// the real schema is available.
    #[must_use]
    pub fn from_value(value: &DuckDynamicValue) -> Self {
        match value {
            DuckDynamicValue::List(items) => Self::list(
                items
                    .iter()
                    .flatten()
                    .next()
                    .map_or(Self::Scalar(TypeId::Varchar), Self::from_value),
            ),
            DuckDynamicValue::Struct(fields) => Self::Struct(
                fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let name = format!("field_{index}");
                        (name, field.as_ref().map_or(Self::Scalar(TypeId::Varchar), Self::from_value))
                    })
                    .collect(),
            ),
            DuckDynamicValue::Map(pairs) => {
                let key = pairs
                    .first()
                    .map_or(Self::Scalar(TypeId::Varchar), |(key, _)| Self::from_value(key));
                let value = pairs
                    .first()
                    .map_or(Self::Scalar(TypeId::Varchar), |(_, value)| Self::from_value(value));
                Self::map(key, value)
            }
            other => other.scalar_type_desc(),
        }
    }

    /// 值能否写进本描述对应的列（递归校验，写向量前用来避免类型错配）。
    ///
    /// Whether a value can be written into a column of this description (validated recursively
    /// before writing, to catch type mismatches).
    #[must_use]
    pub fn matches_value(&self, value: &DuckDynamicValue) -> bool {
        match (self, value) {
            (Self::Scalar(type_id), value) => value.scalar_type_id() == Some(*type_id),
            (
                Self::Decimal { width, scale },
                DuckDynamicValue::Decimal {
                    width: value_width,
                    scale: value_scale,
                    ..
                },
            ) => width == value_width && scale == value_scale,
            (Self::List(element), DuckDynamicValue::List(items)) => items
                .iter()
                .all(|item| item.as_ref().is_none_or(|item| element.matches_value(item))),
            (Self::Struct(fields), DuckDynamicValue::Struct(values)) => {
                fields.len() == values.len()
                    && fields.iter().zip(values).all(|((_, desc), value)| {
                        value.as_ref().is_none_or(|value| desc.matches_value(value))
                    })
            }
            (Self::Map(key, map_value), DuckDynamicValue::Map(pairs)) => pairs
                .iter()
                .all(|(k, v)| key.matches_value(k) && map_value.matches_value(v)),
            _ => false,
        }
    }
}

impl std::fmt::Display for DuckTypeDesc {
    /// 渲染成人类可读的类型名（用于错误信息），形如 `MAP(VARCHAR, BIGINT[])`。
    ///
    /// Renders a human-readable type name (used in error messages), such as
    /// `MAP(VARCHAR, BIGINT[])`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar(type_id) => f.write_str(type_id.sql_name()),
            Self::Decimal { width, scale } => write!(f, "DECIMAL({width}, {scale})"),
            Self::List(element) => write!(f, "{element}[]"),
            Self::Struct(fields) => {
                f.write_str("STRUCT(")?;
                for (index, (name, desc)) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{name} {desc}")?;
                }
                f.write_str(")")
            }
            Self::Map(key, value) => write!(f, "MAP({key}, {value})"),
        }
    }
}

// ============================================================================
// 运行时值
// Runtime values
// ============================================================================

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
    /// `TIMESTAMP WITH TIME ZONE`（自纪元的毫秒数）。
    ///
    /// `TIMESTAMP WITH TIME ZONE` (milliseconds since the epoch).
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
    fn scalar_type_id(&self) -> Option<TypeId> {
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
    fn scalar_type_desc(&self) -> DuckTypeDesc {
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
                        .ok_or_else(|| duck_error("dynamic table function: MAP key cannot be null"))?;
                    let map_value = value
                        .map_value(index)
                        .map(|map_value| Self::from_duck_value(&map_value, value_desc))
                        .transpose()?
                        .flatten()
                        .ok_or_else(|| duck_error("dynamic table function: MAP value cannot be null"))?;
                    pairs.push((key, map_value));
                }
                Self::Map(pairs)
            }
            DuckTypeDesc::Scalar(other) => {
                return Err(duck_error(format!(
                    "dynamic table function: DuckTypeDesc `{other:?}` is not supported by \
                     DuckDynamicValue::from_duck_value"
                )));
            }
        };
        Ok(Some(dynamic))
    }

    /// 把一个标量值写进向量；嵌套值由 [`DynColumnWriter`] 处理，这里返回错误。
    ///
    /// Writes one scalar value into a vector; nested values are handled by [`DynColumnWriter`] and
    /// report an error here.
    fn write_scalar(&self, writer: &mut DuckValueWriter, idx: usize) -> DuckResult<()> {
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
                writer.vector_writer.write_timestamp_ms(idx, *value)
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
                    "dynamic table function: a nested value must be written through its column \
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

// ============================================================================
// 动态 schema / 行 / 结果集
// Dynamic schema / row / result table
// ============================================================================

/// 动态结果集 schema：有序的 `(列名, 列类型)` 列表。
///
/// A dynamic result schema: an ordered list of `(column name, column type)` pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DuckResultSchema {
    /// 列定义，顺序即输出列顺序。
    ///
    /// The column definitions; the order is the output column order.
    columns: Vec<(String, DuckTypeDesc)>,
}

impl DuckResultSchema {
    /// 由 `(列名, 列类型)` 列表构造 schema。
    ///
    /// Builds a schema from a list of `(column name, column type)` pairs.
    #[must_use]
    pub fn new(columns: Vec<(String, DuckTypeDesc)>) -> Self {
        Self { columns }
    }

    /// 由 `(&str, TypeId)` 列表构造「全标量列」schema 的便捷方法。
    ///
    /// A convenience constructor for an all-scalar schema, from a list of `(&str, TypeId)` pairs.
    #[must_use]
    pub fn from_scalar_types<'a, I: IntoIterator<Item = (&'a str, TypeId)>>(columns: I) -> Self {
        Self {
            columns: columns
                .into_iter()
                .map(|(name, type_id)| (name.to_owned(), DuckTypeDesc::Scalar(type_id)))
                .collect(),
        }
    }

    /// 列定义（顺序即输出列顺序）。
    ///
    /// The column definitions (the order is the output column order).
    #[must_use]
    pub fn columns(&self) -> &[(String, DuckTypeDesc)] {
        &self.columns
    }

    /// 取第 `index` 列的定义。
    ///
    /// Returns the definition of column `index`.
    #[must_use]
    pub fn column(&self, index: usize) -> Option<&(String, DuckTypeDesc)> {
        self.columns.get(index)
    }

    /// 列数。
    ///
    /// The number of columns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// 是否没有列。
    ///
    /// Whether there are no columns at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// 把 schema 登记到 bind 信息上：逐列 `add_result_column_with_type`。
    ///
    /// 逻辑类型只在本次调用内构造，声明完即释放 —— 不跨阶段持有。
    ///
    /// Registers the schema on the bind info by calling `add_result_column_with_type` per column.
    /// The logical types are built inside the call and released right after — nothing is kept
    /// across phases.
    pub fn declare(&self, bind: &BindInfo) {
        for (name, desc) in &self.columns {
            let logical_type = desc.to_logical_type();
            bind.add_result_column_with_type(name, &logical_type);
        }
    }

    /// 各列的逻辑类型（顺序与 [`Self::columns`] 一致）。
    ///
    /// The logical types of every column (same order as [`Self::columns`]).
    #[must_use]
    pub fn to_logical_types(&self) -> Vec<LogicalType> {
        self.columns.iter().map(|(_, desc)| desc.to_logical_type()).collect()
    }
}

impl From<Vec<(String, DuckTypeDesc)>> for DuckResultSchema {
    /// `Vec<(列名, 列类型)>` → schema。
    ///
    /// `Vec<(column name, column type)>` → schema.
    fn from(columns: Vec<(String, DuckTypeDesc)>) -> Self {
        Self::new(columns)
    }
}

/// 动态行：一行的各列值，顺序与 [`DuckResultSchema`] 一致。
///
/// A dynamic row: one value per column, in the same order as the [`DuckResultSchema`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DuckDynamicRow {
    /// 每列的值；`None` 表示该单元格是 SQL NULL。
    ///
    /// One value per column; `None` means that cell is SQL NULL.
    values: Vec<Option<DuckDynamicValue>>,
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
                    "dynamic table function: row {row_index} has {} columns but the schema \
                     declares {}",
                    row.values.len(),
                    columns.len()
                )));
            }
            for (column_index, (name, desc)) in columns.iter().enumerate() {
                if let Some(value) = &row.values[column_index] {
                    if !desc.matches_value(value) {
                        return Err(duck_error(format!(
                            "dynamic table function: row {row_index} column `{name}` expects \
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
}

impl From<Vec<Option<DuckDynamicValue>>> for DuckDynamicRow {
    /// `Vec<Option<DuckDynamicValue>>` → 动态行。
    ///
    /// `Vec<Option<DuckDynamicValue>>` → dynamic row.
    fn from(values: Vec<Option<DuckDynamicValue>>) -> Self {
        Self::new(values)
    }
}

/// 动态行迭代器：与静态表函数的 `DuckFullIterator` 同形（可发送、逐行可错可为空）。
///
/// The dynamic-row iterator: the same shape as the static table functions' `DuckFullIterator`
/// (sendable, every row may fail or be NULL).
pub type DuckDynamicIterator = Box<dyn Iterator<Item = DuckOptionResult<DuckDynamicRow>> + Send>;

/// 动态结果集：bind 阶段算出的 schema + 行迭代器。
///
/// A dynamic result table: the schema computed during bind plus the row iterator.
///
/// 它就是用户 `bind` 函数的返回值（见
/// [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter)）：bind 负责「读外部
/// 元数据、给出 schema、造出迭代器」，scan 负责按 schema 批量写出。
///
/// This is what a user `bind` function returns (see
/// [`DynamicTableFunctionAdapter`](crate::DynamicTableFunctionAdapter)): bind reads the external
/// metadata, publishes the schema and builds the iterator; scan writes rows out according to that
/// schema.
pub struct DuckDynamicTable {
    /// 输出列定义。
    ///
    /// The output column definitions.
    schema: DuckResultSchema,
    /// 行迭代器。
    ///
    /// The row iterator.
    rows: DuckDynamicIterator,
}

impl DuckDynamicTable {
    /// 由 schema 与行迭代器构造结果集。
    ///
    /// Builds a result table from a schema and a row iterator.
    #[must_use]
    pub fn new(schema: DuckResultSchema, rows: DuckDynamicIterator) -> Self {
        Self { schema, rows }
    }

    /// 输出 schema。
    ///
    /// The output schema.
    #[must_use]
    pub fn schema(&self) -> &DuckResultSchema {
        &self.schema
    }

    /// 拆成 `(schema, 行迭代器)`，供适配层放进 scan 状态。
    ///
    /// Splits into `(schema, row iterator)` so the adapter can move them into the scan state.
    #[must_use]
    pub fn into_parts(self) -> (DuckResultSchema, DuckDynamicIterator) {
        (self.schema, self.rows)
    }
}

impl std::fmt::Debug for DuckDynamicTable {
    /// 只打印 schema —— 迭代器无法 Debug，也不该被打印。
    ///
    /// Prints the schema only — the iterator is not `Debug` and should not be printed anyway.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DuckDynamicTable")
            .field("schema", &self.schema)
            .finish_non_exhaustive()
    }
}

/// 取一批行在第 `column_index` 列上的值（用于构建该列的写入器）。
///
/// Extracts the values of column `column_index` across a batch (used to build that column's
/// writer).
fn column_values<'a>(
    rows: &'a [Option<&'a DuckDynamicRow>],
    column_index: usize,
) -> Vec<Option<&'a DuckDynamicValue>> {
    rows.iter()
        .map(|row| row.and_then(|row| row.values.get(column_index)).and_then(Option::as_ref))
        .collect()
}

// ============================================================================
// 列写入器树
// Column writer tree
// ============================================================================

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
struct DynColumnWriter {
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
    fn prepare(
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
    fn write_value(&mut self, idx: usize, value: &DuckDynamicValue) -> DuckResult<()> {
        match value {
            DuckDynamicValue::List(items) => {
                if self.children.len() != 1 {
                    return Err(duck_error(
                        "dynamic table function: LIST value written through a non-LIST column writer",
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
                        "dynamic table function: STRUCT has {} fields but the column declares {}",
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
                        "dynamic table function: MAP value written through a non-MAP column writer",
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
    fn write_null(&mut self, idx: usize) -> DuckResult<()> {
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
    fn finish(&mut self) {
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
