//! YAML parser (`yaml` feature).
//!
//! **Formats:** `yml`, `yaml`
//!
//! # Behaviour
//!
//! - Parses YAML with source markers. Mappings become maps, sequences become lists, and
//!   scalars become strings/integers/floats/booleans. An empty document yields an empty map.
//! - Every node carries its marker as a [`Location`] (line/column); for single-line input the
//!   line/column are omitted.
//! - YAML `null` is rejected with [`Error::UnsupportedNull`]. Non-scalar mapping keys, aliases,
//!   and malformed nodes become [`Error::Parse`]; non-UTF-8 input fails with
//!   [`Error::InvalidUtf8`].
//! - [`is_format_supported`](crate::Parse::is_format_supported) returns `Some(true)` when
//!   the bytes parse as YAML, else `Some(false)`.
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{Parse, yaml::Yaml};
//! use tanzim_source::SourceBuilder;
//!
//! let source = SourceBuilder::new()
//!     .with_source("file")
//!     .with_resource("config.yaml")
//!     .build()
//!     .unwrap();
//! let value = Yaml::new().parse(&source, b"host: 127.0.0.1\n").unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("host").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::span::is_single_line;
use crate::{Parse, Source};
use cfg_if::cfg_if;
use saphyr::{LoadableYamlNode, MarkedYaml, Scalar, YamlData};
use tanzim_value::{Error, LocatedValue, Location, Map, Value};

/// Parser for the `yml`/`yaml` formats: YAML into a source-located value tree.
///
/// Mappings, sequences, and scalars map to the value tree with a per-node marker [`Location`];
/// YAML `null` is rejected with [`Error::UnsupportedNull`]. Stateless — construct with
/// [`Yaml::new`].
///
/// ```
/// use tanzim_parse::{Parse, yaml::Yaml};
/// use tanzim_source::SourceBuilder;
///
/// let source = SourceBuilder::new()
///     .with_source("file")
///     .with_resource("config.yaml")
///     .build()
///     .unwrap();
/// let value = Yaml::new().parse(&source, b"port: 8080\n").unwrap();
/// let port = value.value.as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value.as_int().unwrap(), 8080);
/// ```
#[derive(Default, Copy, Clone)]
pub struct Yaml;

impl Yaml {
    /// Create a YAML parser.
    pub fn new() -> Self {
        Self
    }
}

impl Parse for Yaml {
    fn name(&self) -> &str {
        "YAML"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["yml".into(), "yaml".into()]
    }

    fn parse(&self, src: &Source, bytes: &[u8]) -> Result<LocatedValue, Error> {
        #[cfg(any(feature = "tracing", feature = "logging"))]
        let source = src.source();
        #[cfg(any(feature = "tracing", feature = "logging"))]
        let resource = src.resource();
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Parsing YAML configuration", source = source, resource = resource, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Parsing YAML configuration\" source={source} resource={resource} bytes={}", bytes.len());
            }
        }
        let text = match std::str::from_utf8(bytes) {
            Ok(value) => value,
            Err(_) => {
                return Err(Error::InvalidUtf8 {
                    location: Box::new(Location::in_source(src.clone(), None, None, None)),
                });
            }
        };
        let single_line = is_single_line(bytes);
        let docs = match MarkedYaml::load_from_str(text) {
            Ok(value) => value,
            Err(error) => {
                let marker = error.marker();
                return Err(Error::Parse {
                    text: text.to_string(),
                    location: Some(Box::new(Location::in_source(
                        src.clone(),
                        Some(marker.line()),
                        Some(marker.col() + 1),
                        None,
                    ))),
                    message: error.info().to_string(),
                });
            }
        };
        if docs.is_empty() {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Parsed YAML configuration (empty document)", source = source, resource = resource);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Parsed YAML configuration (empty document)\" source={source} resource={resource}");
                }
            }
            return Ok(LocatedValue {
                value: Value::Map(Map::new()),
                location: Location::in_source(src.clone(), None, None, None),
            });
        }
        let result = convert_node(src, text, single_line, &docs[0]);
        if result.is_ok() {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Parsed YAML configuration", source = source, resource = resource);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Parsed YAML configuration\" source={source} resource={resource}");
                }
            }
        }
        result
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        match std::str::from_utf8(bytes) {
            Ok(text) => Some(MarkedYaml::load_from_str(text).is_ok()),
            Err(_) => Some(false),
        }
    }
}

