use duckfn::duck_scalar_function;
use duckfn::duck_table_function;
use duckfn::{DuckStruct, DuckValueReader, DuckValueType, DuckValueWriter};
use libduckdb_sys::{duckdb_vector, duckdb_vector_get_data};
use quack_rs::prelude::{LogicalType, TypeId, Value, VectorReader, VectorWriter};

use super::table_echo_util::echo_rows;

// ============================================================================
// 用户自定义类型
//
// 演示 duckfn 的扩展点：`DuckValueType` 是公开、未封闭的 trait，在 duckfn 之外
// 也能为自己的类型实现它 —— 本文件就是在 duckfn 之外实现的。
//
// 必须重写的只有三个方法：
//   type_id()                        声明对应的 DuckDB 逻辑类型
//   read_valid_by_vector_reader()    从输入向量读一个有效值
//   write_valid_to_vector_writer()   把一个有效值写进输出向量
// 想让这个类型还能用在表函数参数 / 结构体字段上，再补一个：
//   read_by_duck_value_valid_simple()  从 bind 阶段的 duckdb_value 读取
// NULL 处理、批量读写等其余方法都由 trait 的默认实现给出。
//
// 另外两点约束：
//   - trait 本身要求 Clone + Debug + Send + Sync + 'static；
//   - 作为函数参数时还需要 Default，因为宏生成的 DuckArgsImpl 会 derive(Default)。
//
// 更复杂的情况 —— quack-rs 没有读写方法的逻辑类型（ENUM / BIT / VARINT ...），
// 或者需要子向量的容器类型 —— DuckValueReader / DuckValueWriter 的
// c_duckdb_vector 字段是公开的裸向量，可以直接下到 DuckDB 的 C API；
// 容器类型的完整实现可参考 duckfn 自己的 value_types/duck_list.rs 与 duck_struct.rs。
//
// 本文件下半部分就是 ENUM 的完整例子：`Color` —— 字典存在逻辑类型里、向量里只存下标，
// 读写都得自己下到 C API。
// ============================================================================

/// 摄氏温度：物理表示就是 `DOUBLE`，语义由这个 newtype 承载。
///
/// A Celsius temperature: the physical representation is `DOUBLE`, and the newtype carries the
/// semantics — DuckDB has no temperature type.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Celsius(pub f64);

impl DuckValueType for Celsius {
    fn type_id() -> TypeId {
        TypeId::Double
    }
    fn read_valid_by_vector_reader(reader: &VectorReader, row: usize) -> Self {
        Self(unsafe { reader.read_f64(row) })
    }
    fn write_valid_to_vector_writer(writer: &mut VectorWriter, idx: usize, v: &Self) {
        unsafe { writer.write_f64(idx, v.0) }
    }
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self(value.as_f64())
    }
}

/// ```sql
/// SELECT dfn_echo_celsius(21.5);
/// ```
#[duck_scalar_function]
fn dfn_echo_celsius(t: Celsius) -> Celsius {
    t
}

/// 自定义类型可以直接当 `STRUCT` 字段：derive 会为每个字段生成
/// `assert_impl_duck_value_type::<T>()` 编译期断言，因此这段能编译就说明已经接上。
///
/// A custom type works as a `STRUCT` field: the derive emits a compile-time
/// `assert_impl_duck_value_type::<T>()` for every field.
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct TemperatureReading {
    pub place: String,
    pub celsius: Celsius,
}

/// ```sql
/// SELECT (dfn_echo_temperature_reading({'place': 'oslo', 'celsius': -3.5::DOUBLE})).place;
/// ```
#[duck_scalar_function]
fn dfn_echo_temperature_reading(r: TemperatureReading) -> TemperatureReading {
    r
}

/// ```sql
/// SELECT v FROM dfn_table_echo_celsius(21.5::DOUBLE, count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoCelsiusRow {
    pub v: Option<Celsius>,
}

#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_celsius(
    v: Option<Celsius>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoCelsiusRow> {
    echo_rows(v, count, |v| TableEchoCelsiusRow { v })
}

