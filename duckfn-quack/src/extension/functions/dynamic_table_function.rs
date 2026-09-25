// 动态列表函数示例：输出列在 bind 阶段才确定。
//
// Dynamic-column table functions: the output columns are only decided during bind.
//
// 普通表函数的输出列由 `#[derive(DuckStruct)]` 结构体在编译期固定；当列本身要读外部元数据
// （文件头、字典表、远端 schema）才知道时，用 `dynamic_columns = true`（或手写
// `DynamicTableFunctionAdapter`）走这条动态通路。
//
// An ordinary table function fixes its output columns at compile time through a
// `#[derive(DuckStruct)]` row struct. When the columns can only be known after reading external
// metadata (a file header, a dictionary table, a remote schema), take this dynamic path with
// `dynamic_columns = true` (or a hand-written `DynamicTableFunctionAdapter`).

use duckfn::{
    BindInfo, DuckBindArgs, DuckDynamicRow, DuckDynamicTable, DuckDynamicValue, DuckOptionResult,
    DuckResult, DuckResultSchema, DuckStruct, DuckTypeDesc, DynamicTableFunctionAdapter,
    LogicalType, duck_custom_register, duck_error, duck_table_function,
};
use quack_rs::prelude::{Connection, Registrar, TypeId};

// ============================================================================
// 宏层：`#[duck_table_function(dynamic_columns = true)]`
//
// 函数在 bind 阶段拿到参数，读外部元数据算出 schema，再连同行迭代器一起返回
// （`-> DuckResult<DuckDynamicTable>`：读元数据失败会让整条查询失败）。
//
// Macro layer: with `#[duck_table_function(dynamic_columns = true)]` the function receives its
// arguments during bind, reads the external metadata to work out the schema, and returns it
// together with the row iterator (`-> DuckResult<DuckDynamicTable>`, so a metadata failure fails
// the whole query).
// ============================================================================

/// 伪「外部元数据」：真实场景里这里会去读文件头 / 查字典表 / 调远端 schema 接口。
///
/// A stand-in for "external metadata": in a real extension this would read a file header, query a
/// dictionary table or call a remote schema endpoint.
///
/// 返回的 schema 覆盖了标量、可空标量、`LIST`、`STRUCT` 与 `MAP` 列，用来演示运行时动态类型。
///
/// The returned schema covers scalar, nullable scalar, `LIST`, `STRUCT` and `MAP` columns, to
/// exercise the runtime-dynamic type handling.
fn demo_external_metadata(source: &str) -> DuckResult<DuckResultSchema> {
    match source {
        "sales" => Ok(DuckResultSchema::new(vec![
            ("id".to_string(), DuckTypeDesc::scalar(TypeId::BigInt)),
            (
                "region".to_string(),
                DuckTypeDesc::scalar(TypeId::Varchar),
            ),
            ("amount".to_string(), DuckTypeDesc::scalar(TypeId::Double)),
            (
                "tags".to_string(),
                DuckTypeDesc::list(DuckTypeDesc::scalar(TypeId::Varchar)),
            ),
            (
                "info".to_string(),
                DuckTypeDesc::struct_type([
                    ("host".to_string(), DuckTypeDesc::scalar(TypeId::Varchar)),
                    ("code".to_string(), DuckTypeDesc::scalar(TypeId::BigInt)),
                ]),
            ),
            (
                "attrs".to_string(),
                DuckTypeDesc::map(
                    DuckTypeDesc::scalar(TypeId::Varchar),
                    DuckTypeDesc::scalar(TypeId::BigInt),
                ),
            ),
        ])),
        other => Err(duck_error(format!(
            "dfn_table_dynamic: unknown source `{other}`, valid sources are: sales"
        ))),
    }
}