/// Serialize a [`Value`] tree into block-style YAML.
///
/// Accepts a [`Value`], `&Value`, [`LocatedValue`], or `&LocatedValue`. `source` is
/// accepted for signature symmetry with [`Parse::parse`] but is unused here.
///
/// ```
/// use tanzim_parse::yaml::unparse;
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{Map, LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("file").build().unwrap();
/// let mut map = Map::new();
/// map.insert("port".into(), LocatedValue {
///     value: Value::Int(8080),
///     location: Location::at("file", "", None, None, None),
/// });
/// assert_eq!(unparse(&source, Value::Map(map)).unwrap(), "port: 8080\n");
/// ```
pub fn unparse<V: AsRef<Value>>(
    _source: &Source,
    value: V,
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let value = value.as_ref();
    let mut out = String::new();
    match value {
        Value::Map(map) if map.entries().is_empty() => out.push_str("{}\n"),
        Value::List(items) if items.is_empty() => out.push_str("[]\n"),
        Value::Map(map) => write_yaml_map(&mut out, map, 0)?,
        Value::List(items) => write_yaml_list(&mut out, items, 0)?,
        scalar => {
            write_yaml_scalar(&mut out, scalar)?;
            out.push('\n');
        }
    }
    Ok(out)
}

fn write_yaml_map(
    out: &mut String,
    map: &Map,
    indent: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    for (key, item) in map.entries() {
        push_yaml_indent(out, indent);
        write_yaml_string(out, key);
        out.push(':');
        match &item.value {
            Value::Map(inner) if inner.entries().is_empty() => out.push_str(" {}\n"),
            Value::List(items) if items.is_empty() => out.push_str(" []\n"),
            Value::Map(inner) => {
                out.push('\n');
                write_yaml_map(out, inner, indent + 1)?;
            }
            Value::List(items) => {
                out.push('\n');
                write_yaml_list(out, items, indent + 1)?;
            }
            scalar => {
                out.push(' ');
                write_yaml_scalar(out, scalar)?;
                out.push('\n');
            }
        }
    }
    Ok(())
}

fn write_yaml_list(
    out: &mut String,
    items: &[LocatedValue],
    indent: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    for item in items {
        push_yaml_indent(out, indent);
        match &item.value {
            Value::Map(inner) if inner.entries().is_empty() => out.push_str("- {}\n"),
            Value::List(inner) if inner.is_empty() => out.push_str("- []\n"),
            Value::Map(inner) => {
                out.push_str("-\n");
                write_yaml_map(out, inner, indent + 1)?;
            }
            Value::List(inner) => {
                out.push_str("-\n");
                write_yaml_list(out, inner, indent + 1)?;
            }
            scalar => {
                out.push_str("- ");
                write_yaml_scalar(out, scalar)?;
                out.push('\n');
            }
        }
    }
    Ok(())
}

fn write_yaml_scalar(
    out: &mut String,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    match value {
        Value::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Value::Int(value) => out.push_str(&value.to_string()),
        Value::Float(value) => {
            if !value.is_finite() {
                return Err(format!("cannot serialize non-finite float {value} as YAML").into());
            }
            out.push_str(&format!("{value:?}"));
        }
        Value::String(value) => write_yaml_string(out, value),
        Value::List(_) | Value::Map(_) => {
            return Err("internal error: write_yaml_scalar called on a collection".into());
        }
    }
    Ok(())
}

fn push_yaml_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn write_yaml_string(out: &mut String, value: &str) {
    let needs_quote = value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
        )
        || value.parse::<i64>().is_ok()
        || value.parse::<f64>().is_ok()
        || value.starts_with(char::is_whitespace)
        || value.ends_with(char::is_whitespace)
        || value.starts_with(|ch: char| {
            matches!(
                ch,
                '-' | '?'
                    | ':'
                    | ','
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '&'
                    | '*'
                    | '!'
                    | '|'
                    | '>'
                    | '\''
                    | '"'
                    | '%'
                    | '@'
                    | '`'
                    | '#'
            )
        })
        || value.contains(':')
        || value.contains('#')
        || value.contains('\n')
        || value.contains('\t');
    if !needs_quote {
        out.push_str(value);
        return;
    }
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
}

