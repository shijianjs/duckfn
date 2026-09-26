// TSV 单元格编解码（示例共用）：把动态值渲染成「一行一条记录」的文本，再解析回来。
//
// TSV cell codec shared by the examples: renders a dynamic value into one-record-per-line text and
// parses it back.
//
// 这一层刻意**不属于 duckfn**：框架只负责把 `DataChunk` 变成 `DuckDynamicRow`（以及反过来），
// 具体格式怎么编码由格式作者决定。`DuckDynamicValue::to_text` 是给人看的展示渲染（字符串原样输出、
// 嵌套 NULL 写作 `NULL`），用它做往返会在「字符串恰好是 `NULL`」这类情况下产生歧义，所以这里自己
// 定义一套**无歧义**的编码。
//
// This layer deliberately lives **outside duckfn**: the framework only turns a `DataChunk` into
// `DuckDynamicRow`s (and back); how a format encodes them is the format author's call.
// `DuckDynamicValue::to_text` is a display rendering (strings verbatim, a nested NULL printed as
// `NULL`), and round-tripping through it is ambiguous whenever a string happens to be `NULL`, so this
// module defines its own **unambiguous** encoding instead.
//
// 编码约定：
// - 一行一条记录，列之间用制表符分隔；NULL 写成 `\N`；
// - **嵌套位置**（容器元素、结构体字段、映射的键与值）里的字符串一律用单引号包起来，因此字符串
//   `NULL` 与真正的 NULL 不会混淆；
// - 顶层单元格里的 `VARCHAR` 不加引号（整个单元格就是它的值，不存在歧义，读起来也更像普通 TSV）；
// - 转义：`\` → `\\`、`'` → `\'`、制表符 → `\t`、换行 → `\n`、回车 → `\r`；
// - `BLOB` 按 `\xHH` 编码；容器形如 `[1, 2]` / `{'k': 1}` / `{'m'=1}`；空容器写 `[]` / `{}`；
// - 日期 / 时间 / 时间戳 / UUID / INTERVAL / DECIMAL 写它们的**物理整数**（就是 `DuckDynamicValue`
//   里存的值），与 DuckDB 的 `CAST(... AS VARCHAR)` 不同 —— 示例格式追求可往返，不追求好看。
//
// The encoding: one record per line with tab-separated columns, NULL as `\N`, and **every string in a
// nested position** (container elements, struct fields, map keys and values) wrapped in single quotes
// so the string `NULL` cannot be confused with a real NULL. A top-level `VARCHAR` cell is unquoted
// (the whole cell is its value, so there is no ambiguity, and it reads like an ordinary TSV). Escapes
// are `\` → `\\`, `'` → `\'`, tab → `\t`, newline → `\n`, carriage return → `\r`. `BLOB` is written
// as `\xHH`, containers as `[1, 2]` / `{'k': 1}` / `{'m'=1}`, empty containers as `[]` / `{}`. The
// datetime / `UUID` / `INTERVAL` / `DECIMAL` wrappers are written as their **physical integer** (the
// value the enum branch stores), unlike DuckDB's `CAST(... AS VARCHAR)`: this example format favours
// round-tripping over prettiness.

use duckfn::{DuckDynamicValue, DuckResult, DuckTypeDesc, TypeId, duck_error};
use quack_rs::interval::DuckInterval;

/// 列分隔符。
///
/// The column separator.
pub const DELIMITER: char = '\t';

/// NULL 的文本标记。
///
/// The textual marker for NULL.
pub const NULL_MARKER: &str = "\\N";

/// 把一个单元格渲染成文本（顶层；`VARCHAR` 不加引号）；`None` 表示 SQL NULL。
///
/// Renders one cell as text (top level, so a `VARCHAR` is unquoted); `None` means SQL NULL.
#[must_use]
pub fn format_cell(value: Option<&DuckDynamicValue>, desc: &DuckTypeDesc) -> String {
    match value {
        Some(value) => encode(value, desc, false),
        None => NULL_MARKER.to_owned(),
    }
}

