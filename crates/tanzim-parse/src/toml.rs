//! TOML parser (`toml` feature).
//!
//! **Format:** `toml`
//!
//! # Behaviour
//!
//! - Parses TOML with source spans. Tables and inline tables become maps, arrays become lists, and
//!   strings/integers/floats/booleans become the matching scalar values. Prefix and inline comments
//!   are preserved on each [`LocatedValue`] via [`tanzim_value::Comment`].
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
//! use tanzim_source::SourceBuilder;
//!
//! let source = SourceBuilder::new()
//!     .with_source("file")
//!     .with_resource("config.toml")
//!     .build()
//!     .unwrap();
//! let value = Toml::new().parse(&source, b"host = \"127.0.0.1\"\n").unwrap();
//! assert_eq!(
//!     value.value().as_map().unwrap().get("host").unwrap().value().as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::span::{char_count, is_single_line, line_column};
use crate::{Parse, Source};
use cfg_if::cfg_if;
use tanzim_value::{Comment, Error, LocatedValue, Location, Map, Value};
use toml_edit::{
    Array, DocumentMut, ImDocument, InlineTable, Item, RawString, Table, Value as TomlValue,
};

/// Parser for the `toml` format: TOML into a source-located value tree.
///
/// Tables, arrays, and scalars map to the value tree with a per-node span [`Location`]; date-times
/// are rejected with [`Error::UnsupportedType`]. Stateless — construct with [`Toml::new`].
///
/// ```
/// use tanzim_parse::{Parse, toml::Toml};
/// use tanzim_source::SourceBuilder;
///
/// let source = SourceBuilder::new()
///     .with_source("file")
///     .with_resource("config.toml")
///     .build()
///     .unwrap();
/// let value = Toml::new().parse(&source, b"port = 8080\n").unwrap();
/// let port = value.value().as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value().as_int().unwrap(), 8080);
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

    fn parse(&self, src: &Source, bytes: &[u8]) -> Result<LocatedValue, Error> {
        #[cfg(any(feature = "tracing", feature = "logging"))]
        let source = src.source();
        #[cfg(any(feature = "tracing", feature = "logging"))]
        let resource = src.resource();
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
                    location: Box::new(Location::in_source(src.clone(), None, None, None)),
                });
            }
        };
        let single_line = is_single_line(bytes);
        let document = match ImDocument::parse(text.to_string()) {
            Ok(value) => value,
            Err(error) => {
                let location = match error.span() {
                    Some(span) => {
                        let (line, column) = line_column(text, span.start);
                        let length = char_count(text, span.start, span.end).max(1);
                        Some(Box::new(Location::in_source(
                            src.clone(),
                            Some(line),
                            Some(column),
                            Some(length),
                        )))
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
        let result = convert_table(src, text, single_line, document.as_table(), 0);
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
            Ok(text) => Some(ImDocument::parse(text.to_string()).is_ok()),
            Err(_) => Some(false),
        }
    }
}

/// Serialize a [`Value`] map into TOML.
///
/// Accepts a [`Value`], `&Value`, [`LocatedValue`], or `&LocatedValue`; the root must be a
/// [`Value::Map`], since a TOML document is a table. Nested maps under a key become
/// `[table]` sections; maps inside a list become inline tables. `source` is accepted for
/// signature symmetry with [`Parse::parse`] but is unused here.
///
/// ```
/// use tanzim_parse::toml::unparse;
/// use tanzim_source::SourceBuilder;
/// use tanzim_value::{Map, LocatedValue, Location, Value};
///
/// let source = SourceBuilder::new().with_source("file").build().unwrap();
/// let mut map = Map::new();
/// map.insert("port".into(), LocatedValue::new(
///     Value::Int(8080),
///     Location::at("file", "", None, None, None),
/// ));
/// assert_eq!(unparse(&source, Value::Map(map)).unwrap(), "port = 8080\n");
/// ```
pub fn unparse<V: AsRef<Value>>(
    _source: &Source,
    value: V,
) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
    let value = value.as_ref();
    let map = match value.as_map() {
        Some(map) => map,
        None => {
            return Err(format!("toml root must be a map, found {}", value.type_name()).into());
        }
    };
    let mut document = DocumentMut::new();
    build_table(document.as_table_mut(), map)?;
    Ok(document.to_string())
}

