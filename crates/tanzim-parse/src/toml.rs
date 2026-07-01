//! TOML parser (`toml` feature).
//!
//! **Format:** `toml`
//!
//! # Behaviour
//!
//! - Parses TOML with source spans. Tables and inline tables become maps, arrays become lists, and
//!   strings/integers/floats/booleans become the matching scalar values.
//! - Every node carries its span as a [`Location`] (line/column); for single-line input the
//!   line/column are omitted.
//! - TOML date-times have no configuration representation and are rejected with
//!   [`Error::UnsupportedType`]. Non-UTF-8 input fails with [`Error::InvalidUtf8`], and any syntax
//!   error becomes [`Error::Parse`].
//! - [`is_format_supported`](crate::Parse::is_format_supported) returns `Some(true)` when
//!   the bytes parse as TOML, else `Some(false)`.
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{Parse, toml::Toml};
//!
//! let value = Toml::new().parse("file", "config.toml", b"host = \"127.0.0.1\"\n").unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("host").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::Parse;
use crate::span::{char_count, is_single_line, line_column};
use cfg_if::cfg_if;
use tanzim_value::{Error, LocatedValue, Location, Map, Value};
use toml_edit::{DocumentMut, Item, Table, Value as TomlValue};

/// Parser for the `toml` format: TOML into a source-located value tree.
///
/// Tables, arrays, and scalars map to the value tree with a per-node span [`Location`]; date-times
/// are rejected with [`Error::UnsupportedType`]. Stateless — construct with [`Toml::new`].
///
/// ```
/// use tanzim_parse::{Parse, toml::Toml};
///
/// let value = Toml::new().parse("file", "config.toml", b"port = 8080\n").unwrap();
/// let port = value.value.as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value.as_int().unwrap(), 8080);
/// ```
#[derive(Default, Debug, Clone, Copy)]
pub struct Toml;

impl Toml {
    /// Create a TOML parser.
    pub fn new() -> Self {
        Self
    }
}

impl Parse for Toml {
    fn name(&self) -> &str {
        "TOML"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["toml".into()]
    }

    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Parsing TOML configuration", source = source, resource = resource, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Parsing TOML configuration\" source={source} resource={resource} bytes={}", bytes.len());
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
        let document = match text.parse::<DocumentMut>() {
            Ok(value) => value,
            Err(error) => {
                let location = match error.span() {
                    Some(span) => {
                        let (line, column) = line_column(text, span.start);
                        let length = char_count(text, span.start, span.end).max(1);
                        Some(Location::at(
                            source,
                            resource,
                            Some(line),
                            Some(column),
                            Some(length),
                        ))
                    }
                    None => None,
                };
                return Err(Error::Parse {
                    text: text.to_string(),
                    location,
                    message: error.message().to_string(),
                });
            }
        };
        let result = convert_table(source, resource, text, single_line, document.as_table(), 0);
        if result.is_ok() {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Parsed TOML configuration", source = source, resource = resource);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Parsed TOML configuration\" source={source} resource={resource}");
                }
            }
        }
        result
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        match std::str::from_utf8(bytes) {
            Ok(text) => Some(text.parse::<DocumentMut>().is_ok()),
            Err(_) => Some(false),
        }
    }
}

fn convert_table(
    source: &str,
    resource: &str,
    text: &str,
    single_line: bool,
    table: &Table,
    fallback_offset: usize,
) -> Result<LocatedValue, Error> {
    let location = location_from_span(
        source,
        resource,
        text,
        single_line,
        table.span(),
        fallback_offset,
    );
    let mut map = Map::new();
    for (key, item) in table {
        let fallback_offset = span_start(item.span(), 0);
        let location = location_from_span(
            source,
            resource,
            text,
            single_line,
            item.span(),
            fallback_offset,
        );
        let value = match item {
            Item::Value(value) => {
                convert_toml_value(source, resource, text, single_line, value, location)
            }
            Item::Table(table) => {
                convert_table(source, resource, text, single_line, table, fallback_offset)
            }
            Item::ArrayOfTables(array) => {
                let mut list = Vec::new();
                for index in 0..array.len() {
                    if let Some(table) = array.get(index) {
                        list.push(convert_table(
                            source,
                            resource,
                            text,
                            single_line,
                            table,
                            span_start(table.span(), fallback_offset),
                        )?);
                    }
                }
                Ok(LocatedValue {
                    value: Value::List(list),
                    location,
                })
            }
            Item::None => Err(Error::Parse {
                text: text.to_string(),
                location: Some(location),
                message: "unexpected empty toml item".to_string(),
            }),
        }?;
        map.insert(key.to_string(), value);
    }
    Ok(LocatedValue {
        value: Value::Map(map),
        location,
    })
}

