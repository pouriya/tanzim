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
//! let value = Toml::new().parse(&source, b"host = \"127.0.0.1\"\n", &[]).unwrap();
//! assert_eq!(
//!     value.value().as_map().unwrap().get("host").unwrap().value().as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::span::{char_count, is_single_line, line_column};
use crate::{Parse, Source};
use cfg_if::cfg_if;
use tanzim_value::{Comment, Error, LocatedValue, Location, Map, Value};
use toml_edit::{ImDocument, Item, RawString, Table, Value as TomlValue};

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
/// let value = Toml::new().parse(&source, b"port = 8080\n", &[]).unwrap();
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

    fn parse(
        &self,
        src: &Source,
        bytes: &[u8],
        _other_source_list: &[Source],
    ) -> Result<LocatedValue, Error> {
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
                        Some(Box::new(Location::in_text(
                            src.clone(),
                            text,
                            Some(line),
                            Some(column),
                            Some(length),
                        )))
                    }
                    None => None,
                };
                return Err(Error::Parse {
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

/// Extract raw decor text from a TOML [`RawString`], using inline content or the source span.
fn raw_decor_text(raw: &RawString, text: &str) -> Option<String> {
    match raw.as_str() {
        Some(value) if !value.is_empty() => Some(value.to_string()),
        Some(_) => None,
        None => match raw.span() {
            Some(span) => match text.get(span) {
                Some(value) if !value.is_empty() => Some(value.to_string()),
                _ => None,
            },
            None => None,
        },
    }
}

/// Extract comment body from a `# …` line (no sigil in the result).
fn hash_comment_body(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }
    Some(trimmed[1..].trim_start().to_string())
}

/// Extract comment bodies from raw TOML decor text (lines starting with `#`).
fn raw_comments_before(raw: &RawString, text: &str) -> Vec<String> {
    let Some(prefix_str) = raw_decor_text(raw, text) else {
        return Vec::new();
    };
    let mut bodies = Vec::new();
    for line in prefix_str.lines() {
        if let Some(body) = hash_comment_body(line) {
            bodies.push(body);
        }
    }
    bodies
}

/// Extract the first `# …` comment body from raw TOML decor text.
fn raw_comment_after(raw: &RawString, text: &str) -> Option<String> {
    let suffix_str = raw_decor_text(raw, text)?;
    for line in suffix_str.lines() {
        if let Some(body) = hash_comment_body(line) {
            return Some(body);
        }
    }
    None
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
            let mut list: Vec<LocatedValue> = Vec::new();
            let fallback_offset = span_start(array.span(), 0);
            let len = array.len();
            for index in 0..len {
                if let Some(value) = array.get(index) {
                    let mut before: Vec<String> = Vec::new();
                    let mut previous_after: Option<String> = None;
                    if let Some(raw_prefix) = value.decor().prefix()
                        && let Some(prefix_str) = raw_decor_text(raw_prefix, text)
                    {
                        let mut prefix_comment_lines: Vec<(String, String)> = Vec::new();
                        for line in prefix_str.lines() {
                            if line.trim().is_empty() {
                                continue;
                            }
                            if let Some(body) = hash_comment_body(line) {
                                prefix_comment_lines.push((line.to_string(), body));
                            }
                        }
                        if prefix_comment_lines.len() == 1 && index > 0 {
                            previous_after = Some(prefix_comment_lines.pop().unwrap().1);
                        } else {
                            for (line_idx, (line, body)) in prefix_comment_lines.iter().enumerate()
                            {
                                if line_idx == 0
                                    && index > 0
                                    && before.is_empty()
                                    && previous_after.is_none()
                                    && line.starts_with(" #")
                                {
                                    previous_after = Some(body.clone());
                                } else {
                                    before.push(body.clone());
                                }
                            }
                        }
                    }
                    if let Some(after) = previous_after
                        && let Some(previous) = list.last_mut()
                    {
                        let mut comment_state = previous.comment().clone();
                        comment_state.set_after(Some(after));
                        *previous = previous.clone().with_comment(comment_state);
                    }

                    let mut after = if let Some(raw_suffix) = value.decor().suffix().cloned() {
                        raw_comment_after(&raw_suffix, text)
                    } else {
                        None
                    };
                    if index + 1 == len
                        && after.is_none()
                        && let Some(body) = raw_comment_after(array.trailing(), text)
                    {
                        after = Some(body);
                    }

                    let item_location = location_from_span(
                        source,
                        text,
                        single_line,
                        value.span(),
                        fallback_offset,
                    );
                    let mut item =
                        convert_toml_value(source, text, single_line, value, item_location)?;
                    if !before.is_empty() || after.is_some() {
                        let mut comment = Comment::new();
                        if !before.is_empty() {
                            comment.set_before(before);
                        }
                        if let Some(body) = after {
                            comment.set_after(Some(body));
                        }
                        item = item.with_comment(comment);
                    }
                    list.push(item);
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
    Location::in_text(
        source.clone(),
        text,
        Some(line),
        Some(column),
        if length > 0 { Some(length) } else { None },
    )
}
