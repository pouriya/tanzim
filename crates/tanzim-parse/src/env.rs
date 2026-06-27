//! Dotenv / env-file parser (`env` feature).
//!
//! **Format:** `env`
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{Deserialize, Env};
//!
//! let value = Env::new().parse("file", ".env", b"SERVER_HOST=\"127.0.0.1\"\n").unwrap();
//! assert_eq!(
//!     value.value.as_map().unwrap().get("SERVER_HOST").unwrap().value.as_string().unwrap(),
//!     "127.0.0.1"
//! );
//! ```

use crate::Deserialize;
use crate::span::{is_single_line, line_column_from_line};
use cfg_if::cfg_if;
use tanzim_value::{Error, LocatedValue, Location, Map, Value};

#[derive(Clone, Copy, Default)]
pub struct Env;

impl Env {
    pub fn new() -> Self {
        Self
    }
}

impl Deserialize for Env {
    fn name(&self) -> &str {
        "Environment-Variables"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["env".into()]
    }

    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Parsing env-format configuration", source = source, resource = resource, bytes = bytes.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Parsing env-format configuration\" source={source} resource={resource} bytes={}", bytes.len());
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
                            Location::at(source, resource, None, None, None)
                        } else {
                            Location::at(source, resource, Some(line_number), Some(column), None)
                        };
                        map.insert(
                            key.to_string(),
                            LocatedValue {
                                value: Value::String(value),
                                location,
                            },
                        );
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
                tracing::trace!(msg = "Parsed env-format configuration", source = source, resource = resource, key_count = map.len());
            } else if #[cfg(feature = "logging")] {
                log::trace!("msg=\"Parsed env-format configuration\" source={source} resource={resource} key_count={}", map.len());
            }
        }
        Ok(LocatedValue {
            value: Value::Map(map),
            location: Location::at(source, resource, None, None, None),
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

#[cfg(all(test, feature = "env"))]
mod tests {
    use super::*;

    #[test]
    fn parses_dotenv_contents() {
        let parsed = Env::new()
            .parse("file", ".env", b"FOO=bar\nBAZ=qux\n")
            .unwrap();
        let map = parsed.value.as_map().unwrap();
        assert_eq!(map.get("FOO").unwrap().value.as_string().unwrap(), "bar");
        assert_eq!(map.get("BAZ").unwrap().value.as_string().unwrap(), "qux");
    }

    #[test]
    fn parses_env_with_line_numbers() {
        let root = Env::new()
            .parse("file", ".env", b"FOO=bar\nBAZ=qux\n")
            .unwrap();
        let map = root.value.as_map().unwrap();
        let foo = map.get("FOO").unwrap();
        assert_eq!(foo.value.as_string().unwrap(), "bar");
        assert_eq!(foo.location.line, std::num::NonZeroU32::new(1));
        let baz = map.get("BAZ").unwrap();
        assert_eq!(baz.location.line, std::num::NonZeroU32::new(2));
    }
}