// ============================================================================
// ENUM：quack-rs 没有读写方法的逻辑类型，自己下到 C API
//
// DuckDB 的 ENUM 由两部分组成：
//   - 逻辑类型里存**标签字典**（'red' / 'green' / 'blue' …），用 LogicalType::enum_type 声明；
//   - 向量数据里只存**下标**，宽度按成员个数选（≤255 用 UTINYINT、≤65535 用 USMALLINT，
//     再多用 UINTEGER）。
//
// 所以自己实现 DuckValueType 的关键是三点：
//   1. logical_type() 返回带字典的 ENUM 类型（quack-rs 的 LogicalType 是 RAII 的，drop 自动销毁）；
//   2. 读/写要覆盖**带裸向量**的 read_valid / write_valid（而不是 read_valid_by_vector_reader /
//      write_valid_to_vector_writer），因为要拿 c_duckdb_vector 去 duckdb_vector_get_data 取下标数组；
//   3. bind 阶段（表函数参数、结构体字段）拿到的是 duckdb_value，用 Value::as_str() 取标签。
//
// 要读一个「别人创建的」ENUM 类型时，可以用 duckdb_vector_get_column_type +
// duckdb_enum_internal_type 问出它的内部表示；这里由自己的成员个数直接算出来，
// 省掉每次读取都创建/销毁一个逻辑类型。
// ============================================================================

/// DuckDB `ENUM('red', 'green', 'blue')`：物理表示是下标，语义由这个 Rust 枚举承载。
///
/// A DuckDB `ENUM('red', 'green', 'blue')`: the physical representation is an index, and this Rust
/// enum carries the semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Red,
    Green,
    Blue,
}

impl Color {
    /// 标签字典，**顺序即下标**，必须和 DuckDB 侧一致。
    ///
    /// The label dictionary — the order *is* the index and must match the DuckDB side.
    pub const MEMBERS: [&'static str; 3] = ["red", "green", "blue"];

    /// DuckDB 按成员个数挑的内部表示类型（枚举下标在向量里的物理宽度）。
    ///
    /// The internal representation DuckDB picks from the member count: the physical width of the
    /// index inside the vector.
    fn internal_type() -> TypeId {
        match Self::MEMBERS.len() {
            0..=255 => TypeId::UTinyInt,
            256..=65535 => TypeId::USmallInt,
            _ => TypeId::UInteger,
        }
    }

    /// 变体 → 下标。
    ///
    /// Variant → index.
    fn index(self) -> u32 {
        match self {
            Color::Red => 0,
            Color::Green => 1,
            Color::Blue => 2,
        }
    }

    /// 下标 → 变体；越界时返回 `None`（读取侧会把它当成 NULL）。
    ///
    /// Index → variant; an out-of-range index yields `None` (the read side turns it into NULL).
    fn from_index(index: u32) -> Option<Self> {
        match index {
            0 => Some(Color::Red),
            1 => Some(Color::Green),
            2 => Some(Color::Blue),
            _ => None,
        }
    }

    /// 标签 → 变体（bind 阶段的 `duckdb_value` 给的是标签文本）。
    ///
    /// Label → variant (the bind-time `duckdb_value` holds the label text).
    fn from_label(label: &str) -> Option<Self> {
        Self::MEMBERS
            .iter()
            .position(|member| *member == label)
            .and_then(|index| Self::from_index(index as u32))
    }

    /// 从向量的下标数组里读一个下标。
    ///
    /// Reads one index out of the vector's index array.
    fn read_index(vector: duckdb_vector, row: usize) -> u32 {
        let data = unsafe { duckdb_vector_get_data(vector) };
        unsafe {
            match Self::internal_type() {
                TypeId::UTinyInt => u32::from(*data.cast::<u8>().add(row)),
                TypeId::USmallInt => u32::from(*data.cast::<u16>().add(row)),
                _ => *data.cast::<u32>().add(row),
            }
        }
    }

    /// 往向量的下标数组里写一个下标。
    ///
    /// Writes one index into the vector's index array.
    fn write_index(vector: duckdb_vector, row: usize, index: u32) {
        let data = unsafe { duckdb_vector_get_data(vector) };
        unsafe {
            match Self::internal_type() {
                TypeId::UTinyInt => *data.cast::<u8>().add(row) = index as u8,
                TypeId::USmallInt => *data.cast::<u16>().add(row) = index as u16,
                _ => *data.cast::<u32>().add(row) = index,
            }
        }
    }
}

impl DuckValueType for Color {
    fn type_id() -> TypeId {
        TypeId::Enum
    }