/// 解析一个顶层单元格；`Ok(None)` 表示该单元格是 SQL NULL。
///
/// Parses one top-level cell; `Ok(None)` means the cell is SQL NULL.
///
/// # Errors
///
/// 文本与 `desc` 描述的类型对不上时返回错误（括号不闭合、数字非法、字段个数不符等）。
///
/// Returns an error when the text does not match the type `desc` describes (unclosed brackets, a
/// malformed number, the wrong field count, ...).
pub fn parse_cell(text: &str, desc: &DuckTypeDesc) -> DuckResult<Option<DuckDynamicValue>> {
    if text == NULL_MARKER {
        return Ok(None);
    }
    Ok(Some(parse_value(text, desc, false)?))
}

/// 把值编码成文本；`nested` 为真时字符串带引号（见模块头部的约定）。
///
/// Encodes a value as text; with `nested` set, strings are quoted (see the module docs).
fn encode(value: &DuckDynamicValue, desc: &DuckTypeDesc, nested: bool) -> String {
    match value {
        DuckDynamicValue::Varchar(text) if nested => format!("'{}'", escape(text)),
        // BLOB 直接在编码层写 `\xHH`（而不是先走展示渲染再转义，那样会变成 `\\xHH`）。
        //
        // BLOB is written as `\xHH` right here rather than going through the display rendering and
        // then being escaped (which would produce `\\xHH`).
        DuckDynamicValue::Blob(bytes) => bytes
            .iter()
            .map(|byte| format!("\\x{byte:02X}"))
            .collect(),
        DuckDynamicValue::List(items) => {
            let element = element_desc(desc);
            let inner: Vec<String> = items
                .iter()
                .map(|item| match item {
                    Some(item) => encode(item, element, true),
                    None => "NULL".to_owned(),
                })
                .collect();
            format!("[{}]", inner.join(", "))
        }
        DuckDynamicValue::Struct(values) => {
            let fields = struct_fields(desc);
            let inner: Vec<String> = values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let field_desc = fields.get(index).map(|(_, desc)| desc);
                    let text = match (value, field_desc) {
                        (Some(value), Some(field_desc)) => encode(value, field_desc, true),
                        // 理论上到不了这里（schema 一定给得出字段）；退化成展示渲染。
                        //
                        // Unreachable in practice (the schema always has the field); falls back to
                        // the display rendering.
                        (Some(value), None) => value.to_text(desc),
                        (None, _) => "NULL".to_owned(),
                    };
                    match fields.get(index) {
                        Some((name, _)) => format!("'{name}': {text}"),
                        None => text,
                    }
                })
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        DuckDynamicValue::Map(pairs) => {
            let (key_desc, value_desc) = map_descs(desc);
            let inner: Vec<String> = pairs
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        encode(key, key_desc.unwrap_or(desc), true),
                        encode(value, value_desc.unwrap_or(desc), true)
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        // 标量（含顶层的字符串、BLOB、数值、日期时间）：用展示渲染即可，标量本身没有歧义。
        //
        // Scalars (including a top-level string, BLOB, numbers and datetime wrappers): the display
        // rendering is enough, since a scalar on its own is unambiguous.
        scalar => escape(&scalar.to_text(desc)),
    }
}