fn convert_node(
    source: &Source,
    text: &str,
    single_line: bool,
    node: &MarkedYaml<'_>,
) -> Result<LocatedValue, Error> {
    let location = if single_line {
        Location::in_source(source.clone(), None, None, None)
    } else {
        let marker = node.span.start;
        let length = if !node.span.is_empty() {
            Some(node.span.len())
        } else {
            None
        };
        Location::in_source(
            source.clone(),
            Some(marker.line()),
            Some(marker.col() + 1),
            length,
        )
    };
    match &node.data {
        YamlData::Value(scalar) => match scalar {
            Scalar::Null => Err(Error::UnsupportedNull {
                text: text.to_string(),
                location: Box::new(location),
            }),
            Scalar::Boolean(value) => Ok(LocatedValue {
                value: Value::Bool(*value),
                location,
            }),
            Scalar::Integer(value) => Ok(LocatedValue {
                value: Value::Int(*value as isize),
                location,
            }),
            Scalar::FloatingPoint(value) => Ok(LocatedValue {
                value: Value::Float(value.into_inner()),
                location,
            }),
            Scalar::String(value) => Ok(LocatedValue {
                value: Value::String(value.to_string()),
                location,
            }),
        },
        YamlData::Sequence(sequence) => {
            let mut list = Vec::new();
            for node in sequence {
                list.push(convert_node(source, text, single_line, node)?);
            }
            Ok(LocatedValue {
                value: Value::List(list),
                location,
            })
        }
        YamlData::Mapping(mapping) => {
            let mut map = Map::new();
            for (key_node, value_node) in mapping {
                let key = match &key_node.data {
                    YamlData::Value(Scalar::String(value)) => value.to_string(),
                    YamlData::Representation(value, _, _) => value.to_string(),
                    _ => {
                        return Err(Error::Parse {
                            text: String::new(),
                            location: None,
                            message: "yaml map key must be a string".to_string(),
                        });
                    }
                };
                let value = convert_node(source, text, single_line, value_node)?;
                map.insert(key, value);
            }
            Ok(LocatedValue {
                value: Value::Map(map),
                location,
            })
        }
        YamlData::Tagged(_, inner) => convert_node(source, text, single_line, inner),
        YamlData::Representation(representation, _, _) => {
            if representation == "~" || representation == "null" || representation == "Null" {
                return Err(Error::UnsupportedNull {
                    text: text.to_string(),
                    location: Box::new(location),
                });
            }
            Ok(LocatedValue {
                value: Value::String(representation.to_string()),
                location,
            })
        }
        YamlData::Alias(_) | YamlData::BadValue => Err(Error::Parse {
            text: text.to_string(),
            location: Some(Box::new(location)),
            message: "unsupported yaml node".to_string(),
        }),
    }
}

#[cfg(all(test, feature = "yaml"))]
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
    fn unparses_complex_yaml() {
        let mut nested = Map::new();
        nested.insert("key".into(), loc(Value::String("value".into())));
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

        let text = unparse(&file_source("out.yaml"), Value::Map(map)).unwrap();
        assert_eq!(
            text,
            "name: tanzim\nport: 8080\nratio: 0.5\ndebug: true\ntags:\n  - a\n  - b\nnested:\n  key: value\n"
        );
    }

    #[test]
    fn parses_yaml_map() {
        let parsed = Yaml::new()
            .parse(&file_source("config.yaml"), b"hello: world\n")
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
    fn parses_yaml_map_with_lines() {
        let root = Yaml::new()
            .parse(&file_source("config.yaml"), b"foo: bar\nbaz: qux\n")
            .unwrap();
        let map = root.value.as_map().unwrap();
        let foo = map.get("foo").unwrap();
        assert_eq!(foo.value.as_string().unwrap(), "bar");
        assert_eq!(foo.location.line, std::num::NonZeroU32::new(1));
        let baz = map.get("baz").unwrap();
        assert_eq!(baz.location.line, std::num::NonZeroU32::new(2));
    }

    #[test]
    fn rejects_yaml_null_at_correct_column() {
        let text = "foo: bar\n\nbaz:\n\n  qux: ~\n";
        let error = Yaml::new()
            .parse(&file_source("config.yaml"), text.as_bytes())
            .unwrap_err();
        if let Error::UnsupportedNull { location, .. } = &error {
            assert_eq!(location.line, std::num::NonZeroU32::new(5));
            assert_eq!(location.column, std::num::NonZeroU32::new(8));
            assert_eq!(location.length, std::num::NonZeroU32::new(1));
        } else {
            panic!("expected unsupported null");
        }
        let message = format!("{error:#}");
        let mut source_line = "";
        for line in message.split('\n') {
            if line.contains("qux: ~") {
                source_line = line;
                break;
            }
        }
        let mut caret_line = "";
        for line in message.split('\n') {
            if line.contains('^') {
                caret_line = line;
                break;
            }
        }
        let mut tilde_column = 0usize;
        if let Some(after_pipe) = source_line.split('|').nth(1) {
            let mut index = 0usize;
            let mut byte_index = 0usize;
            while byte_index < after_pipe.len() {
                let ch = after_pipe[byte_index..]
                    .chars()
                    .next()
                    .expect("valid utf-8");
                if ch == '~' {
                    tilde_column = index;
                    break;
                }
                index += 1;
                byte_index += ch.len_utf8();
            }
        }
        let mut caret_column = 0usize;
        if let Some(after_pipe) = caret_line.split('|').nth(1) {
            let mut index = 0usize;
            let mut byte_index = 0usize;
            while byte_index < after_pipe.len() {
                let ch = after_pipe[byte_index..]
                    .chars()
                    .next()
                    .expect("valid utf-8");
                if ch == '^' {
                    caret_column = index;
                    break;
                }
                index += 1;
                byte_index += ch.len_utf8();
            }
        }
        assert_eq!(caret_column, tilde_column);
    }

    #[test]
    fn syntax_error_has_location() {
        let error = Yaml::new()
            .parse(&file_source("config.yaml"), b"foo: [\n")
            .unwrap_err();
        if let Error::Parse { location, .. } = &error {
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
