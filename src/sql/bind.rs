//! 把 [`DuckDynamicValue`] 绑到预处理语句的参数上。
//!
//! Binding [`DuckDynamicValue`]s onto a prepared statement's parameters.

use crate::{DuckDynamicValue, DuckResult, Value, duck_error};
use quack_rs::error::ExtensionError;
use quack_rs::query::PreparedStatement;

/// 按顺序把 `params` 绑到语句的 `?` 位置上（C API 的参数下标从 1 开始）。
///
/// Binds `params` to the statement's `?` placeholders in order (the C API's parameter index is
/// 1-based).
///
/// 先核对参数个数：多一个少一个都直接报错，而不是让 DuckDB 在绑定到一半时给出一个更难读的消息。
///
/// The count is checked first: too few or too many is reported here, rather than letting DuckDB
/// produce a harder-to-read message half way through the bindings.
///
/// # Errors
///
/// 参数个数与语句不符、或某个值无法绑定（含无法绑定的嵌套值）时返回错误。
///
/// Returns an error when the count does not match the statement, or a value cannot be bound
/// (including nested values, which are not bindable).
pub(super) fn bind_all(statement: &PreparedStatement, params: &[DuckDynamicValue]) -> DuckResult<()> {
    let expected = statement.parameter_count();
    if expected != params.len() {
        return Err(duck_error(format!(
            "duckfn::sql: the statement has {expected} parameter(s) but {} value(s) were given",
            params.len()
        )));
    }
    for (offset, param) in params.iter().enumerate() {
        let index = offset + 1;
        let value = to_duck_value(param).map_err(|error| parameter_error(index, error))?;
        statement
            .bind_value(index, &value)
            .map_err(|error| parameter_error(index, error))?;
    }
    Ok(())
}

/// 给「第几个参数出错了」加上下标，便于定位（绑定的是个列表时尤其有用）。
///
/// Tags a parameter error with its index, so a failing list of bindings is easy to locate.
fn parameter_error(index: usize, error: ExtensionError) -> ExtensionError {
    duck_error(format!("duckfn::sql: parameter {index}: {}", error.as_str()))
}