/// 递归解析一个「非 NULL」的值。
///
/// Recursively parses one non-NULL value.
fn parse_value(text: &str, desc: &DuckTypeDesc, nested: bool) -> DuckResult<DuckDynamicValue> {
    match desc {
        DuckTypeDesc::Scalar(TypeId::Varchar) => {
            let text = text.trim();
            let text = if nested { unquote(text)? } else { text };
            Ok(DuckDynamicValue::Varchar(unescape(text)))
        }
        DuckTypeDesc::Scalar(type_id) => parse_scalar(&unescape(text.trim()), *type_id),
        DuckTypeDesc::Decimal { width, scale } => Ok(DuckDynamicValue::Decimal {
            width: *width,
            scale: *scale,
            unscaled: parse_number(text.trim(), "DECIMAL")?,
        }),
        DuckTypeDesc::List(element) => {
            let inner = strip(text, '[', ']')?;
            let mut items = Vec::new();
            if !inner.trim().is_empty() {
                for part in split_top_level(inner, ',') {
                    items.push(parse_nested(part.trim(), element)?);
                }
            }
            Ok(DuckDynamicValue::List(items))
        }
        DuckTypeDesc::Struct(fields) => {
            let inner = strip(text, '{', '}')?;
            let parts: Vec<&str> = if inner.trim().is_empty() {
                Vec::new()
            } else {
                split_top_level(inner, ',')
            };
            if parts.len() != fields.len() {
                return Err(duck_error(format!(
                    "dfn_copy_tsv: STRUCT cell `{text}` has {} fields but the schema declares {}",
                    parts.len(),
                    fields.len()
                )));
            }
            let mut values = Vec::with_capacity(parts.len());
            for (part, (_, field_desc)) in parts.iter().zip(fields) {
                // 渲染时每个字段是 `'名字': 值`；按第一个顶层冒号把名字去掉。
                //
                // Each field renders as `'name': value`; drop the name at the first top-level colon.
                let part = part.trim();
                let value_text = match split_first_top_level(part, ':') {
                    Some((_, value)) => value.trim(),
                    None => part,
                };
                values.push(parse_nested(value_text, field_desc)?);
            }
            Ok(DuckDynamicValue::Struct(values))
        }
        DuckTypeDesc::Map(key_desc, value_desc) => {
            let inner = strip(text, '{', '}')?;
            let mut pairs = Vec::new();
            if !inner.trim().is_empty() {
                for part in split_top_level(inner, ',') {
                    let part = part.trim();
                    let (key_text, value_text) = split_first_top_level(part, '=').ok_or_else(|| {
                        duck_error(format!("dfn_copy_tsv: MAP entry `{part}` has no `=`"))
                    })?;
                    let key = parse_nested(key_text.trim(), key_desc)?
                        .ok_or_else(|| duck_error("dfn_copy_tsv: MAP key cannot be NULL"))?;
                    let value = parse_nested(value_text.trim(), value_desc)?
                        .ok_or_else(|| duck_error("dfn_copy_tsv: MAP value cannot be NULL"))?;
                    pairs.push((key, value));
                }
            }
            Ok(DuckDynamicValue::Map(pairs))
        }
    }
}

/// 解析一个嵌套位置的值；`NULL` 是 NULL 标记（字符串 `NULL` 会被引号包住，因此不冲突）。
///
/// Parses one value in a nested position; `NULL` is the NULL marker (a string `NULL` is quoted, so
/// there is no clash).
fn parse_nested(text: &str, desc: &DuckTypeDesc) -> DuckResult<Option<DuckDynamicValue>> {
    if text.trim() == "NULL" {
        return Ok(None);
    }
    Ok(Some(parse_value(text, desc, true)?))
}

/// 去掉字符串两侧的单引号，并校验内部没有未转义的引号。
///
/// Strips the surrounding single quotes and checks that no unescaped quote is left inside.
fn unquote(text: &str) -> DuckResult<&str> {
    text.strip_prefix('\'')
        .and_then(|inner| inner.strip_suffix('\''))
        .ok_or_else(|| {
            duck_error(format!(
                "dfn_copy_tsv: expected a quoted string but got `{text}`"
            ))
        })
}

