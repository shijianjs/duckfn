//! 把逻辑类型渲染成 SQL，并在加载期把它注册成 DuckDB 里的**命名类型**。
//!
//! Rendering a logical type as SQL and registering it as a **named type** in DuckDB at load time.
//!
//! `#[derive(DuckEnum)]` / `#[derive(DuckStruct)]` 的 `create_type` 走的就是这里：
//!
//! ```sql
//! CREATE TYPE IF NOT EXISTS "priority" AS ENUM ('low', 'medium', 'high');
//! CREATE TYPE IF NOT EXISTS "ticket" AS STRUCT(id BIGINT, priority ENUM('low', 'medium', 'high'));
//! ```
//!
//! 其中 `create_type = "print"` 不执行语句，只把渲染好的 DDL 收进队列
//! （[`queue_named_type_ddl`] / [`queue_enum_type_ddl`]）；等全部注册项跑完，扩展入口点
//! [`crate::register_all_duckfn`] 再调 [`flush_queued_type_ddl`] 把这批**一次性**打印出来
//! （两行 `-- [duckfn]` 提示 + 每行一条 DDL + 一行结束语），所以有几个 `"print"` 类型也只有一块提示。
//! `create_type = false`（宏完全不介入）时，插件作者也可以自己调 [`named_type_ddl`] +
//! [`print_sql_preview`] 把某一条 DDL 亮出来。
//!
//! The `"print"` mode of `create_type` never runs the statement: it queues the rendered DDL
//! ([`queue_named_type_ddl`] / [`queue_enum_type_ddl`]) and the extension entry point
//! ([`crate::register_all_duckfn`]) flushes the batch in one block afterwards
//! ([`flush_queued_type_ddl`]), so any number of print-mode types yields a single notice. With
//! `create_type = false`, where the macro stays out of the way, an author can show a single
//! statement with [`named_type_ddl`] + [`print_sql_preview`].
//!
//! 关键点是「SQL 文本从哪来」。DuckDB 的 C API 没有「逻辑类型 → 文本」的函数，
//! 但类型 introspection 齐全（`duckdb_get_type_id` + 子类型 / 字典 / 精度查询），
//! 所以 [`logical_type_sql`] 递归地把 `LogicalType` 渲染出来 —— 这意味着**任何**能给出逻辑类型的
//! 字段类型都能渲染，包括手写的自定义类型（它们的 `logical_type()` 本来就是引擎眼里的那个类型），
//! 而不需要为类型文本再维护一套「Rust 类型 → SQL 文本」的映射。
//!
//! The SQL text has to be built by introspection: the DuckDB C API has no "logical type to string"
//! function, but it does expose the type id plus child / dictionary / precision lookups. So
//! [`logical_type_sql`] renders a `LogicalType` recursively, which means **any** type that can
//! produce a logical type renders — hand-written custom ones included — with no parallel
//! "Rust type → SQL text" table to keep in sync.
//!
//! 语句一律用 `CREATE TYPE IF NOT EXISTS`：扩展可能被 `LOAD` 多次，重复执行不能报错，
//! 同时也不会覆盖用户已有的同名类型。执行路径与 SQL 宏相同（[`register_sql_macro_str`]，
//! 内部就是 `duckdb_query`）。
//!
//! Every statement uses `CREATE TYPE IF NOT EXISTS`: an extension may be loaded more than once, so
//! re-running must not fail, and a pre-existing type of that name is left alone. Execution goes
//! through the same path as the SQL macros ([`register_sql_macro_str`], i.e. `duckdb_query`).

use crate::register_sql_macro_str;
use crate::{DuckResult, duck_error};
use quack_rs::connection::Connection;
use quack_rs::prelude::{LogicalType, TypeId};
use std::sync::Mutex;