/// 动态列出「按外部元数据推断出来的列」，`n` 行。
///
/// ```sql
/// DESCRIBE SELECT * FROM dfn_table_dynamic('sales', 3);
/// SELECT id, region, amount, tags, info, attrs FROM dfn_table_dynamic('sales', 3);
/// SELECT * FROM dfn_table_dynamic('nope', 1);   -- error: unknown source `nope`
/// ```
///
/// 列的行为（用来覆盖动态写出的各种形态）：
/// - `id`：不可空 `BIGINT`；
/// - `region`：`VARCHAR`；
/// - `amount`：每两行一个 NULL（可空 `DOUBLE`）；
/// - `tags`：每 5 行整个 LIST 是 NULL（可空 `VARCHAR[]`），其余是 0~3 个元素；
/// - `info`：每 3 行整个 STRUCT 是 NULL，其余是 `{host, code}`；
/// - `attrs`：`MAP(VARCHAR, BIGINT)`。
///
/// Lists the columns inferred from the external metadata, `n` rows deep. The per-column behaviour
/// exercises every dynamic shape: a non-nullable `BIGINT`, a `VARCHAR`, a nullable `DOUBLE` (NULL
/// every second row), a nullable `VARCHAR[]` whose whole list is NULL every fifth row, a nullable
/// `STRUCT{host, code}` that is NULL every third row, and a `MAP(VARCHAR, BIGINT)`.
#[duck_table_function(dynamic_columns = true)]
fn dfn_table_dynamic(source: String, n: i64) -> DuckResult<DuckDynamicTable> {
    // bind 阶段：读外部元数据、把 schema 定下来。
    //
    // Bind phase: read the external metadata and pin the schema down.
    let schema = demo_external_metadata(&source)?;

    let rows = (0..n.max(0)).map(|i| -> DuckOptionResult<DuckDynamicRow> {
        let tags = if i % 5 == 4 {
            None
        } else {
            Some(DuckDynamicValue::list((0..(i % 4)).map(|t| {
                Some(DuckDynamicValue::Varchar(format!("t{t}")))
            })))
        };
        let info = if i % 3 == 2 {
            None
        } else {
            Some(DuckDynamicValue::struct_value([
                Some(DuckDynamicValue::Varchar(format!("host-{}", i % 3))),
                Some(DuckDynamicValue::BigInt(i * 10)),
            ]))
        };
        let attrs = DuckDynamicValue::map((0..(i % 2)).map(|k| {
            (
                DuckDynamicValue::Varchar(format!("k{k}")),
                DuckDynamicValue::BigInt(k),
            )
        }));
        Ok(Some(DuckDynamicRow::new(vec![
            Some(DuckDynamicValue::BigInt(i)),
            Some(DuckDynamicValue::Varchar(format!("r{}", i % 3))),
            if i % 2 == 0 {
                Some(DuckDynamicValue::Double(i as f64 / 2.0))
            } else {
                None
            },
            tags,
            info,
            Some(attrs),
        ])))
    });

    Ok(DuckDynamicTable::new(schema, Box::new(rows)))
}

/// 零行也要能正确声明列：schema 只来自外部元数据，不由数据决定。
///
/// ```sql
/// DESCRIBE SELECT * FROM dfn_table_dynamic_empty('sales');
/// SELECT count(*) FROM dfn_table_dynamic_empty('sales');  -- 0
/// ```
///
/// Even an empty result declares its columns: the schema comes from the external metadata alone,
/// never from the data.
#[duck_table_function(dynamic_columns = true)]
fn dfn_table_dynamic_empty(source: String) -> DuckResult<DuckDynamicTable> {
    let schema = demo_external_metadata(&source)?;
    let rows = std::iter::empty::<DuckOptionResult<DuckDynamicRow>>();
    Ok(DuckDynamicTable::new(schema, Box::new(rows)))
}

// ============================================================================
// 底层：手写 `DynamicTableFunctionAdapter`
//
// 不想用宏、或者要在 bind 里做更复杂的事情时，直接实现这个 trait 即可：
// 只写 `NAME` / `Args` / `bind`，builder、with_state、scan 都有默认实现。
//
// Low level: a hand-written `DynamicTableFunctionAdapter`. When the macro does not fit, or bind has
// to do something more involved, implement the trait directly: only `NAME` / `Args` / `bind` are
// required; the builder, with_state and scan all have defaults.
// ============================================================================

/// 底层示例的参数：`start` 之前是位置参数，`label` 是命名参数（可空，缺省为 `n`）。
///
/// Arguments of the low-level example: everything from `start` on is a named parameter, and
/// `label` is optional (it defaults to `n`).
#[derive(Default, Debug, Clone, DuckStruct)]
#[duck(named_param_from = "start")]
struct DynamicCountdownArgs {
    start: i64,
    label: Option<String>,
}

/// 底层示例：列名与列数是 bind 阶段由参数算出来的，迭代器也在这里造好。
///
/// ```sql
/// SELECT * FROM dfn_table_dynamic_manual(start=3, label='step');
/// ```
///
/// The low-level example: the column names and even the column count are worked out during bind
/// from the arguments, and the iterator is built there too.
struct DynamicCountdown;

impl DynamicTableFunctionAdapter for DynamicCountdown {
    const NAME: &'static str = "dfn_table_dynamic_manual";
    type Args = DynamicCountdownArgs;