fn build_table(
    table: &mut Table,
    map: &Map,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    for (key, lv) in map.entries() {
        let item = to_toml_item(lv.value())?;
        table.insert(key, item);

        let before = lv.comment().before();
        if !before.is_empty() {
            let mut prefix = String::new();
            for line in before {
                if !prefix.is_empty() && !prefix.ends_with('\n') {
                    prefix.push('\n');
                }
                prefix.push_str(line);
                if !line.ends_with('\n') {
                    prefix.push('\n');
                }
            }
            if let Some(mut key_mut) = table.key_mut(key) {
                key_mut.leaf_decor_mut().set_prefix(prefix.as_str());
            }
        }

        if let Some(after) = lv.comment().after()
            && let Some(item) = table.get_mut(key)
        {
            match item {
                Item::Value(v) => v.decor_mut().set_suffix(format!(" {after}")),
                Item::Table(t) => t.decor_mut().set_suffix(format!(" {after}")),
                Item::ArrayOfTables(_) | Item::None => {}
            }
        }
    }
    Ok(())
}

fn to_toml_item(value: &Value) -> Result<Item, Box<dyn std::error::Error + Send + Sync + 'static>> {
    match value {
        Value::Map(map) => {
            let mut table = Table::new();
            build_table(&mut table, map)?;
            Ok(Item::Table(table))
        }
        Value::Null => Err("cannot serialize null as TOML".into()),
        other => Ok(Item::Value(to_toml_value(other)?)),
    }
}

fn to_toml_value(
    value: &Value,
) -> Result<TomlValue, Box<dyn std::error::Error + Send + Sync + 'static>> {
    match value {
        Value::Bool(value) => Ok((*value).into()),
        Value::Int(value) => Ok((*value as i64).into()),
        Value::Float(value) => {
            if !value.is_finite() {
                return Err(format!("cannot serialize non-finite float {value} as TOML").into());
            }
            Ok((*value).into())
        }
        Value::String(value) => Ok(value.clone().into()),
        Value::List(items) => {
            let mut array = Array::new();
            for item in items {
                array.push(to_toml_value(item.value())?);
            }
            Ok(TomlValue::Array(array))
        }
        Value::Map(map) => {
            let mut table = InlineTable::new();
            for (key, item) in map.entries() {
                if matches!(item.value(), Value::Null) {
                    continue;
                }
                table.insert(key, to_toml_value(item.value())?);
            }
            Ok(TomlValue::InlineTable(table))
        }
        Value::Null => Err("cannot serialize null as TOML".into()),
    }
}

/// Extract comment lines (lines starting with `#`) from raw TOML decor text.
fn raw_comments_before(raw: &RawString, text: &str) -> Vec<String> {
    let prefix_str = match raw.as_str() {
        Some(s) if !s.is_empty() => s.to_string(),
        Some(_) => return Vec::new(),
        None => match raw.span() {
            Some(span) => match text.get(span) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return Vec::new(),
            },
            None => return Vec::new(),
        },
    };
    prefix_str
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            t.starts_with('#').then(|| t.to_string())
        })
        .collect()
}

/// Extract the first comment line (starting with `#`) from raw TOML inline suffix text.
fn raw_comment_after(raw: &RawString, text: &str) -> Option<String> {
    let suffix_str = match raw.as_str() {
        Some(s) if !s.is_empty() => s.to_string(),
        Some(_) => return None,
        None => match raw.span() {
            Some(span) => match text.get(span) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => return None,
            },
            None => return None,
        },
    };
    suffix_str.lines().next().and_then(|l| {
        let t = l.trim();
        t.starts_with('#').then(|| t.to_string())
    })
}

