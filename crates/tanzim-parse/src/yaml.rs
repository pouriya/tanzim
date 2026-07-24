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
//! - YAML `null` becomes [`Value::Null`]. Non-scalar mapping keys, aliases,
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
//! let value = Yaml::new().parse(&source, b"host: 127.0.0.1\n", &[]).unwrap();
//! assert_eq!(
//!     value.value().as_map().unwrap().get("host").unwrap().value().as_string().unwrap(),
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
/// YAML `null` becomes [`Value::Null`]. Stateless — construct with
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
/// let value = Yaml::new().parse(&source, b"port: 8080\n", &[]).unwrap();
/// let port = value.value().as_map().unwrap().get("port").unwrap();
/// assert_eq!(port.value().as_int().unwrap(), 8080);
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
                    location: Some(Box::new(Location::in_text(
                        src.clone(),
                        text,
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
            return Ok(LocatedValue::new(
                Value::Map(Map::new()),
                Location::in_source(src.clone(), None, None, None),
            ));
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
        Location::in_text(
            source.clone(),
            text,
            Some(marker.line()),
            Some(marker.col() + 1),
            length,
        )
    };
    match &node.data {
        YamlData::Value(scalar) => match scalar {
            Scalar::Null => Ok(LocatedValue::new(Value::Null, location)),
            Scalar::Boolean(value) => Ok(LocatedValue::new(Value::Bool(*value), location)),
            Scalar::Integer(value) => Ok(LocatedValue::new(Value::Int(*value as isize), location)),
            Scalar::FloatingPoint(value) => Ok(LocatedValue::new(
                Value::Float(value.into_inner()),
                location,
            )),
            Scalar::String(value) => Ok(LocatedValue::new(
                Value::String(value.to_string()),
                location,
            )),
        },
        YamlData::Sequence(sequence) => {
            let mut list = Vec::new();
            for node in sequence {
                list.push(convert_node(source, text, single_line, node)?);
            }
            Ok(LocatedValue::new(Value::List(list), location))
        }
        YamlData::Mapping(mapping) => {
            let mut map = Map::new();
            for (key_node, value_node) in mapping {
                let key = match &key_node.data {
                    YamlData::Value(Scalar::String(value)) => value.to_string(),
                    YamlData::Representation(value, _, _) => value.to_string(),
                    _ => {
                        return Err(Error::Parse {
                            location: None,
                            message: "yaml map key must be a string".to_string(),
                        });
                    }
                };
                let value = convert_node(source, text, single_line, value_node)?;
                map.insert(key, value);
            }
            Ok(LocatedValue::new(Value::Map(map), location))
        }
        YamlData::Tagged(_, inner) => convert_node(source, text, single_line, inner),
        YamlData::Representation(representation, _, _) => {
            if representation == "~" || representation == "null" || representation == "Null" {
                return Ok(LocatedValue::new(Value::Null, location));
            }
            Ok(LocatedValue::new(
                Value::String(representation.to_string()),
                location,
            ))
        }
        YamlData::Alias(_) | YamlData::BadValue => Err(Error::Parse {
            location: Some(Box::new(location)),
            message: "unsupported yaml node".to_string(),
        }),
    }
}
