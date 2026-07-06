//! Dotenv / env-file parser (`env` feature).
//!
//! **Format:** `env`
//!
//! # Behaviour
//!
//! - Splits the UTF-8 input into lines; blank lines are ignored. Full-line `#` comments and
//!   inline `#` comments after a value are preserved on each entry via [`tanzim_value::Comment`].
//!   An optional leading `export ` is stripped.
//! - Each remaining `KEY=VALUE` line becomes a string entry. Values may be double-quoted (with
//!   `\n`, `\r`, `\t`, `\"`, `\\` escapes), single-quoted (taken literally), or unquoted (used
//!   verbatim). The result is always a [`Value::Map`] of [`Value::String`]s.
//! - When the source carries a `separator` option, keys are split on that separator and nested
//!   into sub-maps (e.g. `BAR__BAZ=val` with `separator=__` becomes `{bar: {baz: "val"}}`).
//!   For non-`env` sources (e.g. file-loaded `.env` payloads), the parser inherits `separator`
//!   and `lowercase` from a sibling `env(...)` source in `other_source_list` when present.
//! - Each key carries its line/column [`Location`]; for single-line input the line/column are
//!   omitted. The root map has no line.
//! - Non-UTF-8 input fails with [`Error::InvalidUtf8`]; there are
//!   no syntax errors otherwise. [`is_format_supported`](crate::Parse::is_format_supported)
//!   returns `Some(true)` when any non-comment line contains `=`, else `Some(false)`.
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{Parse, env::Env};
//! use tanzim_source::SourceBuilder;
//!
//! let source = SourceBuilder::new()
//!     .with_source("file")
//!     .with_resource(".env")
//!     .build()
//!     .unwrap();
//! let value = Env::new()
//!     .parse(&source, b"SERVER_HOST=\"127.0.0.1\"\n", &[])
//!     .unwrap();
//! assert_eq!(
//!     value.value().as_map().unwrap().get("server_host").unwrap().value().as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::span::{is_single_line, line_column_from_line};
use crate::{Parse, Source};
use cfg_if::cfg_if;
use tanzim_value::{Comment, Error, LocatedValue, Location, Map, Value};

/// Parser for the `env` format: dotenv / env-file `KEY=VALUE` lines into a string map.
///
/// Skips blank lines, preserves `#` comments on each entry, supports quoted values, and records each key's line number
/// as a [`Location`]. When the source carries a `separator` option, keys are nested into
/// sub-maps. Stateless — construct with [`Env::new`].
///
/// ```
/// use tanzim_parse::{Parse, env::Env};
/// use tanzim_source::SourceBuilder;
///
/// let source = SourceBuilder::new()
///     .with_source("file")
///     .with_resource(".env")
///     .build()
///     .unwrap();
/// let value = Env::new()
///     .parse(&source, b"# comment\nPORT=8080\n", &[])
///     .unwrap();
/// let port = value.value().as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value().as_string().unwrap(), "8080");
/// ```
#[derive(Clone, Copy, Default)]
pub struct Env;

impl Env {
    /// Create an env-format parser.
    pub fn new() -> Self {
        Self
    }
}

fn hash_comment_body(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    Some(trimmed[1..].trim_start().to_string())
}