fn convert_table(
    source: &Source,
    text: &str,
    single_line: bool,
    table: &Table,
    fallback_offset: usize,
) -> Result<LocatedValue, Error> {
    let location = location_from_span(source, text, single_line, table.span(), fallback_offset);
    let mut map = Map::new();
    for (key, item) in table {
        let item_fallback = span_start(item.span(), fallback_offset);

        let mut before: Vec<String> = Vec::new();
        if let Some(key_obj) = table.key(key)
            && let Some(raw_prefix) = key_obj.leaf_decor().prefix()
        {
            for line in raw_comments_before(raw_prefix, text) {
                before.push(line);
            }
        }

        let (mut located, suffix_raw) = match item {
            Item::Value(value) => {
                let suffix = value.decor().suffix().cloned();
                let lv = convert_toml_value(
                    source,
                    text,
                    single_line,
                    value,
                    location_from_span(source, text, single_line, value.span(), item_fallback),
                )?;
                (lv, suffix)
            }
            Item::Table(table) => {
                if let Some(raw_prefix) = table.decor().prefix() {
                    for line in raw_comments_before(raw_prefix, text) {
                        before.push(line);
                    }
                }
                let suffix = table.decor().suffix().cloned();
                let lv = convert_table(source, text, single_line, table, item_fallback)?;
                (lv, suffix)
            }
            Item::ArrayOfTables(array) => {
                let loc = location_from_span(source, text, single_line, item.span(), item_fallback);
                let mut list = Vec::new();
                for index in 0..array.len() {
                    if let Some(table) = array.get(index) {
                        list.push(convert_table(
                            source,
                            text,
                            single_line,
                            table,
                            span_start(table.span(), item_fallback),
                        )?);
                    }
                }
                (LocatedValue::new(Value::List(list), loc), None)
            }
            Item::None => {
                return Err(Error::Parse {
                    text: text.to_string(),
                    location: Some(Box::new(location_from_span(
                        source,
                        text,
                        single_line,
                        item.span(),
                        item_fallback,
                    ))),
                    message: "unexpected empty toml item".to_string(),
                });
            }
        };

        let after: Option<String> = if let Some(raw_suffix) = suffix_raw {
            raw_comment_after(&raw_suffix, text)
        } else {
            None
        };

        if !before.is_empty() || after.is_some() {
            located = located.with_comment(Comment::new().with_before(before).with_after(after));
        }

        map.insert(key.to_string(), located);
    }
    Ok(LocatedValue::new(Value::Map(map), location))
}

