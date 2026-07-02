//! Dotenv / env-file parser (`env` feature).
//!
//! **Format:** `env`
//!
//! # Behaviour
//!
//! - Splits the UTF-8 input into lines; blank lines and `#` comments are ignored, and an optional
//!   leading `export ` is stripped.
//! - Each remaining `KEY=VALUE` line becomes a string entry. Values may be double-quoted (with
//!   `\n`, `\r`, `\t`, `\"`, `\\` escapes), single-quoted (taken literally), or unquoted (used
//!   verbatim). The result is always a [`Value::Map`] of [`Value::String`]s.
//! - When the source carries a `separator` option, keys are split on that separator and nested
//!   into sub-maps (e.g. `BAR__BAZ=val` with `separator=__` becomes `{bar: {baz: "val"}}`).
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
//!     .parse(&source, b"SERVER_HOST=\"127.0.0.1\"\n")
//!     .unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("server_host").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::span::{is_single_line, line_column_from_line};
use crate::{Parse, Source};
use cfg_if::cfg_if;
use tanzim_value::{Error, LocatedValue, Location, Map, Value};

/// Parser for the `env` format: dotenv / env-file `KEY=VALUE` lines into a string map.
///
/// Skips blank lines and `#` comments, supports quoted values, and records each key's line number
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
///     .parse(&source, b"# comment\nPORT=8080\n")
///     .unwrap();
/// let port = value.value.as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value.as_string().unwrap(), "8080");
/// ```
#[derive(Clone, Copy, Default)]
pub struct Env;

impl Env {
    /// Create an env-format parser.
    pub fn new() -> Self {
        Self
    }
}

impl Parse for Env {
    fn name(&self) -> &str {
        "Environment-Variables"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["env".into()]
    }

    fn parse(&self, source: &Source, bytes: &[u8]) -> Result<LocatedValue, Error> {
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
                    if let Value::Map(ref mut inner) = existing.value {
                        insert_nested(inner, rest, value);
                        return;
                    }
                    let loc = value.location.clone();
                    let mut inner = Map::new();
                    insert_nested(&mut inner, rest, value);
                    existing.value = Value::Map(inner);
                    existing.location = loc;
                }
                None => {
                    let loc = value.location.clone();
                    let mut inner = Map::new();
                    insert_nested(&mut inner, rest, value);
                    map.insert(
                        head,
                        LocatedValue {
                            value: Value::Map(inner),
                            location: loc,
                        },
                    );
                }
            }
        }

        let source_name = source.source();
        let resource = source.resource();
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Parsing env-format configuration", source = source_name, resource = resource, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Parsing env-format configuration\" source={source_name} resource={resource} bytes={}", bytes.len());
            }
        }

        let separator = match source.options().get("separator") {
            None => None,
            Some(value) => value.as_string().cloned(),
        };

        let lowercase = match source.options().get("lowercase") {
            None => true,
            Some(value) => value.as_bool().unwrap_or(true),
        };

        let text = match std::str::from_utf8(bytes) {
            Ok(value) => value,
            Err(_) => {
                return Err(Error::InvalidUtf8 {
                    location: Location::at(source_name, resource, None, None, None),
                });
            }
        };
        let single_line = is_single_line(bytes);
        let mut map = Map::new();
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
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
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
                        let value = if value_part.starts_with('"')
                            && value_part.ends_with('"')
                            && value_part.len() >= 2
                        {
                            let inner = &value_part[1..value_part.len() - 1];
                            let mut out = String::new();
                            let mut index = 0usize;
                            while index < inner.len() {
                                let ch = inner[index..].chars().next().expect("valid utf-8");
                                let ch_len = ch.len_utf8();
                                if ch == '\\' {
                                    index += ch_len;
                                    if index < inner.len() {
                                        let next =
                                            inner[index..].chars().next().expect("valid utf-8");
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
                                        out.push('\\');
                                    }
                                } else {
                                    out.push(ch);
                                    index += ch_len;
                                }
                            }
                            out
                        } else if value_part.starts_with('\'')
                            && value_part.ends_with('\'')
                            && value_part.len() >= 2
                        {
                            value_part[1..value_part.len() - 1].to_string()
                        } else {
                            value_part.to_string()
                        };
                        let location = if single_line {
                            Location::at(source_name, resource, None, None, None)
                        } else {
                            Location::at(
                                source_name,
                                resource,
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
                        let located_value = LocatedValue {
                            value: Value::String(value),
                            location,
                        };
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
        Ok(LocatedValue {
            value: Value::Map(map),
            location: Location::at(source_name, resource, None, None, None),
        })
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
/// map.insert("port".into(), LocatedValue {
///     value: Value::String("8080".into()),
///     location: Location::at("env", "", None, None, None),
/// });
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
        let full_key = format!("{prefix}{key}");
        match &item.value {
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
                    // Maps and lists are handled by the arms above.
                    Value::List(_) | Value::Map(_) => {}
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
    use tanzim_source::{OptionValue, SourceBuilder};

    fn file_source(resource: &str) -> Source {
        SourceBuilder::new()
            .with_source("file")
            .with_resource(resource)
            .build()
            .unwrap()
    }

    fn loc(value: Value) -> LocatedValue {
        LocatedValue {
            value,
            location: Location::at("env", "test", None, None, None),
        }
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
        let parsed = Env::new().parse(&source, b"FOO=bar\nBAZ=qux\n").unwrap();
        let map = parsed.value.as_map().unwrap();
        assert_eq!(map.get("foo").unwrap().value.as_string().unwrap(), "bar");
        assert_eq!(map.get("baz").unwrap().value.as_string().unwrap(), "qux");
    }

    #[test]
    fn parses_env_with_line_numbers() {
        let source = file_source(".env");
        let root = Env::new().parse(&source, b"FOO=bar\nBAZ=qux\n").unwrap();
        let map = root.value.as_map().unwrap();
        let foo = map.get("foo").unwrap();
        assert_eq!(foo.value.as_string().unwrap(), "bar");
        assert_eq!(foo.location.line, std::num::NonZeroU32::new(1));
        let baz = map.get("baz").unwrap();
        assert_eq!(baz.location.line, std::num::NonZeroU32::new(2));
    }

    #[test]
    fn parses_nested_keys_with_separator() {
        let source = SourceBuilder::new()
            .with_source("env")
            .with_option("separator", OptionValue::String("__".into()))
            .build()
            .unwrap();
        let parsed = Env::new().parse(&source, b"BAR__BAZ=val\n").unwrap();
        let map = parsed.value.as_map().unwrap();
        let bar = map.get("bar").unwrap();
        let nested = bar.value.as_map().unwrap();
        assert_eq!(nested.get("baz").unwrap().value.as_string().unwrap(), "val");
    }
}
