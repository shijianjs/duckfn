use duckfn::{duck_sql_macro, duck_sql_macro_files, DuckResult};
use quack_rs::prelude::SqlMacro;

// ============================================================================
// duck_sql_macro：SQL 宏的注册
//
// SQL 宏不是 C++/FFI 回调，而是扩展初始化阶段执行的一条
// `CREATE OR REPLACE MACRO ...` 语句，之后由 DuckDB 的 SQL 层解析。
// duckfn-macro/src/duck_function.rs::build_sql_macro 按「函数返回类型」选四种收尾：
//
//   -> SqlMacro                    直接构造 SqlMacro，register_sql_macro(builder)
//   -> DuckResult<SqlMacro>        同上，构造失败会中止注册并冒泡错误
//   -> String / &'static str       当成完整 SQL 语句，由 register_sql_macro_str 执行
//                                  （可以 include_str! 内联外部 .sql 脚本，见文末「include_str!」一节）
//   -> DuckResult<String> / <&'static str>
//                                  同上，Err 时冒泡错误
//
// 两类宏：
//   SqlMacro::scalar(name, params, expr)  ->  CREATE OR REPLACE MACRO name(p) AS (expr)
//   SqlMacro::table(name, params, query)  ->  CREATE OR REPLACE MACRO name(p) AS TABLE query
//
// 名称与参数名统一走 quack_rs::validate::validate_function_name：
// [A-Za-z_][A-Za-z0-9_]*（大小写不敏感、允许下划线开头、最长 256），
// 因此 SQL 里不需要引号；宏体（expr/query）由扩展作者书写，不做转义。
//
// 除了「一函数一宏」的 `#[duck_sql_macro]`，还有函数式宏
// `duck_sql_macro_files!("a.sql", "b.sql", ...)`：直接给若干 .sql 文件路径一次注册，
// 不用写函数（见文末「duck_sql_macro_files!」一节）。
//
// 下面按「返回类型 / 参数形态 / 表达式类型 / 宏组合 / 表宏 / 脚本文件导入 / 命名」分组覆盖各场景。
// ============================================================================

// ============================================================================
// 返回类型 1/4：-> SqlMacro（最直接的构造路径）
// ============================================================================

/// 三参数标量宏，表达式复用两个参数。`greatest/least` 让它在整数与字符串上都成立
/// ```sql
/// SELECT dfn_macro_clamp(15, 0, 10);
/// SELECT dfn_macro_clamp('bb', 'a', 'c');
/// ```
#[duck_sql_macro]
pub fn dfn_macro_clamp() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_clamp", &["x", "lo", "hi"], "greatest(lo, least(hi, x))")
        .expect("dfn_macro_clamp: invalid macro name")
}

/// 零参数标量宏：参数列表为空，SQL 侧必须写成 `()`，且无返回类型推导
/// ```sql
/// SELECT dfn_macro_pi();
/// ```
#[duck_sql_macro]
pub fn dfn_macro_pi() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_pi", &[], "3.14159265358979")
        .expect("dfn_macro_pi: invalid macro name")
}

/// 多参数且每个参数都参与表达式：验证参数顺序不被搞混
/// ```sql
/// SELECT dfn_macro_weighted(1, 2, 3);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_weighted() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_weighted", &["a", "b", "c"], "a * 100 + b * 10 + c")
        .expect("dfn_macro_weighted: invalid macro name")
}

/// 数值表达式：`* 1.0` 把整数除法提升成浮点
/// ```sql
/// SELECT dfn_macro_norm(5, 0, 10);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_norm() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_norm", &["x", "lo", "hi"], "(x - lo) * 1.0 / (hi - lo)")
        .expect("dfn_macro_norm: invalid macro name")
}

/// 字符串表达式：字符串拼接要用 `concat`，不能用 `+`
/// ```sql
/// SELECT dfn_macro_concat('he', 'llo');
/// ```
#[duck_sql_macro]
pub fn dfn_macro_concat() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_concat", &["s", "t"], "concat(s, t)")
        .expect("dfn_macro_concat: invalid macro name")
}

// ============================================================================
// 返回类型 2/4：-> DuckResult<SqlMacro>（构造失败可冒泡）
// ============================================================================