fn parse_double_quoted_value(input: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut index = 1usize;
    while index < input.len() {
        let ch = input[index..].chars().next()?;
        let ch_len = ch.len_utf8();
        if ch == '"' {
            return Some((out, &input[index + ch_len..]));
        }
        if ch == '\\' {
            index += ch_len;
            if index >= input.len() {
                out.push('\\');
                break;
            }
            let next = input[index..].chars().next()?;
            let next_len = next.len_utf8();
            match next {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
            index += next_len;
        } else {
            out.push(ch);
            index += ch_len;
        }
    }
    None
}

fn parse_single_quoted_value(input: &str) -> Option<(String, &str)> {
    let mut index = 1usize;
    while index < input.len() {
        let ch = input[index..].chars().next()?;
        if ch == '\'' {
            return Some((input[1..index].to_string(), &input[index + ch.len_utf8()..]));
        }
        index += ch.len_utf8();
    }
    None
}

fn parse_env_value_and_comment(value_part: &str) -> (String, Option<String>) {
    let trimmed = value_part.trim_start();
    if trimmed.starts_with('"')
        && let Some((value, rest)) = parse_double_quoted_value(trimmed)
    {
        return (value, hash_comment_body(rest));
    } else if trimmed.starts_with('\'')
        && let Some((value, rest)) = parse_single_quoted_value(trimmed)
    {
        return (value, hash_comment_body(rest));
    }
    if let Some(space_index) = trimmed.find(" #") {
        let value = trimmed[..space_index].trim_end();
        let comment = trimmed[space_index + 1..].trim();
        if comment.starts_with('#') {
            return (value.to_string(), hash_comment_body(comment));
        }
    }
    (trimmed.to_string(), None)
}

impl Parse for Env {
    fn name(&self) -> &str {
        "Environment-Variables"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["env".into()]
    }

    fn parse(
        &self,
        source: &Source,
        bytes: &[u8],
        other_source_list: &[Source],
    ) -> Result<LocatedValue, Error> {
        fn insert_nested(map: &mut Map, parts: &[String], value: LocatedValue) {
            if parts.is_empty() {
                return;
            }
            if parts.len() == 1 {
                map.insert(parts[0].clone(), value);
                return;
            }
            let head = parts[0].clone();
            let rest = &parts[1..];
            match map.get_mut(&head) {
                Some(existing) => {
                    if let Value::Map(inner) = existing.value_mut() {
                        insert_nested(inner, rest, value);
                        return;
                    }
                    let loc = value.location().clone();
                    let mut inner = Map::new();
                    insert_nested(&mut inner, rest, value);
                    existing.set_value(Value::Map(inner));
                    existing.set_location(loc);
                }
                None => {
                    let loc = value.location().clone();
                    let mut inner = Map::new();
                    insert_nested(&mut inner, rest, value);
                    map.insert(head, LocatedValue::new(Value::Map(inner), loc));
                }
            }
        }

        #[cfg(any(feature = "tracing", feature = "logging"))]
        let source_name = source.source();
        #[cfg(any(feature = "tracing", feature = "logging"))]
        let resource = source.resource();
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Parsing env-format configuration", source = source_name, resource = resource, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Parsing env-format configuration\" source={source_name} resource={resource} bytes={}", bytes.len());
            }
        }

        let (separator, lowercase) = if source.source() == "env" {
            let separator = match source.options().get("separator") {
                None => None,
                Some(value) => value.as_string().cloned(),
            };
            let lowercase = match source.options().get("lowercase") {
                None => true,
                Some(value) => value.as_bool().unwrap_or(true),
            };
            (separator, lowercase)
        } else {
            let mut env_sources: Vec<&Source> = Vec::new();
            for other in other_source_list {
                if other.source() == "env" {
                    env_sources.push(other);
                }
            }
            if env_sources.is_empty() {
                let separator = match source.options().get("separator") {
                    None => None,
                    Some(value) => value.as_string().cloned(),
                };
                let lowercase = match source.options().get("lowercase") {
                    None => true,
                    Some(value) => value.as_bool().unwrap_or(true),
                };
                (separator, lowercase)
            } else {
                let mut first_separator: Option<Option<String>> = None;
                for env_source in &env_sources {
                    let sep = match env_source.options().get("separator") {
                        None => None,
                        Some(value) => value.as_string().cloned(),
                    };
                    match &first_separator {
                        None => first_separator = Some(sep),
                        Some(expected) => {
                            if *expected != sep {
                                return Err(Error::Parse {
                                    location: Some(Box::new(Location::in_source(
                                        source.clone(),
                                        None,
                                        None,
                                        None,
                                    ))),
                                    message: "cannot determine env separator: multiple env sources with different separator options".to_string(),
                                });
                            }
                        }
                    }
                }
                let separator = first_separator.unwrap_or(None);
                let lowercase = match env_sources[0].options().get("lowercase") {
                    None => true,
                    Some(value) => value.as_bool().unwrap_or(true),
                };
                (separator, lowercase)
            }
        };

        let text = match std::str::from_utf8(bytes) {
            Ok(value) => value,
            Err(_) => {
                return Err(Error::InvalidUtf8 {
                    location: Box::new(Location::in_source(source.clone(), None, None, None)),
                });
            }
        };
        let single_line = is_single_line(bytes);
        let mut map = Map::new();
        let mut pending_before: Vec<String> = Vec::new();
        let mut line_number = 0usize;
        let mut offset = 0usize;
        while offset < text.len() {
            let rest = &text[offset..];
            let line_end = match rest.find('\n') {
                Some(index) => index,
                None => rest.len(),
            };
            let line = &rest[..line_end];
            line_number += 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                offset += line_end;
                if offset < text.len() {
                    offset += 1;
                }
                continue;
            }
            if let Some(body) = hash_comment_body(trimmed) {
                pending_before.push(body);
            } else {
                let mut line_body = trimmed;
                if line_body.starts_with("export ") {
                    line_body = line_body["export ".len()..].trim_start();
                }
                if let Some(equal_index) = line_body.find('=') {
                    let key = line_body[..equal_index].trim();
                    let value_part = line_body[equal_index + 1..].trim();
                    if !key.is_empty() {
                        let key_start = line.find(key).unwrap_or(0);
                        let column = line_column_from_line(line, 1, key_start);
                        let (value, after_comment) = parse_env_value_and_comment(value_part);
                        let location = if single_line {
                            Location::in_source(source.clone(), None, None, None)
                        } else {
                            Location::in_text(
                                source.clone(),
                                text,
                                Some(line_number),
                                Some(column),
                                None,
                            )
                        };
                        let final_key = if lowercase {
                            key.to_lowercase()
                        } else {
                            key.to_string()
                        };
                        let mut located_value = LocatedValue::new(Value::String(value), location);
                        let mut comment = Comment::new();
                        if !pending_before.is_empty() {
                            comment.set_before(pending_before.clone());
                            pending_before.clear();
                        }
                        if let Some(after) = after_comment {
                            comment.set_after(Some(after));
                        }
                        if !comment.before().is_empty() || comment.after().is_some() {
                            located_value = located_value.with_comment(comment);
                        }
                        match &separator {
                            None => {
                                map.insert(final_key, located_value);
                            }
                            Some(sep) => {
                                let mut part_list: Vec<String> = Vec::new();
                                let mut remaining = final_key.as_str();
                                loop {
                                    if let Some(index) = remaining.find(sep.as_str()) {
                                        part_list.push(remaining[..index].to_string());
                                        remaining = &remaining[index + sep.len()..];
                                    } else {
                                        part_list.push(remaining.to_string());
                                        break;
                                    }
                                }
                                if part_list.len() == 1 {
                                    map.insert(part_list[0].clone(), located_value);
                                } else {
                                    insert_nested(&mut map, &part_list, located_value);
                                }
                            }
                        }
                    }
                }
            }
            offset += line_end;
            if offset < text.len() {
                offset += 1;
            }
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::trace!(msg = "Parsed env-format configuration", source = source_name, resource = resource, key_count = map.len());
            } else if #[cfg(feature = "logging")] {
                log::trace!("msg=\"Parsed env-format configuration\" source={source_name} resource={resource} key_count={}", map.len());
            }
        }
        Ok(LocatedValue::new(
            Value::Map(map),
            Location::in_source(source.clone(), None, None, None),
        ))
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        let text = std::str::from_utf8(bytes).ok()?;
        for line in text.split('\n') {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') && line.contains('=') {
                return Some(true);
            }
        }
        Some(false)
    }
}

