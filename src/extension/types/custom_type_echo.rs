use duckfn::duck_scalar_function;
use duckfn::duck_table_function;
use duckfn::{DuckStruct, DuckValueType};
use quack_rs::prelude::{TypeId, Value, VectorReader, VectorWriter};

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