/// `?` 把 `SqlMacro::scalar` 的 ExtensionError 直接交给注册流程
/// ```sql
/// SELECT dfn_macro_add(2, 3);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_add() -> DuckResult<SqlMacro> {
    Ok(SqlMacro::scalar("dfn_macro_add", &["a", "b"], "a + b")?)
}

/// 表宏：`AS TABLE`，参数是标量（DuckDB 的表宏参数由 body 里的用法决定）
/// ```sql
/// SELECT * FROM dfn_macro_gen(3);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_gen() -> DuckResult<SqlMacro> {
    Ok(SqlMacro::table("dfn_macro_gen", &["n"], "SELECT * FROM range(n)")?)
}

/// 零参数表宏
/// ```sql
/// SELECT * FROM dfn_macro_constants();
/// ```
#[duck_sql_macro]
pub fn dfn_macro_constants() -> DuckResult<SqlMacro> {
    Ok(SqlMacro::table(
        "dfn_macro_constants",
        &[],
        "SELECT 1 AS one, 'x' AS s",
    )?)
}

// ============================================================================
// 返回类型 3/4：-> String（直接产出一条 SQL 语句）
// ============================================================================

/// `String` 路径：注册时执行整条 `CREATE OR REPLACE MACRO`
/// ```sql
/// SELECT dfn_macro_double(21);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_double() -> String {
    "CREATE OR REPLACE MACRO dfn_macro_double(x) AS (x * 2)".to_string()
}

/// 同一条 String 路径也能建表宏
/// ```sql
/// SELECT * FROM dfn_macro_series(3);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_series() -> String {
    "CREATE OR REPLACE MACRO dfn_macro_series(n) AS TABLE \
     SELECT x, x * x AS sq FROM range(n) t(x)"
        .to_string()
}

// ============================================================================
// 返回类型 4/4：-> DuckResult<String> / DuckResult<&'static str>
// ============================================================================

/// `DuckResult<String>`：Err 会让扩展注册失败（这里恒 Ok）
/// ```sql
/// SELECT dfn_macro_square(7);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_square() -> DuckResult<String> {
    Ok("CREATE OR REPLACE MACRO dfn_macro_square(x) AS (x * x)".to_string())
}

/// `&'static str`：不经过堆分配
/// ```sql
/// SELECT dfn_macro_negate(7);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_negate() -> &'static str {
    "CREATE OR REPLACE MACRO dfn_macro_negate(x) AS (-x)"
}

/// `DuckResult<&'static str>`：静态字符串 + 可冒泡错误
/// ```sql
/// SELECT dfn_macro_half(7);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_half() -> DuckResult<&'static str> {
    Ok("CREATE OR REPLACE MACRO dfn_macro_half(x) AS (x / 2)")
}

/// 静态字符串建固定内容的表宏
/// ```sql
/// SELECT * FROM dfn_macro_static_gen();
/// ```
#[duck_sql_macro]
pub fn dfn_macro_static_gen() -> &'static str {
    "CREATE OR REPLACE MACRO dfn_macro_static_gen() AS TABLE SELECT * FROM range(2)"
}

/// 静态字符串的表宏同样可以带参数
/// ```sql
/// SELECT * FROM dfn_macro_bounds(1, 4);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_bounds() -> DuckResult<&'static str> {
    Ok("CREATE OR REPLACE MACRO dfn_macro_bounds(lo, hi) AS TABLE SELECT * FROM range(lo, hi)")
}

// ============================================================================
// include_str!：直接内联外部 .sql 脚本
//
// 字符串返回形式并不要求把 SQL 写进 Rust 源码字面量，也可以用 include_str!
// 在编译期把一个 .sql 文件读成 &'static str 再交给 register_sql_macro_str。
// 好处：
//   - SQL 单独成文件，编辑器有方言高亮、可被 SQL linter / 测试工具直接复用；
//   - 一份脚本里可以放多条语句（分号分隔），一次注册多个宏 —— 相当于
//     「导入脚本」而不是「写死一条 CREATE MACRO」；
//   - 注释用 SQL 的 `--`，不需要转义。
// 脚本路径相对当前 .rs 文件（src/extension/functions/sql/）。
// ============================================================================