/// 解析一个标量；没有文本往返约定的类型（`ENUM` / `ARRAY` 等）报错。
///
/// Parses one scalar; types without a textual round-trip convention (such as `ENUM` / `ARRAY`)
/// report an error.
fn parse_scalar(text: &str, type_id: TypeId) -> DuckResult<DuckDynamicValue> {
    let value = match type_id {
        TypeId::Boolean => DuckDynamicValue::Boolean(match text {
            "true" => true,
            "false" => false,
            other => {
                return Err(duck_error(format!("dfn_copy_tsv: `{other}` is not a BOOLEAN")));
            }
        }),
        TypeId::TinyInt => DuckDynamicValue::TinyInt(parse_number(text, "TINYINT")?),
        TypeId::SmallInt => DuckDynamicValue::SmallInt(parse_number(text, "SMALLINT")?),
        TypeId::Integer => DuckDynamicValue::Integer(parse_number(text, "INTEGER")?),
        TypeId::BigInt => DuckDynamicValue::BigInt(parse_number(text, "BIGINT")?),
        TypeId::HugeInt => DuckDynamicValue::HugeInt(parse_number(text, "HUGEINT")?),
        TypeId::UTinyInt => DuckDynamicValue::UTinyInt(parse_number(text, "UTINYINT")?),
        TypeId::USmallInt => DuckDynamicValue::USmallInt(parse_number(text, "USMALLINT")?),
        TypeId::UInteger => DuckDynamicValue::UInteger(parse_number(text, "UINTEGER")?),
        TypeId::UBigInt => DuckDynamicValue::UBigInt(parse_number(text, "UBIGINT")?),
        TypeId::UHugeInt => DuckDynamicValue::UHugeInt(parse_number(text, "UHUGEINT")?),
        TypeId::Float => DuckDynamicValue::Float(parse_number(text, "FLOAT")?),
        TypeId::Double => DuckDynamicValue::Double(parse_number(text, "DOUBLE")?),
        TypeId::Blob => DuckDynamicValue::Blob(parse_blob(text)?),
        TypeId::Date => DuckDynamicValue::Date(parse_number(text, "DATE")?),
        TypeId::Time => DuckDynamicValue::Time(parse_number(text, "TIME")?),
        TypeId::TimeTz => DuckDynamicValue::TimeTz(parse_number(text, "TIMETZ")?),
        TypeId::Timestamp => DuckDynamicValue::Timestamp(parse_number(text, "TIMESTAMP")?),
        TypeId::TimestampTz => DuckDynamicValue::TimestampTz(parse_number(text, "TIMESTAMPTZ")?),
        TypeId::TimestampS => DuckDynamicValue::TimestampS(parse_number(text, "TIMESTAMP_S")?),
        TypeId::TimestampMs => DuckDynamicValue::TimestampMs(parse_number(text, "TIMESTAMP_MS")?),
        TypeId::TimestampNs => DuckDynamicValue::TimestampNs(parse_number(text, "TIMESTAMP_NS")?),
        TypeId::Uuid => DuckDynamicValue::Uuid(parse_number(text, "UUID")?),
        TypeId::Interval => DuckDynamicValue::Interval(parse_interval(text)?),
        TypeId::Varchar => DuckDynamicValue::Varchar(text.to_owned()),
        other => {
            return Err(duck_error(format!(
                "dfn_copy_tsv: cannot parse `{}` from text",
                other.sql_name()
            )));
        }
    };
    Ok(value)
}

/// 解析 `months days micros` 三段整数。
///
/// Parses the three integers of `months days micros`.
fn parse_interval(text: &str) -> DuckResult<DuckInterval> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(duck_error(format!(
            "dfn_copy_tsv: INTERVAL cell `{text}` must be `months days micros`"
        )));
    }
    Ok(DuckInterval {
        months: parse_number(parts[0], "INTERVAL months")?,
        days: parse_number(parts[1], "INTERVAL days")?,
        micros: parse_number(parts[2], "INTERVAL micros")?,
    })
}

/// 解析 `\xHH` 形式的 BLOB。
///
/// Parses a BLOB written as `\xHH`.
fn parse_blob(text: &str) -> DuckResult<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4);
    let mut index = 0;
    while index < bytes.len() {
        let is_escape =
            bytes[index] == b'\\' && index + 3 < bytes.len() && bytes[index + 1] == b'x';
        if !is_escape {
            return Err(duck_error(format!(
                "dfn_copy_tsv: malformed BLOB cell `{text}` (expected \\xHH sequences)"
            )));
        }
        let high = hex_digit(bytes[index + 2])?;
        let low = hex_digit(bytes[index + 3])?;
        out.push(high * 16 + low);
        index += 4;
    }
    Ok(out)
}

