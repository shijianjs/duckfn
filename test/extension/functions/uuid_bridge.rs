// ============================================================================
// uuid 桥：DuckUuid <-> uuid::Uuid（duckfn 的 `uuid` feature）
//
//   DuckUuid 里只有 `value: u128`，但那是 **DuckDB 渲染出来的那 128 位**：UUID 列物理上是
//   HUGEINT，DuckDB 又给最高位做了翻转，好让有符号整数的排序与 UUID 文本排序一致；quack-rs 的
//   read_uuid / write_uuid / Value::as_uuid 已经把这层翻转撤销了。所以 `Uuid::from_u128` /
//   `Uuid::as_u128` 直接就能用，两个方向都是无损的，不需要 DuckResult。
//
//   下面三个函数分别验证：渲染文本与 DuckDB 自己的 CAST 一致（编码对）、文本能解析回来（反向对）、
//   版本号取自规范字节的第 13 个十六进制位（布局确实是 UUID 规范字节序）。
//
//   The `uuid` bridge. `DuckUuid` holds only `value: u128`, and that is **the 128 bits DuckDB
//   renders**: a UUID column is physically a HUGEINT and DuckDB flips the top bit so that signed
//   integer ordering matches UUID string ordering, a flip quack-rs' read_uuid / write_uuid /
//   Value::as_uuid already undo. `Uuid::from_u128` / `Uuid::as_u128` therefore apply directly, in
//   both directions, losslessly — no DuckResult needed. The three functions below check that the
//   rendered text matches DuckDB's own CAST (the encoding), that text parses back (the reverse) and
//   that the version nibble lands where the UUID spec puts it (the layout).
// ============================================================================

use duckfn::{DuckOptionResult, DuckUuid, duck_error, duck_scalar_function};
use uuid::Uuid;

/// UUID 转规范文本。
///
/// ```sql
/// SELECT dfn_uuid_to_text('11111111-2222-3333-4444-555555555555'::UUID);
/// -- 与 CAST(... AS VARCHAR) 逐字相同
/// ```
///
/// Renders a UUID in the canonical textual form, character-for-character what
/// `CAST(... AS VARCHAR)` gives.
#[duck_scalar_function]
fn dfn_uuid_to_text(u: DuckUuid) -> String {
    u.to_uuid().to_string()
}

/// 文本转 UUID；不是合法 UUID 时报错，而不是静默变成全零。
///
/// ```sql
/// SELECT dfn_uuid_parse('11111111-2222-3333-4444-555555555555');
/// ```
///
/// Parses text into a UUID; invalid input is an error rather than a silent all-zero value.
#[duck_scalar_function]
fn dfn_uuid_parse(text: String) -> DuckOptionResult<DuckUuid> {
    let uuid = Uuid::parse_str(&text)
        .map_err(|error| duck_error(format!("dfn_uuid_parse: '{text}' is not a UUID: {error}")))?;
    Ok(Some(DuckUuid::from_uuid(uuid)))
}

/// UUID 的版本号（uuid crate 按规范取第 13 个十六进制位）。
///
/// ```sql
/// SELECT dfn_uuid_version('11111111-2222-3333-4444-555555555555'::UUID);  -- 3
/// ```
///
/// The UUID's version, which the uuid crate reads off the 13th hexadecimal digit of the canonical
/// form — so this also pins down that `value` uses the canonical byte order.
#[duck_scalar_function]
fn dfn_uuid_version(u: DuckUuid) -> i32 {
    i32::try_from(u.to_uuid().get_version_num()).unwrap_or(0)
}