    /// 带字典的 ENUM 逻辑类型：SQL 侧就是 `ENUM('red', 'green', 'blue')`。
    ///
    /// The ENUM logical type together with its dictionary: on the SQL side this is
    /// `ENUM('red', 'green', 'blue')`.
    fn logical_type() -> LogicalType {
        LogicalType::enum_type(&Color::MEMBERS)
    }

    /// 读：覆盖带裸向量的版本，才能拿到下标数组（也可以顺便按向量自己的内部表示读）。
    ///
    /// Reading: the raw-vector flavour is overridden so the index array is reachable.
    fn read_valid(reader: &DuckValueReader, row: usize) -> Option<Self> {
        Self::from_index(Self::read_index(reader.c_duckdb_vector, row))
    }

    /// 写：把变体对应的下标写进输出向量即可，不需要分配字符串。
    ///
    /// Writing: only the index has to be stored — no string allocation.
    fn write_valid(writer: &mut DuckValueWriter, idx: usize, v: &Self) {
        Self::write_index(writer.c_duckdb_vector, idx, v.index());
    }

    /// bind 阶段：`duckdb_value` 给的是标签文本。
    ///
    /// The bind path: the `duckdb_value` holds the label text.
    ///
    /// 标签一定来自同一个字典（DuckDB 已经校验过），取不到就退回默认变体。
    ///
    /// The label always comes from the same dictionary (DuckDB validated it), so a miss falls back
    /// to the default variant.
    fn read_by_duck_value_valid_simple(value: &Value) -> Self {
        Self::from_label(&value.as_str().unwrap_or_default()).unwrap_or_default()
    }
}

/// ```sql
/// SELECT dfn_echo_color('red');
/// SELECT dfn_echo_color(NULL);
/// ```
#[duck_scalar_function]
fn dfn_echo_color(c: Color) -> Color {
    c
}

/// 可空版本：`Option<Color>` 的 NULL 语义与其它类型一致。
///
/// The nullable flavour: `Option<Color>` follows the same NULL rules as every other type.
///
/// ```sql
/// SELECT dfn_echo_color_n(NULL);
/// ```
#[duck_scalar_function]
fn dfn_echo_color_n(c: Option<Color>) -> Option<Color> {
    c
}

/// 自定义 ENUM 也能直接当 `STRUCT` 字段（含可空字段）。
///
/// A custom ENUM works as a `STRUCT` field as well (including a nullable one).
#[derive(Debug, Clone, Default, DuckStruct)]
pub struct ColorReading {
    pub name: String,
    pub color: Color,
    pub fallback: Option<Color>,
}

/// ```sql
/// SELECT (dfn_echo_color_reading({'name': 'sky', 'color': 'blue', 'fallback': NULL})).color;
/// ```
#[duck_scalar_function]
fn dfn_echo_color_reading(r: ColorReading) -> ColorReading {
    r
}

/// ```sql
/// SELECT v FROM dfn_table_echo_color('green', count => 3);
/// ```
#[derive(Clone, Debug, Default, DuckStruct)]
pub struct TableEchoColorRow {
    pub v: Option<Color>,
}

/// 表函数：入参走 bind 阶段的 `duckdb_value`（即 `read_by_duck_value_valid_simple`），
/// 出参是 ENUM 列。
///
/// A table function: the argument arrives as a bind-time `duckdb_value` (i.e. through
/// `read_by_duck_value_valid_simple`) and the output column is an ENUM.
#[duck_table_function(named_param_from = "count")]
fn dfn_table_echo_color(
    v: Option<Color>,
    count: Option<i64>,
) -> impl Iterator<Item = TableEchoColorRow> {
    echo_rows(v, count, |v| TableEchoColorRow { v })
}