/// 一个脚本文件注册三个宏（两个标量 + 一个表宏），注释与分号都在脚本里
/// ```sql
/// SELECT dfn_macro_inc_add(2, 3);
/// SELECT dfn_macro_inc_triple(4);
/// SELECT * FROM dfn_macro_inc_gen(3);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_inc_script() -> &'static str {
    include_str!("sql/macro_inc.sql")
}

/// `DuckResult<&'static str>` + include_str!：同一个脚本文件也能走可冒泡错误路径
/// ```sql
/// SELECT dfn_macro_inc_negate(7);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_inc_script_checked() -> DuckResult<&'static str> {
    Ok(include_str!("sql/macro_inc_checked.sql"))
}

// ============================================================================
// duck_sql_macro_files!：一次注册多个 .sql 文件（快捷方式）
//
// 上面两种写法都要先写一个 `#[duck_sql_macro]` 函数，再用 include_str! 返回脚本。
// `duck_sql_macro_files!` 把这层样板去掉：直接给出若干文件路径，宏展开成一次
// inventory 注册，按书写顺序对每个文件执行 register_sql_macro_str(include_str!(..))。
//
//   - 参数是可变多个字符串字面量（文件路径），支持尾随逗号，至少一个；
//   - 路径相对「调用本宏的 .rs 文件」，编译期内联；
//   - 单个文件里仍可含多条分号分隔的语句。
//
// 下面一次导入 3 个文件：a 里两个标量宏、b 里一个表宏、c 里一个标量宏。
// ============================================================================

duck_sql_macro_files!(
    "sql/macro_files_a.sql",
    "sql/macro_files_b.sql",
    "sql/macro_files_c.sql"
);

// ============================================================================
// 表达式类型：STRUCT / LIST 也可以在宏体里直接构造
// ============================================================================

/// 返回 STRUCT 的标量宏
/// ```sql
/// SELECT dfn_macro_pair(5);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_pair() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_pair", &["x"], "{'a': x, 'b': x * 2}")
        .expect("dfn_macro_pair: invalid macro name")
}

/// 返回 LIST 的标量宏
/// ```sql
/// SELECT dfn_macro_mklist(5);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_mklist() -> SqlMacro {
    SqlMacro::scalar("dfn_macro_mklist", &["x"], "[x, x * 2]")
        .expect("dfn_macro_mklist: invalid macro name")
}

// ============================================================================
// 宏组合：一个 Rust 函数一次注册多条 SQL
//
// DuckDB 在 CREATE MACRO 时就会解析宏体，被引用的宏必须已经存在；
// 而 inventory 的注册顺序不确定，所以「引用了另一个 dfn_macro_* 的宏」
// 必须把自己的依赖一并建出来 —— 返回多条语句的 String 即可（duckdb_query
// 支持分号分隔的多条语句，取最后一条的结果）。
// ============================================================================

/// 先建被依赖的 dfn_macro_double，再建调用它两次的 dfn_macro_quad
/// ```sql
/// SELECT dfn_macro_quad(3);
/// ```
#[duck_sql_macro]
pub fn dfn_macro_quad() -> DuckResult<String> {
    Ok("CREATE OR REPLACE MACRO dfn_macro_double(x) AS (x * 2); \
        CREATE OR REPLACE MACRO dfn_macro_quad(x) AS (dfn_macro_double(dfn_macro_double(x)))"
        .to_string())
}

// ============================================================================
// 命名：允许大小写混合，SQL 侧大小写不敏感
// ============================================================================

/// 名称校验只拒绝需要引号的字符，不强制 snake_case；DuckDB 标识符大小写不敏感
/// ```sql
/// SELECT dfnmacrocamel(1);
/// SELECT DFNMACROCAMEL(2);
/// ```
#[duck_sql_macro]
#[allow(non_snake_case)] // 故意用大小写混合，验证名字校验不强制 snake_case
pub fn dfnMacroCamel() -> SqlMacro {
    SqlMacro::scalar("dfnMacroCamel", &["x"], "x + 1")
        .expect("dfnMacroCamel: invalid macro name")
}
