//! HTTP loader (`http` feature).
//!
//! Fetches configuration bytes through a user-provided closure so this crate does not
//! depend on any HTTP client library.
//!
//! **Source:** `http`
//!
//! # Options
//!
//! - `headers` — map of string headers (default `{}`)
//! - `timeout` — positive integer seconds (default `15`)
//! - `insecure` — allow invalid TLS certificates (default `false`)
//!
//! # Example
//!
//! ```text
//! http(headers=(Authorization="TOKEN"),timeout=30,insecure=true):https://example.com/config.yml
//! ```

use crate::{Error, Load, Payload, Source};
use cfg_if::cfg_if;
use std::{collections::HashMap, time::Duration};

pub const NAME: &str = "HTTP";
pub const SOURCE: &str = "http";

pub type HttpFetchFn = Box<
    dyn Fn(&str, &HashMap<String, String>, Duration, bool) -> Result<Vec<Payload>, String>
        + Send
        + Sync
        + 'static,
>;

pub struct Http {
    fetch: HttpFetchFn,
}

impl Http {
    pub fn new(fetch: HttpFetchFn) -> Self {
        Self { fetch }
    }
}

impl Load for Http {
    fn name(&self) -> &str {
        NAME
    }

    fn supported_source_list(&self) -> Vec<String> {
        vec![SOURCE.to_string()]
    }

    fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
        let options = source.options().clone();
        let resource = source.resource().to_string();

        if resource.is_empty() {
            return Err(Error::InvalidResource {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                reason: "resource URL is required".into(),
            });
        }

        for key in options.keys() {
            if !matches!(key, "headers" | "timeout" | "insecure") {
                return Err(Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: key.to_string(),
                    reason: "unknown option".into(),
                });
            }
        }
        let headers = match options.get("headers") {
            None => HashMap::new(),
            Some(value) => {
                let map = value.as_map().ok_or_else(|| Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: "headers".to_string(),
                    reason: format!("expected map, found {}", value.type_name()),
                })?;
                let mut headers = HashMap::with_capacity(map.len());
                for (entry_key, entry_value) in map.iter() {
                    headers.insert(
                        entry_key.to_string(),
                        entry_value
                            .as_string()
                            .cloned()
                            .ok_or_else(|| Error::InvalidOption {
                                loader: NAME.to_string(),
                                key: "headers".to_string(),
                                reason: format!(
                                    "expected string, found {}",
                                    entry_value.type_name()
                                ),
                            })?,
                    );
                }
                headers
            }
        };
        let timeout_seconds = match options.get("timeout") {
            None => 15,
            Some(value) => {
                let integer = value.as_integer().ok_or_else(|| Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: "timeout".to_string(),
                    reason: format!("expected positive integer, found {}", value.type_name()),
                })?;
                if integer <= 0 {
                    return Err(Error::InvalidOption {
                        loader: NAME.to_string(),
                        key: "timeout".to_string(),
                        reason: "expected positive integer".into(),
                    });
                }
                integer as u64
            }
        };
        let insecure = match options.get("insecure") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| Error::InvalidOption {
                loader: NAME.to_string(),
                key: "insecure".to_string(),
                reason: format!("expected boolean, found {}", value.type_name()),
            })?,
        };
        let timeout = Duration::from_secs(timeout_seconds);

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Fetching configuration via HTTP", resource = resource, timeout_seconds = timeout_seconds, header_count = headers.len(), insecure = insecure);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Fetching configuration via HTTP\" resource={resource} timeout_seconds={timeout_seconds} header_count={} insecure={insecure}", headers.len());
            }
        }

        let fetched =
            (self.fetch)(&resource, &headers, timeout, insecure).map_err(|error| Error::Load {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                description: "fetch configuration".into(),
                source: error.into(),
            })?;

        let mut payloads = Vec::with_capacity(fetched.len());
        for payload in fetched {
            let payload = Payload {
                source: source.clone(),
                name: payload.name,
                format: payload.format,
                content: payload.content,
            }
            .normalize();
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::info!(msg = "Fetched configuration via HTTP", resource = resource, name = ?payload.name, format = ?payload.format);
                } else if #[cfg(feature = "logging")] {
                    log::info!("msg=\"Fetched configuration via HTTP\" resource={resource} name={:?} format={:?}", payload.name, payload.format);
                }
            }
            payloads.push(payload);
        }

        Ok(payloads)
    }
}

#[cfg(all(test, feature = "http"))]
mod tests {
    use super::*;
    use tanzim_source::SourceBuilder;

    fn placeholder_source() -> Source {
        SourceBuilder::new().with_source("http").build().unwrap()
    }

    #[test]
    fn load_delegates_to_fetch_closure() {
        let loader = Http::new(Box::new(|url, headers, timeout, insecure| {
            assert_eq!(url, "https://example.com/config.json");
            assert_eq!(
                headers.get("Authorization").map(String::as_str),
                Some("TOKEN")
            );
            assert_eq!(timeout, Duration::from_secs(30));
            assert!(insecure);
            Ok(vec![Payload {
                source: placeholder_source(),
                name: Some("demo".into()),
                format: Some("json".into()),
                content: br#"{"hello":"world"}"#.to_vec(),
            }])
        }));

        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com/config.json")
            .with_option("headers", HashMap::from([("Authorization", "TOKEN")]))
            .with_option("timeout", 30_i64)
            .with_option("insecure", true)
            .build()
            .unwrap();
        let loaded = loader.load(source).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, Some("demo".to_string()));
    }

    #[test]
    fn load_requires_resource() {
        let loader = Http::new(Box::new(|_, _, _, _| {
            Ok(vec![Payload {
                source: placeholder_source(),
                name: None,
                format: None,
                content: Vec::new(),
            }])
        }));
        let source = SourceBuilder::new().with_source("http").build().unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidResource { .. }));
    }
}
