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
//!
//! let value = Json::new()
//!     .parse("file", "config.json", br#"{"host":"127.0.0.1"}"#)
//!     .unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("host").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::Parse;
use crate::span::is_single_line;
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
///
/// let value = Json::new().parse("file", "config.json", br#"{"port":8080}"#).unwrap();
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

    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error> {
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

    #[test]
    fn parses_json_object() {
        let parsed = Json::new()
            .parse("file", "config.json", br#"{"hello":"world"}"#)
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
        let root = Json::new().parse("file", "a.json", br#"{"a":1}"#).unwrap();
        let map = root.value.as_map().unwrap();
        let entry = map.get("a").unwrap();
        assert_eq!(entry.location.line, None);
        assert_eq!(entry.location.column, None);
    }

    #[test]
    fn rejects_null() {
        let error = Json::new()
            .parse("file", "a.json", b"{\n  \"a\": null\n}")
            .unwrap_err();
        assert!(matches!(error, Error::UnsupportedNull { .. }));
        let message = format!("{error:#}");
        assert!(message.contains('^'));
        assert!(message.contains("null"));
    }

    #[test]
    fn syntax_error_has_location() {
        let error = Json::new()
            .parse("file", "a.json", b"{\n  \"a\":\n}\n")
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