    fn bind(args: Self::Args) -> DuckResult<DuckDynamicTable> {
        // 先取值再构造闭包：迭代器要 'static，不能借用 `args` 这个局部变量。
        //
        // Copy the values out first: the iterator must be 'static and cannot borrow the local
        // `args`.
        let start = args.start;
        let label = args.label.unwrap_or_else(|| "n".to_string());
        let schema = DuckResultSchema::new(vec![
            (label, DuckTypeDesc::scalar(TypeId::BigInt)),
            ("squared".to_string(), DuckTypeDesc::scalar(TypeId::BigInt)),
        ]);

        let rows = (0..start.max(0)).map(move |i| -> DuckOptionResult<DuckDynamicRow> {
            let value = start - i;
            Ok(Some(DuckDynamicRow::new(vec![
                Some(DuckDynamicValue::BigInt(value)),
                Some(DuckDynamicValue::BigInt(value * value)),
            ])))
        });

        Ok(DuckDynamicTable::new(schema, Box::new(rows)))
    }
}

/// 手动注册底层动态表函数（`dynamic_columns` 走宏默认的自动注册，这里演示手动注册）。
///
/// Manually registers the low-level dynamic table function (the macro path above auto-registers;
/// this shows the manual route).
#[duck_custom_register]
fn dfn_table_dynamic_manual_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_table(DynamicCountdown::table_function_builder()?) }
}

// ============================================================================
// 底层：手写 `DuckBindArgs`，用 `DuckDynamicValue` 读取原始 `Value`
//
// `#[derive(DuckStruct)]` 只能把参数读成「编译期已知的 Rust 类型」。当你希望按**运行时描述**读值
// （值来自 `MAP` / `LIST` 的子元素，或类型到运行时才知道）时，手写 `DuckBindArgs` 即可：
// 用一个 `DuckTypeDesc` 描述该参数，再交给 `DuckDynamicValue::from_duck_value` 转换 —— 它连
// `arg = NULL` 这种情况都会正确读成 `None`。
//
// Low level: a hand-written `DuckBindArgs` reading the raw `Value` through `DuckDynamicValue`.
// `#[derive(DuckStruct)]` can only read arguments into types known at compile time; when the value
// should be read against a *runtime* description — it comes from a `MAP` / `LIST` child, or its
// type is only known at run time — implement `DuckBindArgs` directly: describe the parameter with a
// `DuckTypeDesc` and let `DuckDynamicValue::from_duck_value` convert it. That conversion also reads
// an explicit `arg = NULL` as `None`.
// ============================================================================

/// 手写的 bind 参数：只声明类型（`BIGINT`），值按运行时描述读成动态值。
///
/// A hand-written bind argument: it only declares the type (`BIGINT`) and reads the value against a
/// runtime description instead.
#[derive(Default, Debug, Clone)]
struct DynamicEchoArgs {
    value: Option<DuckDynamicValue>,
}

impl DuckBindArgs for DynamicEchoArgs {
    fn read_bind_args(bind: &BindInfo) -> DuckResult<Self> {
        // SAFETY: 位置参数 0 已由 `bind_param_logical` 声明为 BIGINT。
        //
        // SAFETY: positional argument 0 is declared as BIGINT by `bind_param_logical`.
        let raw = unsafe { bind.get_parameter_value(0) };
        let desc = DuckTypeDesc::scalar(TypeId::BigInt);
        Ok(Self {
            value: DuckDynamicValue::from_duck_value(&raw, &desc)?,
        })
    }

    fn bind_param_logical() -> Vec<(Option<String>, LogicalType)> {
        vec![(None, LogicalType::new(TypeId::BigInt))]
    }
}

/// 把这个 `BIGINT` 参数原样回显成一行一列，`NULL` 也原样回显。
///
/// ```sql
/// SELECT * FROM dfn_table_dynamic_echo_arg(42);    -- 42
/// SELECT * FROM dfn_table_dynamic_echo_arg(NULL);  -- NULL（显式 NULL 也能正确读成 None）
/// ```
///
/// Echoes that `BIGINT` argument back as a single row and column, `NULL` included.
struct DynamicEchoArg;

impl DynamicTableFunctionAdapter for DynamicEchoArg {
    const NAME: &'static str = "dfn_table_dynamic_echo_arg";
    type Args = DynamicEchoArgs;

    fn bind(args: Self::Args) -> DuckResult<DuckDynamicTable> {
        let schema = DuckResultSchema::from_scalar_types([("v", TypeId::BigInt)]);
        let rows = std::iter::once::<DuckOptionResult<DuckDynamicRow>>(Ok(Some(
            DuckDynamicRow::new(vec![args.value]),
        )));
        Ok(DuckDynamicTable::new(schema, Box::new(rows)))
    }
}

/// 手动注册「动态回显参数」表函数。
///
/// Manually registers the "echo a dynamically-read argument" table function.
#[duck_custom_register]
fn dfn_table_dynamic_echo_arg_register(c: &Connection) -> DuckResult<()> {
    unsafe { c.register_table(DynamicEchoArg::table_function_builder()?) }
}