fn convert_toml_value(
    source: &Source,
    text: &str,
    single_line: bool,
    value: &TomlValue,
    location: Location,
) -> Result<LocatedValue, Error> {
    match value {
        TomlValue::String(value) => Ok(LocatedValue::new(
            Value::String(value.value().to_string()),
            location,
        )),
        TomlValue::Integer(value) => Ok(LocatedValue::new(
            Value::Int(*value.value() as isize),
            location,
        )),
        TomlValue::Float(value) => Ok(LocatedValue::new(Value::Float(*value.value()), location)),
        TomlValue::Boolean(value) => Ok(LocatedValue::new(Value::Bool(*value.value()), location)),
        TomlValue::Array(array) => {
            let mut list = Vec::new();
            let fallback_offset = span_start(array.span(), 0);
            for index in 0..array.len() {
                if let Some(value) = array.get(index) {
                    let item_location = location_from_span(
                        source,
                        text,
                        single_line,
                        value.span(),
                        fallback_offset,
                    );
                    list.push(convert_toml_value(
                        source,
                        text,
                        single_line,
                        value,
                        item_location,
                    )?);
                }
            }
            Ok(LocatedValue::new(Value::List(list), location))
        }
        TomlValue::InlineTable(table) => {
            let mut map = Map::new();
            let fallback_offset = span_start(table.span(), 0);
            for (key, value) in table {
                let item_location =
                    location_from_span(source, text, single_line, value.span(), fallback_offset);
                let converted =
                    convert_toml_value(source, text, single_line, value, item_location)?;
                map.insert(key.to_string(), converted);
            }
            Ok(LocatedValue::new(Value::Map(map), location))
        }
        TomlValue::Datetime(_) => Err(Error::UnsupportedType {
            text: text.to_string(),
            location: Box::new(location),
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
    source: &Source,
    text: &str,
    single_line: bool,
    span: Option<std::ops::Range<usize>>,
    fallback_offset: usize,
) -> Location {
    if single_line {
        return Location::in_source(source.clone(), None, None, None);
    }
    let mut length = 0usize;
    if let Some(range) = &span {
        length = char_count(text, range.start, range.end);
    }
    let offset = span_start(span, fallback_offset);
    let (line, column) = line_column(text, offset);
    Location::in_source(
        source.clone(),
        Some(line),
        Some(column),
        if length > 0 { Some(length) } else { None },
    )
}

#[cfg(all(test, feature = "toml"))]
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
        LocatedValue::new(value, Location::at("file", "test", None, None, None))
    }

    #[test]
    fn unparses_complex_toml_round_trip() {
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

        let text = unparse(&file_source("out.toml"), Value::Map(map)).unwrap();
        let reparsed = Toml::new()
            .parse(&file_source("out.toml"), text.as_bytes())
            .unwrap();
        let map = reparsed.value().as_map().unwrap();
        assert_eq!(
            map.get("name").unwrap().value().as_string().unwrap(),
            "tanzim"
        );
        assert_eq!(map.get("port").unwrap().value().as_int().unwrap(), 8080);
        assert_eq!(map.get("ratio").unwrap().value().as_float().unwrap(), 0.5);
        assert!(map.get("debug").unwrap().value().as_bool().unwrap());
        let tags = map.get("tags").unwrap().value().as_list().unwrap();
        assert_eq!(tags[0].value().as_string().unwrap(), "a");
        assert_eq!(tags[1].value().as_string().unwrap(), "b");
        let nested = map.get("nested").unwrap().value().as_map().unwrap();
        assert_eq!(
            nested.get("key").unwrap().value().as_string().unwrap(),
            "value"
        );
    }

    #[test]
    fn unparse_non_map_root_is_error() {
        assert!(unparse(&file_source("out.toml"), Value::Int(1)).is_err());
    }

    #[test]
    fn parses_toml_table() {
        let parsed = Toml::new()
            .parse(&file_source("config.toml"), b"hello = \"world\"\n")
            .unwrap();
        assert_eq!(
            parsed
                .value()
                .as_map()
                .unwrap()
                .get("hello")
                .unwrap()
                .value()
                .as_string()
                .unwrap(),
            "world"
        );
    }

    #[test]
    fn nested_table_key_has_line_number() {
        let parsed = Toml::new()
            .parse(
                &file_source("config.toml"),
                b"[https]\nfollow_redirects = false\ninsecure = true\nretries = 3\n",
            )
            .unwrap();
        let https = parsed.value().as_map().unwrap().get("https").unwrap();
        let nested = https.value().as_map().unwrap();
        let retries = nested.get("retries").unwrap();
        assert_eq!(retries.location().line, std::num::NonZeroU32::new(4));
        assert_eq!(retries.location().column, std::num::NonZeroU32::new(11));
    }

    #[test]
    fn parses_table_header_prefix_comment() {
        let parsed = Toml::new()
            .parse(
                &file_source("baz.toml"),
                b"# This is a comment\n[logging]\nlevel = \"debug\"\n",
            )
            .unwrap();
        let root = parsed.value().as_map().unwrap();
        let logging = root.get("logging").unwrap();
        assert_eq!(logging.comment().before(), &["# This is a comment"]);
        assert!(!root.contains_key("# This is a comment"));
        assert_eq!(
            logging
                .value()
                .as_map()
                .unwrap()
                .get("level")
                .unwrap()
                .value()
                .as_string()
                .unwrap(),
            "debug"
        );
    }

    #[test]
    fn parses_inline_suffix_comments() {
        let text = b"# This is a comment\n[logging]\n# log level\nlevel = \"debug\" # debug, info, warn, error\n# output serialize format\noutput_serialize_format = \"json\" # json, yaml\n";
        let parsed = Toml::new().parse(&file_source("baz.toml"), text).unwrap();
        let root = parsed.value().as_map().unwrap();
        let logging_lv = root.get("logging").unwrap();
        assert_eq!(logging_lv.comment().before(), &["# This is a comment"]);
        let logging = logging_lv.value().as_map().unwrap();
        let level = logging.get("level").unwrap();
        assert_eq!(level.comment().before(), &["# log level"]);
        assert_eq!(level.comment().after(), Some("# debug, info, warn, error"));
        let osf = logging.get("output_serialize_format").unwrap();
        assert_eq!(osf.comment().before(), &["# output serialize format"]);
        assert_eq!(osf.comment().after(), Some("# json, yaml"));

        let reparsed = unparse(&file_source("out.toml"), parsed.into_value()).unwrap();
        assert!(reparsed.contains("# debug, info, warn, error"));
        assert!(reparsed.contains("# json, yaml"));
    }

    #[test]
    fn parses_and_unparses_prefix_comments() {
        let parsed = Toml::new()
            .parse(
                &file_source("config.toml"),
                b"# top comment\nhello = \"world\"\n",
            )
            .unwrap();
        let map = parsed.value().as_map().unwrap();
        let hello = map.get("hello").unwrap();
        assert_eq!(hello.comment().before(), &["# top comment"]);
        assert!(!map.contains_key("# top comment"));
        assert_eq!(hello.value().as_string().unwrap(), "world");

        let text = unparse(&file_source("out.toml"), parsed.into_value()).unwrap();
        assert!(text.contains("# top comment"));
        assert!(text.contains("hello = \"world\""));
    }

    #[test]
    fn syntax_error_has_location() {
        let error = Toml::new()
            .parse(&file_source("config.toml"), b"hello = \n")
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
