//! YAML parser (`yaml` feature).
//!
//! **Formats:** `yml`, `yaml`
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{Deserialize, Yaml};
//!
//! let value = Yaml::new().parse("file", "config.yaml", b"host: 127.0.0.1\n").unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("host").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::Deserialize;
use crate::span::is_single_line;
use cfg_if::cfg_if;
use saphyr::{LoadableYamlNode, MarkedYaml, Scalar, YamlData};
use tanzim_value::{Error, LocatedValue, Location, Map, Value};

#[derive(Default, Copy, Clone)]
pub struct Yaml;

impl Yaml {
    pub fn new() -> Self {
        Self
    }
}

impl Deserialize for Yaml {
    fn name(&self) -> &str {
        "YAML"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["yml".into(), "yaml".into()]
    }

    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error> {
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
                    location: Location::at(source, resource, None, None, None),
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
                    location: Some(Location::at(
                        source,
                        resource,
                        Some(marker.line()),
                        Some(marker.col() + 1),
                        None,
                    )),
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
                location: Location::at(source, resource, None, None, None),
            });
        }
        let result = convert_node(source, resource, text, single_line, &docs[0]);
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
    source: &str,
    resource: &str,
    text: &str,
    single_line: bool,
    node: &MarkedYaml<'_>,
) -> Result<LocatedValue, Error> {
    let location = if single_line {
        Location::at(source, resource, None, None, None)
    } else {
        let marker = node.span.start;
        let length = if !node.span.is_empty() {
            Some(node.span.len())
        } else {
            None
        };
        Location::at(
            source,
            resource,
            Some(marker.line()),
            Some(marker.col() + 1),
            length,
        )
    };
    match &node.data {
        YamlData::Value(scalar) => match scalar {
            Scalar::Null => Err(Error::UnsupportedNull {
                text: text.to_string(),
                location,
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
                list.push(convert_node(source, resource, text, single_line, node)?);
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
                let value = convert_node(source, resource, text, single_line, value_node)?;
                map.insert(key, value);
            }
            Ok(LocatedValue {
                value: Value::Map(map),
                location,
            })
        }
        YamlData::Tagged(_, inner) => convert_node(source, resource, text, single_line, inner),
        YamlData::Representation(representation, _, _) => {
            if representation == "~" || representation == "null" || representation == "Null" {
                return Err(Error::UnsupportedNull {
                    text: text.to_string(),
                    location,
                });
            }
            Ok(LocatedValue {
                value: Value::String(representation.to_string()),
                location,
            })
        }
        YamlData::Alias(_) | YamlData::BadValue => Err(Error::Parse {
            text: text.to_string(),
            location: Some(location),
            message: "unsupported yaml node".to_string(),
        }),
    }
}

#[cfg(all(test, feature = "yaml"))]
mod tests {
    use super::*;

    #[test]
    fn parses_yaml_map() {
        let parsed = Yaml::new()
            .parse("file", "config.yaml", b"hello: world\n")
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
            .parse("file", "config.yaml", b"foo: bar\nbaz: qux\n")
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
            .parse("file", "config.yaml", text.as_bytes())
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
            .parse("file", "config.yaml", b"foo: [\n")
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
