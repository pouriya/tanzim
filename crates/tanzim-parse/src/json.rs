//! JSON parser (`json` feature).
//!
//! **Format:** `json`
//!
//! # Behaviour
//!
//! - Parses standard JSON with source spans. Objects become maps, arrays become lists, and
//!   strings/numbers/booleans become the matching scalar values; integers and floats are
//!   distinguished.
//! - Every node — root, map values, and list items — carries its span as a [`Location`]
//!   (line/column); for single-line input the line/column are omitted.
//! - JSON `null` is rejected with [`Error::UnsupportedNull`], since the config model has no null.
//!   Non-UTF-8 input fails with [`Error::InvalidUtf8`], and any syntax error becomes
//!   [`Error::Parse`] with the failing position.
//! - [`is_format_supported`](crate::Parse::is_format_supported) returns `Some(true)` when
//!   the bytes parse as JSON, else `Some(false)`.
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{Parse, json::Json};
//! use tanzim_source::SourceBuilder;
//!
//! let source = SourceBuilder::new()
//!     .with_source("file")
//!     .with_resource("config.json")
//!     .build()
//!     .unwrap();
//! let value = Json::new()
//!     .parse(&source, br#"{"host":"127.0.0.1"}"#)
//!     .unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("host").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::span::is_single_line;
use crate::{Parse, Source};
use cfg_if::cfg_if;
use spanned_json_parser::value::Value as JsonValue;
use spanned_json_parser::{Position, parse};
use tanzim_value::{Error, LocatedValue, Location, Map, Value};

/// Parser for the `json` format: standard JSON into a source-located value tree.
///
/// Objects, arrays, and scalars map to the value tree with a per-node span [`Location`]; JSON
/// `null` is rejected with [`Error::UnsupportedNull`]. Stateless — construct with [`Json::new`].
///
/// ```
/// use tanzim_parse::{Parse, json::Json};
/// use tanzim_source::SourceBuilder;
///
/// let source = SourceBuilder::new()
///     .with_source("file")
///     .with_resource("config.json")
///     .build()
///     .unwrap();
/// let value = Json::new().parse(&source, br#"{"port":8080}"#).unwrap();
/// let port = value.value.as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value.as_int().unwrap(), 8080);
/// ```
#[derive(Clone, Copy, Default)]
pub struct Json;

impl Json {
    /// Create a JSON parser.
    pub fn new() -> Self {
        Self
    }
}

impl Parse for Json {
    fn name(&self) -> &str {
        "JSON"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["json".into()]
    }

    fn parse(&self, src: &Source, bytes: &[u8]) -> Result<LocatedValue, Error> {
        let source = src.source();
        let resource = src.resource();
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Parsing JSON configuration", source = source, resource = resource, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Parsing JSON configuration\" source={source} resource={resource} bytes={}", bytes.len());
            }
        }
        let text = match std::str::from_utf8(bytes) {
            Ok(value) => value,
            Err(_) => {
                return Err(Error::InvalidUtf8 {
                    location: Location::at(source, resource, None, None, None),
                });
            }
        };
        let single_line = is_single_line(bytes);
        let parsed = match parse(text) {
            Ok(value) => value,
            Err(error) => {
                return Err(Error::Parse {
                    text: text.to_string(),
                    location: Some(location_from_position(
                        source,
                        resource,
                        single_line,
                        &error.start,
                        Some(&error.end),
                    )),
                    message: format!("{:?}", error.kind),
                });
            }
        };
        let location = location_from_position(
            source,
            resource,
            single_line,
            &parsed.start,
            Some(&parsed.end),
        );
        let result = convert_value(
            source,
            resource,
            text,
            single_line,
            parsed.value,
            &parsed.start,
            location,
        );
        if result.is_ok() {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Parsed JSON configuration", source = source, resource = resource);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Parsed JSON configuration\" source={source} resource={resource}");
                }
            }
        }
        result
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        match std::str::from_utf8(bytes) {
            Ok(text) => Some(parse(text).is_ok()),
            Err(_) => Some(false),
        }
    }
}

