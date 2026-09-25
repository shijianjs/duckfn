//! 运行时类型描述：`DuckTypeDesc`。
//!
//! Runtime type description: `DuckTypeDesc`.

use crate::{DuckResult, duck_error};
use quack_rs::prelude::{LogicalType, TypeId};

use super::dynamic_value::DuckDynamicValue;

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
                "dynamic column: unknown DuckDB type id {raw}; cannot build a \
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
                    "dynamic column: the DuckDB logical type `{other:?}` cannot be turned \
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