/// Serialize a [`Value`] map into dotenv / env-file `KEY=VALUE` lines.
///
/// Accepts a [`Value`], `&Value`, [`LocatedValue`], or `&LocatedValue`; the root must be
/// a [`Value::Map`]. Nested maps are flattened using the `separator` option carried by
/// `source` (the same option [`Env::parse`] reads); a nested map with no separator
/// configured is an error, as are lists (env has no list representation).
///
/// ```
/// use tanzim_parse::env::unparse;
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{Map, LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("env").build().unwrap();
/// let mut map = Map::new();
/// map.insert("port".into(), LocatedValue::new(
///     Value::String("8080".into()),
///     Location::at("env", "", None, None, None),
/// ));
/// assert_eq!(unparse(&source, Value::Map(map)).unwrap(), "port=8080\n");
/// ```
pub fn unparse<V: AsRef<Value>>(
    source: &Source,
    value: V,
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let value = value.as_ref();
    let map = match value.as_map() {
        Some(map) => map,
        None => {
            return Err(format!("env root must be a map, found {}", value.type_name()).into());
        }
    };
    let separator = source
        .options()
        .get("separator")
        .and_then(|value| value.as_string().cloned());
    let mut out = String::new();
    write_env(&mut out, map, "", separator.as_deref())?;
    Ok(out)
}

fn write_env(
    out: &mut String,
    map: &Map,
    prefix: &str,
    separator: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    for (key, item) in map.entries() {
        if matches!(item.value(), Value::Null) {
            continue;
        }
        let full_key = format!("{prefix}{key}");
        match item.value() {
            Value::Map(inner) => {
                let separator = match separator {
                    Some(separator) => separator,
                    None => {
                        return Err(format!(
                            "cannot serialize nested map at key {full_key:?} to env without a separator option"
                        )
                        .into());
                    }
                };
                write_env(
                    out,
                    inner,
                    &format!("{full_key}{separator}"),
                    Some(separator),
                )?;
            }
            Value::List(_) => {
                return Err(format!(
                    "cannot serialize list at key {full_key:?} to env: env has no list representation"
                )
                .into());
            }
            scalar => {
                for before in item.comment().before() {
                    out.push_str("# ");
                    out.push_str(before);
                    if !before.ends_with('\n') {
                        out.push('\n');
                    }
                }
                out.push_str(&full_key);
                out.push('=');
                match scalar {
                    Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
                    Value::Int(value) => out.push_str(&value.to_string()),
                    Value::Float(value) => out.push_str(&format!("{value:?}")),
                    Value::String(value) => {
                        let needs_quote = value.is_empty()
                            || value.contains(|ch: char| {
                                ch.is_whitespace() || matches!(ch, '"' | '\'' | '#' | '=')
                            });
                        if needs_quote {
                            out.push('"');
                            for ch in value.chars() {
                                match ch {
                                    '"' => out.push_str("\\\""),
                                    '\\' => out.push_str("\\\\"),
                                    '\n' => out.push_str("\\n"),
                                    '\r' => out.push_str("\\r"),
                                    '\t' => out.push_str("\\t"),
                                    other => out.push(other),
                                }
                            }
                            out.push('"');
                        } else {
                            out.push_str(value);
                        }
                    }
                    Value::Null => {}
                    // Maps and lists are handled by the arms above.
                    Value::List(_) | Value::Map(_) => {}
                }
                if let Some(after) = item.comment().after() {
                    out.push_str(" # ");
                    out.push_str(after);
                }
                out.push('\n');
            }
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "env"))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tanzim_source::{OptionValue, SourceBuilder};

    fn file_source(resource: &str) -> Source {
        SourceBuilder::new()
            .with_source("file")
            .with_resource(resource)
            .build()
            .unwrap()
    }

    fn loc(value: Value) -> LocatedValue {
        LocatedValue::new(value, Location::at("env", "test", None, None, None))
    }

    #[test]
    fn unparses_complex_env() {
        let source = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String("__".into()))
            .build()
            .unwrap();
        let mut database = Map::new();
        database.insert("host".into(), loc(Value::String("localhost".into())));
        database.insert("port".into(), loc(Value::Int(5432)));
        let mut map = Map::new();
        map.insert("database".into(), loc(Value::Map(database)));
        map.insert("debug".into(), loc(Value::Bool(true)));
        map.insert("note".into(), loc(Value::String("has space".into())));

        let text = unparse(&source, Value::Map(map)).unwrap();
        assert_eq!(
            text,
            "database__host=localhost\ndatabase__port=5432\ndebug=true\nnote=\"has space\"\n"
        );
    }

    #[test]
    fn unparse_list_is_error() {
        let source = file_source(".env");
        let mut map = Map::new();
        map.insert("items".into(), loc(Value::List(vec![loc(Value::Int(1))])));
        assert!(unparse(&source, Value::Map(map)).is_err());
    }

    #[test]
    fn parses_dotenv_contents() {
        let source = file_source(".env");
        let parsed = Env::new()
            .parse(&source, b"FOO=bar\nBAZ=qux\n", &[])
            .unwrap();
        let map = parsed.value().as_map().unwrap();
        assert_eq!(map.get("foo").unwrap().value().as_string().unwrap(), "bar");
        assert_eq!(map.get("baz").unwrap().value().as_string().unwrap(), "qux");
    }

    #[test]
    fn parses_env_with_line_numbers() {
        let source = file_source(".env");
        let root = Env::new()
            .parse(&source, b"FOO=bar\nBAZ=qux\n", &[])
            .unwrap();
        let map = root.value().as_map().unwrap();
        let foo = map.get("foo").unwrap();
        assert_eq!(foo.value().as_string().unwrap(), "bar");
        assert_eq!(foo.location().line, std::num::NonZeroU32::new(1));
        let baz = map.get("baz").unwrap();
        assert_eq!(baz.location().line, std::num::NonZeroU32::new(2));
    }

    #[test]
    fn parses_nested_keys_with_separator() {
        let source = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String("__".into()))
            .build()
            .unwrap();
        let parsed = Env::new().parse(&source, b"BAR__BAZ=val\n", &[]).unwrap();
        let map = parsed.value().as_map().unwrap();
        let bar = map.get("bar").unwrap();
        let nested = bar.value().as_map().unwrap();
        assert_eq!(
            nested.get("baz").unwrap().value().as_string().unwrap(),
            "val"
        );
    }

    #[test]
    fn parses_prefix_and_suffix_comments() {
        let text = b"# top comment\n# second line\nPORT=8080 # listen port\n";
        let parsed = Env::new().parse(&file_source(".env"), text, &[]).unwrap();
        let port = parsed.value().as_map().unwrap().get("port").unwrap();
        assert_eq!(port.comment().before(), &["top comment", "second line"]);
        assert_eq!(port.comment().after(), Some("listen port"));
        assert_eq!(port.value().as_string().unwrap(), "8080");
    }

    #[test]
    fn parses_quoted_value_with_suffix_comment() {
        let source = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String("__".into()))
            .build()
            .unwrap();
        let parsed = Env::new()
            .parse(&source, b"SERVER__PORT=\"8080\" # listen port\n", &[])
            .unwrap();
        let server = parsed.value().as_map().unwrap().get("server").unwrap();
        let port = server.value().as_map().unwrap().get("port").unwrap();
        assert_eq!(port.value().as_string().unwrap(), "8080");
        assert_eq!(port.comment().after(), Some("listen port"));
    }

    #[test]
    fn unparses_prefix_and_suffix_comments() {
        let source = file_source(".env");
        let mut map = Map::new();
        map.insert(
            "port".into(),
            loc(Value::String("8080".into())).with_comment(
                Comment::new()
                    .with_before(["top comment"])
                    .with_after(Some("listen port")),
            ),
        );
        let text = unparse(&source, Value::Map(map)).unwrap();
        assert_eq!(text, "# top comment\nport=8080 # listen port\n");
    }

    #[test]
    fn parses_and_unparses_foo_env_comments() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/full/etc/foo.env");
        let text = std::fs::read_to_string(&path).unwrap();
        let source = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String(".".into()))
            .build()
            .unwrap();
        let parsed = Env::new().parse(&source, text.as_bytes(), &[]).unwrap();
        let port = parsed
            .value()
            .as_map()
            .unwrap()
            .get("server")
            .unwrap()
            .value()
            .as_map()
            .unwrap()
            .get("port")
            .unwrap();
        assert_eq!(port.value().as_string().unwrap(), "8080");
        assert_eq!(port.comment().after(), Some("listen port"));

        let reparsed = unparse(&source, parsed.into_value()).unwrap();
        assert_eq!(reparsed, "server.port=8080 # listen port\n");

        let reparsed_again = Env::new().parse(&source, reparsed.as_bytes(), &[]).unwrap();
        let port_again = reparsed_again
            .value()
            .as_map()
            .unwrap()
            .get("server")
            .unwrap()
            .value()
            .as_map()
            .unwrap()
            .get("port")
            .unwrap();
        assert_eq!(port_again.value().as_string().unwrap(), "8080");
        assert_eq!(port_again.comment().after(), Some("listen port"));
    }

    #[test]
    fn parses_file_env_inheriting_separator() {
        let env_source = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String(".".into()))
            .build()
            .unwrap();
        let file_source = file_source("foo.env");
        let other_sources = [env_source];
        let parsed = Env::new()
            .parse(
                &file_source,
                b"SERVER.PORT=8080\n",
                other_sources.as_slice(),
            )
            .unwrap();
        let server = parsed.value().as_map().unwrap().get("server").unwrap();
        let port = server.value().as_map().unwrap().get("port").unwrap();
        assert_eq!(port.value().as_string().unwrap(), "8080");
    }

    #[test]
    fn rejects_conflicting_env_separators() {
        let env_dot = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String(".".into()))
            .build()
            .unwrap();
        let env_underscore = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String("__".into()))
            .build()
            .unwrap();
        let other_sources = [env_dot, env_underscore];
        let error = Env::new()
            .parse(
                &file_source("foo.env"),
                b"SERVER.PORT=8080\n",
                other_sources.as_slice(),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Parse { .. }));
        assert!(error.to_string().contains("separator"));
    }
}