/// Serialize a [`Value`] tree into pretty-printed JSON (2-space indent).
///
/// Accepts a [`Value`], `&Value`, [`LocatedValue`], or `&LocatedValue`. `source` is
/// accepted for signature symmetry with [`Parse::parse`] but is unused here.
///
/// ```
/// use tanzim_parse::json::unparse;
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{Map, LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("file").build().unwrap();
/// let mut map = Map::new();
/// map.insert("port".into(), LocatedValue {
///     value: Value::Int(8080),
///     location: Location::at("file", "", None, None, None),
/// });
/// let text = unparse(&source, Value::Map(map)).unwrap();
/// assert_eq!(text, "{\n  \"port\": 8080\n}");
/// ```
pub fn unparse<V: AsRef<Value>>(
    _source: &Source,
    value: V,
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut out = String::new();
    write_json(&mut out, value.as_ref(), 0)?;
    Ok(out)
}

fn write_json(
    out: &mut String,
    value: &Value,
    indent: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    match value {
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::Int(value) => out.push_str(&value.to_string()),
        Value::Float(value) => {
            if !value.is_finite() {
                return Err(format!("cannot serialize non-finite float {value} as JSON").into());
            }
            out.push_str(&format!("{value:?}"));
        }
        Value::String(value) => write_json_string(out, value),
        Value::List(values) => {
            if values.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
            out.push_str("[\n");
            for (index, item) in values.iter().enumerate() {
                push_indent(out, indent + 1);
                write_json(out, &item.value, indent + 1)?;
                if index + 1 < values.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, indent);
            out.push(']');
        }
        Value::Map(map) => {
            let entries = map.entries();
            if entries.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            out.push_str("{\n");
            for (index, (key, item)) in entries.iter().enumerate() {
                push_indent(out, indent + 1);
                write_json_string(out, key);
                out.push_str(": ");
                write_json(out, &item.value, indent + 1)?;
                if index + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            push_indent(out, indent);
            out.push('}');
        }
    }
    Ok(())
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

fn convert_value(
    source: &str,
    resource: &str,
    text: &str,
    single_line: bool,
    value: JsonValue,
    _start: &Position,
    location: Location,
) -> Result<LocatedValue, Error> {
    match value {
        JsonValue::Null => Err(Error::UnsupportedNull {
            text: text.to_string(),
            location,
        }),
        JsonValue::Bool(value) => Ok(LocatedValue {
            value: Value::Bool(value),
            location,
        }),
        JsonValue::Number(number) => match number {
            spanned_json_parser::value::Number::PosInt(value) => Ok(LocatedValue {
                value: Value::Int(value as isize),
                location,
            }),
            spanned_json_parser::value::Number::NegInt(value) => Ok(LocatedValue {
                value: Value::Int(value as isize),
                location,
            }),
            spanned_json_parser::value::Number::Float(value) => Ok(LocatedValue {
                value: Value::Float(value),
                location,
            }),
        },
        JsonValue::String(value) => Ok(LocatedValue {
            value: Value::String(value),
            location,
        }),
        JsonValue::Array(values) => {
            let mut list = Vec::new();
            for item in &values {
                let item_location = location_from_position(
                    source,
                    resource,
                    single_line,
                    &item.start,
                    Some(&item.end),
                );
                let converted = convert_value(
                    source,
                    resource,
                    text,
                    single_line,
                    item.value.clone(),
                    &item.start,
                    item_location,
                )?;
                list.push(converted);
            }
            Ok(LocatedValue {
                value: Value::List(list),
                location,
            })
        }
        JsonValue::Object(values) => {
            let mut map = Map::new();
            for (key, item) in values {
                let item_location = location_from_position(
                    source,
                    resource,
                    single_line,
                    &item.start,
                    Some(&item.end),
                );
                let converted = convert_value(
                    source,
                    resource,
                    text,
                    single_line,
                    item.value.clone(),
                    &item.start,
                    item_location,
                )?;
                map.insert(key, converted);
            }
            Ok(LocatedValue {
                value: Value::Map(map),
                location,
            })
        }
    }
}

fn location_from_position(
    source: &str,
    resource: &str,
    single_line: bool,
    start: &Position,
    end: Option<&Position>,
) -> Location {
    if single_line {
        return Location::at(source, resource, None, None, None);
    }
    let mut length = None;
    if let Some(end) = end
        && start.line == end.line
        && end.col >= start.col
    {
        length = Some(end.col - start.col + 1);
    }
    Location::at(source, resource, Some(start.line), Some(start.col), length)
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use tanzim_source::SourceBuilder;

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
            location: Location::at("file", "test", None, None, None),
        }
    }

    #[test]
    fn unparses_complex_json() {
        let mut nested = Map::new();
        nested.insert("key".into(), loc(Value::String("va\"lue".into())));
        let mut map = Map::new();
        map.insert("name".into(), loc(Value::String("tanzim".into())));
        map.insert("port".into(), loc(Value::Int(8080)));
        map.insert("ratio".into(), loc(Value::Float(0.5)));
        map.insert("debug".into(), loc(Value::Bool(true)));
        map.insert(
            "tags".into(),
            loc(Value::List(vec![
                loc(Value::String("a".into())),
                loc(Value::String("b".into())),
            ])),
        );
        map.insert("nested".into(), loc(Value::Map(nested)));

        let text = unparse(&file_source("out.json"), Value::Map(map)).unwrap();
        assert_eq!(
            text,
            "{\n  \"name\": \"tanzim\",\n  \"port\": 8080,\n  \"ratio\": 0.5,\n  \"debug\": true,\n  \"tags\": [\n    \"a\",\n    \"b\"\n  ],\n  \"nested\": {\n    \"key\": \"va\\\"lue\"\n  }\n}"
        );
    }

    #[test]
    fn parses_json_object() {
        let parsed = Json::new()
            .parse(&file_source("config.json"), br#"{"hello":"world"}"#)
            .unwrap();
        assert_eq!(
            parsed
                .value
                .as_map()
                .unwrap()
                .get("hello")
                .unwrap()
                .value
                .as_string()
                .unwrap(),
            "world"
        );
    }

    #[test]
    fn detects_json_format() {
        let parser = Json::new();
        assert_eq!(parser.is_format_supported(br#"{"a":1}"#), Some(true));
        assert_eq!(parser.is_format_supported(b"not json"), Some(false));
    }

    #[test]
    fn single_line_json_omits_position() {
        let root = Json::new()
            .parse(&file_source("a.json"), br#"{"a":1}"#)
            .unwrap();
        let map = root.value.as_map().unwrap();
        let entry = map.get("a").unwrap();
        assert_eq!(entry.location.line, None);
        assert_eq!(entry.location.column, None);
    }

    #[test]
    fn rejects_null() {
        let error = Json::new()
            .parse(&file_source("a.json"), b"{\n  \"a\": null\n}")
            .unwrap_err();
        assert!(matches!(error, Error::UnsupportedNull { .. }));
        let message = format!("{error:#}");
        assert!(message.contains('^'));
        assert!(message.contains("null"));
    }

    #[test]
    fn syntax_error_has_location() {
        let error = Json::new()
            .parse(&file_source("a.json"), b"{\n  \"a\":\n}\n")
            .unwrap_err();
        if let Error::Parse { ref location, .. } = error {
            let location = location.as_ref().expect("syntax error location");
            assert!(location.line.is_some());
            assert!(location.column.is_some());
        } else {
            panic!("expected parse error");
        }
        let message = format!("{error:#}");
        assert!(message.contains('^'));
    }
}