/// 用双引号包住标识符（类型名、STRUCT 字段名），并把内部的 `"` 转义成 `""`。
///
/// Quotes an identifier (a type name or a STRUCT field name), escaping inner `"` as `""`.
fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// 用单引号包住字符串字面量（ENUM 标签），并把内部的 `'` 转义成 `''`。
///
/// Quotes a string literal (an ENUM label), escaping inner `'` as `''`.
fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// 把一个 `LogicalType` 渲染成可用于 DDL 的 SQL 类型文本。
///
/// 支持：基础类型与包装类型（`BIGINT`、`VARCHAR`、`UUID`、`TIMESTAMP` …）、`DECIMAL(w, s)`、
/// `ENUM('a', 'b')`、`T[]`（LIST）、`T[N]`（ARRAY）、`MAP(K, V)`、`STRUCT("name" T, ...)`，
/// 并且可以任意嵌套。无法渲染的形状（如 `UNION`、`BIT`）会返回错误而不是拼出一段坏 SQL。
///
/// Renders a logical type as SQL usable in a DDL statement. Primitives and wrappers,
/// `DECIMAL(w, s)`, `ENUM('a', 'b')`, `T[]` (LIST), `T[N]` (ARRAY), `MAP(K, V)` and
/// `STRUCT("name" T, ...)` are supported and may nest arbitrarily. Shapes that cannot be rendered
/// (`UNION`, `BIT`, ...) produce an error instead of a broken statement.
///
/// # Errors
///
/// 逻辑类型里出现 DuckDB 的 `UNION` / `BIT` / 内部类型等无法写成 SQL 的形状时返回错误。
///
/// Returns an error when the logical type contains a shape with no SQL spelling.
pub fn logical_type_sql(logical_type: &LogicalType) -> DuckResult<String> {
    // SAFETY: 句柄由调用方在加载后的 DuckDB 运行时里持有（`logical_type()` 刚创建或来自子类型查询）。
    //
    // SAFETY: the handle is valid: the caller runs inside a loaded DuckDB and every child handle
    // below comes from an introspection call on a handle we own.
    let type_id = unsafe { logical_type.get_type_id() };
    let sql = match type_id {
        TypeId::Boolean => "BOOLEAN".to_owned(),
        TypeId::TinyInt => "TINYINT".to_owned(),
        TypeId::SmallInt => "SMALLINT".to_owned(),
        TypeId::Integer => "INTEGER".to_owned(),
        TypeId::BigInt => "BIGINT".to_owned(),
        TypeId::UTinyInt => "UTINYINT".to_owned(),
        TypeId::USmallInt => "USMALLINT".to_owned(),
        TypeId::UInteger => "UINTEGER".to_owned(),
        TypeId::UBigInt => "UBIGINT".to_owned(),
        TypeId::HugeInt => "HUGEINT".to_owned(),
        TypeId::UHugeInt => "UHUGEINT".to_owned(),
        TypeId::Float => "FLOAT".to_owned(),
        TypeId::Double => "DOUBLE".to_owned(),
        TypeId::Varchar => "VARCHAR".to_owned(),
        TypeId::Blob => "BLOB".to_owned(),
        TypeId::Uuid => "UUID".to_owned(),
        TypeId::Date => "DATE".to_owned(),
        TypeId::Time => "TIME".to_owned(),
        TypeId::TimeTz => "TIME WITH TIME ZONE".to_owned(),
        TypeId::Timestamp => "TIMESTAMP".to_owned(),
        TypeId::TimestampTz => "TIMESTAMP WITH TIME ZONE".to_owned(),
        TypeId::TimestampS => "TIMESTAMP_S".to_owned(),
        TypeId::TimestampMs => "TIMESTAMP_MS".to_owned(),
        TypeId::TimestampNs => "TIMESTAMP_NS".to_owned(),
        TypeId::Interval => "INTERVAL".to_owned(),
        TypeId::Decimal => format!(
            "DECIMAL({}, {})",
            unsafe { logical_type.decimal_width() },
            unsafe { logical_type.decimal_scale() }
        ),
        TypeId::Enum => {
            let size = unsafe { logical_type.enum_dictionary_size() };
            let members = (0..size)
                .map(|index| {
                    quote_literal(&unsafe { logical_type.enum_dictionary_value(u64::from(index)) })
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("ENUM({members})")
        }
        // LIST 在 SQL 里写成后缀形式 `T[]`；DuckDB 的 DDL 不认 `LIST(T)`。
        //
        // LIST is written as the postfix form `T[]`; DuckDB's DDL does not accept `LIST(T)`.
        TypeId::List => {
            format!("{}[]", logical_type_sql(&unsafe { logical_type.list_child_type() })?)
        }
        TypeId::Array => format!(
            "{}[{}]",
            logical_type_sql(&unsafe { logical_type.array_child_type() })?,
            unsafe { logical_type.array_size() }
        ),
        TypeId::Map => format!(
            "MAP({}, {})",
            logical_type_sql(&unsafe { logical_type.map_key_type() })?,
            logical_type_sql(&unsafe { logical_type.map_value_type() })?
        ),
        TypeId::Struct => {
            let count = unsafe { logical_type.struct_child_count() };
            let mut fields = Vec::with_capacity(count as usize);
            for index in 0..count {
                fields.push(format!(
                    "{} {}",
                    quote_identifier(&unsafe { logical_type.struct_child_name(index) }),
                    logical_type_sql(&unsafe { logical_type.struct_child_type(index) })?
                ));
            }
            format!("STRUCT({})", fields.join(", "))
        }
        other => {
            return Err(duck_error(format!(
                "cannot render the DuckDB logical type `{other:?}` as SQL: it has no SQL spelling \
                 (or duckfn has no mapping for it). Use `#[duck(create_type = false)]` and write the \
                 `CREATE TYPE` statement yourself."
            )));
        }
    };
    Ok(sql)
}

/// 生成「创建命名类型」的 SQL 语句（幂等）。
///
/// 类型名是**带引号**的，因此区分大小写；两个 derive 的 `sql_name` 默认取类型名的小写蛇形。
///
/// Builds an idempotent `CREATE TYPE` statement. The type name is quoted and therefore
/// case-sensitive; both derives default `sql_name` to the type name in lowercase snake_case.
pub fn named_type_ddl(name: &str, logical_type: &LogicalType) -> DuckResult<String> {
    Ok(format!(
        "CREATE TYPE IF NOT EXISTS {} AS {};",
        quote_identifier(name),
        logical_type_sql(logical_type)?
    ))
}

/// 在 catalog 里创建一个命名类型（幂等）。
///
/// 复用 SQL 宏那条执行路径，所以它既能被 `#[duck_custom_register]` 手写调用，也能像
/// `create_type = true` 那样在加载期自动执行。
///
/// Creates a named type in the catalog, idempotently, through the same execution path the SQL
/// macros use. It can be called from `#[duck_custom_register]` or, automatically, by
/// `create_type = true`.
///
/// ```ignore
/// #[duck_custom_register]
/// fn register_my_types(c: &Connection) -> DuckResult<()> {
///     // 建一个 STRUCT 类型：字段类型由 DuckDB 自己的逻辑类型渲染出来
///     duckfn::register_named_type(c, "ticket", Ticket::logical_type())
/// }
/// ```
pub fn register_named_type(
    connection: &Connection,
    name: &str,
    logical_type: LogicalType,
) -> DuckResult<()> {
    register_sql_macro_str(connection, &named_type_ddl(name, &logical_type)?)
}

/// 打一行 `-- [duckfn] ...` 提示。
///
/// 每行都以 `--` 开头（SQL 注释），所以提示与 DDL 混在一起也能整块复制去执行。
///
/// Prints one `-- [duckfn] ...` hint line. The `--` prefix makes it a SQL comment, so hints and DDL
/// can be copied and run as a single block.
fn eprintln_hint(hint: &str) {
    eprintln!("-- [duckfn] {hint}");
}

/// 把一段**不会执行**的 SQL 连同前后提示一起打印到 stderr（单个语句的即时版本）。
///
/// 输出三行：`-- [duckfn] {before}`、SQL 本身、`-- [duckfn] {after}`。前后两行都以 `--` 开头
/// （SQL 注释），所以整块直接复制进脚本执行也没问题；这样亮出来的 DDL 不会被误读成「已经执行过了」。
///
/// `create_type = "print"` 走的是「先收集、最后一次性打印」的批量路径（[`queue_named_type_ddl`] +
/// [`flush_queued_type_ddl`]）；这个函数是**单个语句**的即时出口，给 `create_type = false`
/// （宏完全不介入）的插件作者自己调，配上自己的说明：
///
/// Prints a single statement that is **not** run, framed by two caller-supplied hints (both are `--`
/// comments, so the block stays copy-pasteable as SQL). `create_type = "print"` goes through the
/// batched path instead ([`queue_named_type_ddl`] + [`flush_queued_type_ddl`]); this is the
/// immediate, single-statement flavour for authors — e.g. with `create_type = false`, where the
/// macro does not step in at all:
///
/// ```ignore
/// #[duck_custom_register]
/// fn show_the_create_type_ddl(_connection: &Connection) -> DuckResult<()> {
///     // create_type = false：要不要亮出 DDL、配什么说明，全部由作者决定
///     let ddl = duckfn::named_type_ddl("ticket", &Ticket::logical_type())?;
///     duckfn::print_sql_preview(
///         "my extension will NOT create this type",
///         &ddl,
///         "end - copy the statement above and run it yourself if you want it",
///     );
///     Ok(())
/// }
/// ```
pub fn print_sql_preview(before: &str, sql: &str, after: &str) {
    eprintln_hint(before);
    eprintln!("{}", sql.trim_end());
    eprintln_hint(after);
}

/// `create_type = "print"` 收集到的 DDL，等注册跑完由 [`flush_queued_type_ddl`] 一次性打印。
///
/// DDL collected by `create_type = "print"`, printed as one block by [`flush_queued_type_ddl`] once
/// registration has finished.
static QUEUED_TYPE_DDL: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// 渲染 `CREATE TYPE` 的 DDL 并**收进队列**（不执行、也不立刻打印）。
///
/// `#[duck(create_type = "print")]` 生成的注册函数调的就是它：`LOAD` 时先把每个类型要建的 DDL
/// 攒起来，等所有注册项跑完，入口点 [`crate::register_all_duckfn`] 再调 [`flush_queued_type_ddl`]
/// 一次性打印 —— 所以一个扩展里有几个 `"print"` 类型，也只有**一块**提示，不会每个类型重复一遍
/// 「未执行 / 可手动执行」。
///
/// Renders the `CREATE TYPE` DDL and queues it — it is neither run nor printed right away. This is
/// what the registration function generated for `#[duck(create_type = "print")]` calls: the DDLs are
/// collected during `LOAD` and the entry point ([`crate::register_all_duckfn`]) flushes them in one
/// block afterwards ([`flush_queued_type_ddl`]), so any number of print-mode types produces a single
/// notice instead of one per type.
///
/// # Errors
///
/// 逻辑类型无法渲染成 SQL 时返回错误（同 [`named_type_ddl`]）。
///
/// Returns an error when the logical type has no SQL spelling (as [`named_type_ddl`] does).
pub fn queue_named_type_ddl(name: &str, logical_type: &LogicalType) -> DuckResult<()> {
    let ddl = named_type_ddl(name, logical_type)?;
    QUEUED_TYPE_DDL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(ddl);
    Ok(())
}

/// 把 [`queue_named_type_ddl`] / [`queue_enum_type_ddl`] 收集到的 DDL 一次性打印出来（队列为空则什么都不做）。
///
/// 由扩展入口点 [`crate::register_all_duckfn`] 在所有注册项跑完后调用，因此整份输出只有一块提示：
/// 两行 `-- [duckfn]` 说明 + 每行一条 DDL + 一行结束语。DDL 一行一条，方便挑需要的复制执行。
/// 输出走 stderr，不会混进查询结果。
///
/// Prints every DDL collected by [`queue_named_type_ddl`] / [`queue_enum_type_ddl`] in one block, and
/// does nothing when the queue is empty. It is called by the extension entry point
/// ([`crate::register_all_duckfn`]) once every registration has run, so the whole output is a single
/// notice: two `-- [duckfn]` hint lines, one DDL per line, and a closing line. One DDL per line keeps
/// them easy to copy. Going to stderr keeps query results clean.
pub fn flush_queued_type_ddl() {
    let statements = {
        let mut queued = QUEUED_TYPE_DDL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if queued.is_empty() {
            return;
        }
        std::mem::take(&mut *queued)
    };

    if statements.len() == 1 {
        eprintln_hint("create_type = \"print\": the statement below was NOT executed.");
        eprintln_hint("Copy it and run it yourself if you want the type created.");
    } else {
        eprintln_hint(&format!(
            "create_type = \"print\": the {} statements below were NOT executed.",
            statements.len()
        ));
        eprintln_hint("Copy the ones you need and run them yourself.");
    }
    for statement in &statements {
        eprintln!("{statement}");
    }
    eprintln_hint("end - nothing above was executed.");
}

/// ENUM 的逻辑类型；空字典直接报错。
///
/// The logical type of an ENUM dictionary; an empty dictionary is an error.
fn enum_logical_type(name: &str, members: &[&str]) -> DuckResult<LogicalType> {
    if members.is_empty() {
        return Err(duck_error(format!(
            "cannot create ENUM type `{name}`: at least one member is required"
        )));
    }
    Ok(LogicalType::enum_type(members))
}

/// 在 catalog 里创建一个 ENUM 类型（幂等）。
///
/// [`register_named_type`] 的便捷版本：字典直接给标签列表，不需要先有 `DuckValueType` 实现。
///
/// The convenience flavour of [`register_named_type`]: the dictionary is given as a list of labels,
/// and no `DuckValueType` implementation is needed.
///
/// ```ignore
/// #[duck_custom_register]
/// fn register_my_types(c: &Connection) -> DuckResult<()> {
///     duckfn::register_enum_type(c, "color", &["red", "green", "blue"])
/// }
/// ```
pub fn register_enum_type(connection: &Connection, name: &str, members: &[&str]) -> DuckResult<()> {
    register_named_type(connection, name, enum_logical_type(name, members)?)
}

/// 渲染 ENUM 的 `CREATE TYPE` DDL 并收进队列：[`queue_named_type_ddl`] 的 ENUM 便捷版本。
///
/// [`register_enum_type`] 的「只排队、不执行」对应物，`#[duck(create_type = "print")]` 在枚举上走的
/// 就是它；字典照样直接给标签列表，不需要先有 `DuckValueType` 实现。
///
/// The queueing counterpart of [`register_enum_type`] and the ENUM flavour of
/// [`queue_named_type_ddl`]; `#[duck(create_type = "print")]` on an enum calls exactly this. The
/// dictionary is still a plain label list, so no `DuckValueType` implementation is needed.
///
/// ```ignore
/// #[duck_custom_register]
/// fn queue_my_types(_connection: &Connection) -> DuckResult<()> {
///     duckfn::queue_enum_type_ddl("color", &["red", "green", "blue"])
/// }
/// ```
pub fn queue_enum_type_ddl(name: &str, members: &[&str]) -> DuckResult<()> {
    queue_named_type_ddl(name, &enum_logical_type(name, members)?)
}