/// 把一个 ASCII 十六进制字符转成数值。
///
/// Converts one ASCII hex digit into its value.
fn hex_digit(byte: u8) -> DuckResult<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        other => Err(duck_error(format!(
            "dfn_copy_tsv: `{}` is not a hex digit",
            other as char
        ))),
    }
}

/// 泛型解析一个数字，失败时带上类型名。
///
/// Parses a number generically, naming the type on failure.
fn parse_number<T: std::str::FromStr>(text: &str, what: &str) -> DuckResult<T> {
    text.trim().parse::<T>().map_err(|_| {
        duck_error(format!(
            "dfn_copy_tsv: `{}` is not a valid {what}",
            text.trim()
        ))
    })
}

/// 取 `LIST` 的元素描述（描述与值不匹配时退化成自身，保证编码不会 panic）。
///
/// Returns a `LIST`'s element description (falling back to the description itself when the value and
/// the description disagree, so encoding never panics).
fn element_desc(desc: &DuckTypeDesc) -> &DuckTypeDesc {
    match desc {
        DuckTypeDesc::List(element) => element,
        other => other,
    }
}

/// 取 `STRUCT` 的字段描述。
///
/// Returns a `STRUCT`'s field descriptions.
fn struct_fields(desc: &DuckTypeDesc) -> &[(String, DuckTypeDesc)] {
    match desc {
        DuckTypeDesc::Struct(fields) => fields,
        _ => &[],
    }
}

/// 取 `MAP` 的键 / 值描述。
///
/// Returns a `MAP`'s key and value descriptions.
fn map_descs(desc: &DuckTypeDesc) -> (Option<&DuckTypeDesc>, Option<&DuckTypeDesc>) {
    match desc {
        DuckTypeDesc::Map(key, value) => (Some(key), Some(value)),
        _ => (None, None),
    }
}

/// 把 `\`、`'`、制表符、换行、回车转义。
///
/// Escapes `\`, `'`, tab, newline and carriage return.
///
/// `'` 也要转义：嵌套位置的字符串由单引号包住，未转义的引号会提前结束字符串。
///
/// `'` is escaped too: a nested string is wrapped in single quotes, so an unescaped quote would end it
/// early.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// [`escape`] 的逆操作；不认识的转义序列原样保留（含 `\N` 与 `\xHH`，由上层判断）。
///
/// The inverse of [`escape`]; unknown escapes are kept verbatim (including `\N` and `\xHH`, which the
/// caller interprets).
#[must_use]
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 去掉最外层的括号并返回里面（允许两侧空白）。
///
/// Strips the outermost brackets and returns the inside (surrounding whitespace allowed).
fn strip(text: &str, open: char, close: char) -> DuckResult<&str> {
    let trimmed = text.trim();
    trimmed
        .strip_prefix(open)
        .and_then(|inner| inner.strip_suffix(close))
        .ok_or_else(|| {
            duck_error(format!(
                "dfn_copy_tsv: expected `{open}...{close}` but got `{trimmed}`"
            ))
        })
}

/// 按顶层分隔符切分：括号 `[]` / `{}` 之内、单引号引起来的字符串之内、以及被反斜杠转义的字符都不切。
///
/// Splits on a top-level separator: separators inside `[]` / `{}`, inside single-quoted strings, or
/// escaped by a backslash are left alone.
fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '\'' => in_quotes = !in_quotes,
            '[' | '{' if !in_quotes => depth += 1,
            ']' | '}' if !in_quotes => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 && !in_quotes => {
                parts.push(&text[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// 找第一个顶层分隔符并按它切成两半（规则同 [`split_top_level`]）。
///
/// Finds the first top-level separator and splits around it (same rules as [`split_top_level`]).
fn split_first_top_level(text: &str, separator: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut in_quotes = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '\'' => in_quotes = !in_quotes,
            '[' | '{' if !in_quotes => depth += 1,
            ']' | '}' if !in_quotes => depth = depth.saturating_sub(1),
            _ if ch == separator && depth == 0 && !in_quotes => {
                return Some((&text[..index], &text[index + ch.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}