fn convert_toml_value(
    source: &str,
    resource: &str,
    text: &str,
    single_line: bool,
    value: &TomlValue,
    location: Location,
) -> Result<LocatedValue, Error> {
    match value {
        TomlValue::String(value) => Ok(LocatedValue {
            value: Value::String(value.value().to_string()),
            location,
        }),
        TomlValue::Integer(value) => Ok(LocatedValue {
            value: Value::Int(*value.value() as isize),
            location,
        }),
        TomlValue::Float(value) => Ok(LocatedValue {
            value: Value::Float(*value.value()),
            location,
        }),
        TomlValue::Boolean(value) => Ok(LocatedValue {
            value: Value::Bool(*value.value()),
            location,
        }),
        TomlValue::Array(array) => {
            let mut list = Vec::new();
            let fallback_offset = span_start(array.span(), 0);
            for index in 0..array.len() {
                if let Some(value) = array.get(index) {
                    let item_location = location_from_span(
                        source,
                        resource,
                        text,
                        single_line,
                        value.span(),
                        fallback_offset,
                    );
                    list.push(convert_toml_value(
                        source,
                        resource,
                        text,
                        single_line,
                        value,
                        item_location,
                    )?);
                }
            }
            Ok(LocatedValue {
                value: Value::List(list),
                location,
            })
        }
        TomlValue::InlineTable(table) => {
            let mut map = Map::new();
            let fallback_offset = span_start(table.span(), 0);
            for (key, value) in table {
                let item_location = location_from_span(
                    source,
                    resource,
                    text,
                    single_line,
                    value.span(),
                    fallback_offset,
                );
                let converted =
                    convert_toml_value(source, resource, text, single_line, value, item_location)?;
                map.insert(key.to_string(), converted);
            }
            Ok(LocatedValue {
                value: Value::Map(map),
                location,
            })
        }
        TomlValue::Datetime(_) => Err(Error::UnsupportedType {
            text: text.to_string(),
            location,
            found: "datetime",
        }),
    }
}

fn span_start(span: Option<std::ops::Range<usize>>, fallback_offset: usize) -> usize {
    match span {
        Some(range) => range.start,
        None => fallback_offset,
    }
}

fn location_from_span(
    source: &str,
    resource: &str,
    text: &str,
    single_line: bool,
    span: Option<std::ops::Range<usize>>,
    fallback_offset: usize,
) -> Location {
    if single_line {
        return Location::at(source, resource, None, None, None);
    }
    let mut length = 0usize;
    if let Some(range) = &span {
        length = char_count(text, range.start, range.end);
    }
    let offset = span_start(span, fallback_offset);
    let (line, column) = line_column(text, offset);
    Location::at(
        source,
        resource,
        Some(line),
        Some(column),
        if length > 0 { Some(length) } else { None },
    )
}

#[cfg(all(test, feature = "toml"))]
mod tests {
    use super::*;

    #[test]
    fn parses_toml_table() {
        let parsed = Toml::new()
            .parse("file", "config.toml", b"hello = \"world\"\n")
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
    fn syntax_error_has_location() {
        let error = Toml::new()
            .parse("file", "config.toml", b"hello = \n")
            .unwrap_err();
        if let Error::Parse { location, .. } = &error {
            assert!(location.is_some());
            assert_eq!(
                location.as_ref().unwrap().line,
                std::num::NonZeroU32::new(1)
            );
        } else {
            panic!("expected parse error");
        }
        let message = format!("{error:#}");
        assert!(message.contains('^'));
    }
}