/// [`DuckDynamicValue`] → quack-rs [`Value`]，交给 `duckdb_bind_value`。
///
/// [`DuckDynamicValue`] → quack-rs [`Value`], handed to `duckdb_bind_value`.
///
/// 统一走 `Value` 而不是逐个 `bind_*`：`duckdb_value` 覆盖全部标量分支（含 `UUID` / `TIME WITH
/// TIME ZONE` / `TIMESTAMP_S|MS|NS`，它们没有各自的 `bind_*`），并且和值类型那边用的是同一套
/// 构造与范围校验。
///
/// This goes through `Value` rather than one `bind_*` per branch: `duckdb_value` covers every scalar
/// (`UUID` / `TIME WITH TIME ZONE` / `TIMESTAMP_S|MS|NS` have no `bind_*` of their own) and reuses the
/// same constructors and range checks as the value-type side.
///
/// # Errors
///
/// 值的物理编码超出 DuckDB 的范围（`TIME` / `TIMESTAMP*` / `DECIMAL` 的构造会校验），或值本身是
/// 不支持绑定的嵌套容器时返回错误。
///
/// Returns an error when the value's physical encoding is outside DuckDB's range (the `TIME` /
/// `TIMESTAMP*` / `DECIMAL` constructors validate) or the value is a nested container, which is not
/// bindable.
fn to_duck_value(value: &DuckDynamicValue) -> DuckResult<Value> {
    let duck_value = match value {
        DuckDynamicValue::Boolean(value) => Value::boolean(*value),
        DuckDynamicValue::TinyInt(value) => Value::tinyint(*value),
        DuckDynamicValue::SmallInt(value) => Value::smallint(*value),
        DuckDynamicValue::Integer(value) => Value::integer(*value),
        DuckDynamicValue::BigInt(value) => Value::bigint(*value),
        DuckDynamicValue::HugeInt(value) => Value::hugeint(*value),
        DuckDynamicValue::UTinyInt(value) => Value::utinyint(*value),
        DuckDynamicValue::USmallInt(value) => Value::usmallint(*value),
        DuckDynamicValue::UInteger(value) => Value::uinteger(*value),
        DuckDynamicValue::UBigInt(value) => Value::ubigint(*value),
        DuckDynamicValue::UHugeInt(value) => Value::uhugeint(*value),
        DuckDynamicValue::Float(value) => Value::float(*value),
        DuckDynamicValue::Double(value) => Value::double(*value),
        DuckDynamicValue::Varchar(value) => Value::varchar(value),
        DuckDynamicValue::Blob(value) => Value::blob(value),
        DuckDynamicValue::Date(value) => Value::date(*value),
        DuckDynamicValue::Time(value) => Value::time(*value)?,
        DuckDynamicValue::TimeTz(value) => Value::time_tz(*value)?,
        DuckDynamicValue::Timestamp(value) => Value::timestamp(*value)?,
        DuckDynamicValue::TimestampTz(value) => Value::timestamp_tz(*value)?,
        DuckDynamicValue::TimestampS(value) => Value::timestamp_s(*value)?,
        DuckDynamicValue::TimestampMs(value) => Value::timestamp_ms(*value)?,
        DuckDynamicValue::TimestampNs(value) => Value::timestamp_ns(*value)?,
        DuckDynamicValue::Uuid(value) => Value::uuid(*value),
        DuckDynamicValue::Interval(value) => Value::interval(*value),
        DuckDynamicValue::Decimal {
            width,
            scale,
            unscaled,
        } => Value::decimal(*width, *scale, *unscaled)?,
        DuckDynamicValue::List(_) | DuckDynamicValue::Struct(_) | DuckDynamicValue::Map(_) => {
            return Err(duck_error(
                "a nested LIST / STRUCT / MAP cannot be bound as a parameter (its nested NULLs have \
                 no duckdb_value form); build it inline in the SQL with list_value / struct_pack / \
                 map, or bind its scalar parts",
            ));
        }
    };
    Ok(duck_value)
}

#[cfg(test)]
mod tests {
    use super::{parameter_error, to_duck_value};
    use crate::{DuckDynamicValue, duck_error};

    // 只测不碰 DuckDB 运行时的部分：这两个用例走的都是「在调用任何 FFI 之前就返回」的分支。
    // 标量分支要真的构造 `duckdb_value`，那必须连着 DuckDB 运行时（单测里 API 表未初始化，
    // 会直接断言失败），所以留给 `test/sql/functions/sql_exec.test`。
    //
    // Only the parts that never touch the DuckDB runtime: both cases take a branch that returns
    // before any FFI call. The scalar branches really do construct a `duckdb_value`, which needs the
    // DuckDB runtime (the API table is uninitialised in a unit test and asserts), so those are left to
    // `test/sql/functions/sql_exec.test`.

    #[test]
    fn nested_values_are_refused_before_any_binding() {
        let list = DuckDynamicValue::list([Some(DuckDynamicValue::BigInt(1))]);
        let error = to_duck_value(&list).expect_err("a LIST is not bindable");
        assert!(error.as_str().contains("LIST / STRUCT / MAP"), "{error}");

        let map = DuckDynamicValue::map([(
            DuckDynamicValue::Varchar("k".to_string()),
            DuckDynamicValue::BigInt(1),
        )]);
        assert!(to_duck_value(&map).is_err());
        assert!(to_duck_value(&DuckDynamicValue::Struct(Vec::new())).is_err());
    }

    #[test]
    fn parameter_errors_carry_the_one_based_index() {
        let message = parameter_error(3, duck_error("boom"));
        assert_eq!(message.as_str(), "duckfn::sql: parameter 3: boom");
    }
}
