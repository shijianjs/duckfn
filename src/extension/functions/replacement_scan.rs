use duckfn::{
    duck_custom_register, duck_error, duck_replacement_scan, duck_table_function, DuckOptionResult,
    DuckResult, DuckStruct,
};
use quack_rs::prelude::Connection;

// ============================================================================
// duck_replacement_scan：把「DuckDB 不认识的表名」重定向到表函数
//
// DuckDB 遇到无法解析的表引用时会按注册顺序调用所有 replacement scan 回调，
// 字符串字面量也算（这就是 `SELECT * FROM 'data.points'` 能工作的原因）。回调有三种出口：
//
//   Ok(Some(table_function))  接管：path 作为第一个 VARCHAR 参数传给该表函数
//   Ok(None)                  不接管：DuckDB 继续尝试下一个回调（最终可能报「表不存在」）
//   Err / panic               整条查询报错（panic 由适配层 catch_unwind 转成错误）
//
// 返回形式（duckfn-macro/src/duck_function.rs::replacement_scan_return_type）：
//   -> Option<String>                    命中才重定向
//   -> Option<&'static str>              同上，名字是静态字符串
//   -> DuckOptionResult<String>          命中才重定向，且可以报错
//   -> DuckOptionResult<&'static str>    同上
//
// 只有「命中才重定向」这几种形式是安全的：回调对**每个**未解析的表名都会被调用，
// 永远返回 Some 会把别人的表名也抢走，所以这里没有「总是接管」的返回形式。
//
// 下面用「假文件路径」当数据源，测试不依赖真实文件：
//   `<n>.points`  -> n 行点 (i, i*i)
//   `任意.echo`   -> 一行，回显 replacement scan 传进来的 path（证明参数确实传到了）
// ============================================================================

/// replacement scan 命中的目标表函数输出：两列都来自这个结构体的字段
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct ScanPoint {
    x: i64,
    y: i64,
}

/// 目标表函数之一：既能被 replacement scan 调用，也能直接 `FROM dfn_scan_read_points('3.points')`
/// ```sql
/// SELECT * FROM dfn_scan_read_points('3.points');
/// SELECT * FROM '3.points';
/// ```
#[duck_table_function]
fn dfn_scan_read_points(path: String) -> DuckResult<impl Iterator<Item = ScanPoint>> {
    let n = scan_parse_points(&path)?;
    Ok((0..n).map(|i| ScanPoint { x: i, y: i * i }))
}

/// 目标表函数的输出：path 与它的字符数
#[derive(Default, Debug, Clone, DuckStruct)]
pub struct ScanEcho {
    path: String,
    len: i64,
}

/// 目标表函数之二：回显参数，用来验证「路径作为第一个 VARCHAR 参数传入」
/// ```sql
/// SELECT * FROM dfn_scan_read_echo('a.echo');
/// ```
#[duck_table_function]
fn dfn_scan_read_echo(path: String) -> impl Iterator<Item = ScanEcho> {
    let len = path.chars().count() as i64;
    std::iter::once(ScanEcho { path, len })
}

fn scan_parse_points(path: &str) -> DuckResult<i64> {
    let stem = path.strip_suffix(".points").ok_or_else(|| {
        duck_error(format!("dfn_scan_read_points: not a .points file: {path}"))
    })?;
    stem.parse::<i64>().map_err(|_| {
        duck_error(format!(
            "dfn_scan_read_points: file name must be an integer: {path}"
        ))
    })
}

// ============================================================================
// 返回形式 1/4：-> DuckOptionResult<String>（命中 / 不管 / 报错 / panic 全都能表达）
// ============================================================================

/// 命中 `.points`；`.error` 返回 Err；`.panic` 直接 panic
/// ```sql
/// SELECT * FROM '3.points';
/// SELECT * FROM 'boom.error';
/// SELECT * FROM 'boom.panic';
/// ```
#[duck_replacement_scan]
fn dfn_scan_points(path: &str) -> DuckOptionResult<String> {
    if path.ends_with(".panic") {
        panic!("dfn_scan_points: panic while handling {path}");
    }
    if path.ends_with(".error") {
        return Err(duck_error(format!("dfn_scan_points: refuses {path}")));
    }
    if path.ends_with(".points") {
        return Ok(Some("dfn_scan_read_points".to_string()));
    }
    Ok(None)
}

// ============================================================================
// 返回形式 2/4：-> Option<String>（入参写成 String，验证宏的另一种签名）
// ============================================================================

/// 命中 `.echo`，否则不管
/// ```sql
/// SELECT * FROM 'hi.echo';
/// ```
#[duck_replacement_scan]
fn dfn_scan_echo(path: String) -> Option<String> {
    if path.ends_with(".echo") {
        return Some("dfn_scan_read_echo".to_string());
    }
    None
}

// ============================================================================
// 返回形式 3/4：-> Option<&'static str>（静态名字，不分配 String）
// ============================================================================

/// 命中 `.static`，目标名字是编译期常量
/// ```sql
/// SELECT * FROM 'hi.static';
/// ```
#[duck_replacement_scan]
fn dfn_scan_static(path: &str) -> Option<&'static str> {
    if path.ends_with(".static") {
        return Some("dfn_scan_read_echo");
    }
    None
}

// ============================================================================
// 返回形式 4/4：-> DuckOptionResult<&'static str>
// ============================================================================

/// 命中 `.checked`；`.checked_fail` 返回 Err
/// ```sql
/// SELECT * FROM 'hi.checked';
/// SELECT * FROM 'x.checked_fail';
/// ```
#[duck_replacement_scan]
fn dfn_scan_static_checked(path: &str) -> DuckOptionResult<&'static str> {
    if path.ends_with(".checked_fail") {
        return Err(duck_error(format!("dfn_scan_static_checked: bad name {path}")));
    }
    if path.ends_with(".checked") {
        return Ok(Some("dfn_scan_read_echo"));
    }
    Ok(None)
}

// ============================================================================
// 接管到一个不存在的表函数：错误由 DuckDB 在绑定阶段抛出
// ============================================================================

/// ```sql
/// SELECT * FROM 'hi.missing';
/// ```
#[duck_replacement_scan]
fn dfn_scan_missing(path: &str) -> DuckOptionResult<String> {
    if path.ends_with(".missing") {
        return Ok(Some("dfn_scan_no_such_function".to_string()));
    }
    Ok(None)
}

// ============================================================================
// 注册控制：
//   #[duck_replacement_scan]                        自动注册
//   #[duck_replacement_scan(auto_register = false)]  只生成模块，注册交给
//                                                    #[duck_custom_register]
// 生成模块里的 replacement_scan_register 就是手动注册入口。
// ============================================================================

/// 只声明、不注册：`.unregistered` 永远不会被接管
/// ```sql
/// SELECT * FROM 'hi.unregistered';
/// ```
#[allow(dead_code)]
#[duck_replacement_scan(auto_register = false)]
fn dfn_scan_unregistered(path: &str) -> Option<String> {
    if path.ends_with(".unregistered") {
        return Some("dfn_scan_read_echo".to_string());
    }
    None
}

/// 手动注册
/// ```sql
/// SELECT * FROM 'hi.manual';
/// ```
#[duck_replacement_scan(auto_register = false)]
fn dfn_scan_manual(path: &str) -> DuckOptionResult<String> {
    if path.ends_with(".manual") {
        return Ok(Some("dfn_scan_read_echo".to_string()));
    }
    Ok(None)
}

#[duck_custom_register]
fn dfn_scan_manual_register(c: &Connection) -> DuckResult<()> {
    dfn_scan_manual::replacement_scan_register(c)
}
