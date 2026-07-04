//! HTTP loader (`http` feature).
//!
//! Fetches configuration bytes through a user-provided closure so this crate does not depend on
//! any HTTP client library. You supply the actual transport (via [`Http::new`]); the loader
//! validates options, invokes your closure, and normalizes the returned entries.
//!
//! **Source:** `http` (the resource is the URL and is required; an empty resource is rejected
//! with [`Error::InvalidResource`])
//!
//! # Behaviour
//!
//! - The `headers`, `timeout`, and `insecure` options are validated here, then passed to your
//!   fetch closure — enforcing them (timeouts, TLS policy) is the closure's responsibility.
//! - Whatever [`Payload`]s the closure returns are post-processed: `maybe_name`
//!   and `maybe_format` are trimmed, emptied to `None`, and lower-cased when `lowercase = true`
//!   (the default).
//! - If the closure returns an error it is wrapped as
//!   [`Error::Load`] with description `"fetch configuration"`.
//!
//! # Options
//!
//! - `headers` — map of string headers (default `{}`)
//! - `timeout` — positive integer seconds (default `15`)
//! - `insecure` — allow invalid TLS certificates (default `false`)
//! - `lowercase` — boolean (default `true`; whether to lowercase entry names and formats)
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

/// The transport closure driving an [`Http`] loader — you implement the actual request here.
///
/// Called once per source with, in order:
///
/// - `&str` — the resolved URL (the source resource).
/// - `&HashMap<String, String>` — the validated `headers` option.
/// - [`Duration`] — the `timeout` option; enforcing it is up to this closure.
/// - `bool` — the `insecure` option (allow invalid TLS); honoring it is up to this closure.
///
/// Return one [`Payload`] per configuration entry fetched. On failure return an `Err(String)`;
/// the loader wraps it as [`Error::Load`] with description
/// `"fetch configuration"`. Names and formats on the returned payloads are normalized afterwards
/// (trimmed, lower-cased per the `lowercase` option), so this closure may set them raw.
///
/// Must be `Send + Sync + 'static` so the loader can be shared across threads.
pub type HttpFetchFn = Box<
    dyn Fn(&str, &HashMap<String, String>, Duration, bool) -> Result<Vec<Payload>, String>
        + Send
        + Sync
        + 'static,
>;

/// Loader for the `http` source: fetches configuration bytes through a user-supplied closure.
///
/// This crate ships no HTTP client — you provide the transport as an [`HttpFetchFn`] when calling
/// [`Http::new`]. The loader validates options, calls your closure with the resolved URL,
/// headers, timeout, and TLS policy, then normalizes the returned entries (see the
/// [module docs](self)).
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
/// use std::time::Duration;
/// use tanzim_load::{http::Http, Load, Payload};
/// use tanzim_source::SourceBuilder;
///
/// // A real fetch would call an HTTP client here; this canned closure needs no network.
/// let http = Http::new(Box::new(
///     |url: &str, _headers: &HashMap<String, String>, _timeout: Duration, _insecure: bool| {
///         Ok(vec![Payload {
///             source: SourceBuilder::new()
///                 .with_source("http")
///                 .with_resource(url)
///                 .build()
///                 .unwrap(),
///             maybe_name: Some("app".into()),
///             maybe_format: Some("json".into()),
///             content: br#"{"debug":true}"#.to_vec(),
///         }])
///     },
/// ));
///
/// let source = SourceBuilder::new()
///     .with_source("http")
///     .with_resource("https://example.com/config.json")
///     .build()
///     .unwrap();
///
/// let payloads = http.load(source).unwrap();
/// assert_eq!(payloads[0].maybe_name.as_deref(), Some("app"));
/// ```
pub struct Http {
    fetch: HttpFetchFn,
}

impl Http {
    /// Create an HTTP loader driven by `fetch`, the closure that performs the actual request.
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
        let lowercase = match options.get("lowercase") {
            None => true,
            Some(value) => value.as_bool().ok_or_else(|| Error::InvalidOption {
                loader: NAME.to_string(),
                key: "lowercase".to_string(),
                reason: format!("expected boolean, found {}", value.type_name()),
            })?,
        };
        let timeout = Duration::from_secs(timeout_seconds);

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Fetching configuration via HTTP", resource = resource, timeout_seconds = timeout_seconds, header_count = headers.len(), insecure = insecure, lowercase = lowercase);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Fetching configuration via HTTP\" resource={resource} timeout_seconds={timeout_seconds} header_count={} insecure={insecure} lowercase={lowercase}", headers.len());
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
            let name = match payload.maybe_name {
                Some(name) => {
                    let trimmed = name.trim();
                    if trimmed.is_empty() {
                        None
                    } else if lowercase {
                        let lower = trimmed.to_lowercase();
                        if lower != trimmed {
                            cfg_if! {
                                if #[cfg(feature = "tracing")] {
                                    tracing::debug!(msg = "Lowercased HTTP configuration entry name", from = trimmed, to = lower.as_str(), resource = resource);
                                } else if #[cfg(feature = "logging")] {
                                    log::debug!("msg=\"Lowercased HTTP configuration entry name\" from={trimmed} to={lower} resource={resource}");
                                }
                            }
                        }
                        Some(lower)
                    } else {
                        Some(trimmed.to_string())
                    }
                }
                None => None,
            };
            let format = match payload.maybe_format {
                Some(format) => {
                    let trimmed = format.trim();
                    if trimmed.is_empty() {
                        None
                    } else if lowercase {
                        let lower = trimmed.to_lowercase();
                        if lower != trimmed {
                            cfg_if! {
                                if #[cfg(feature = "tracing")] {
                                    tracing::debug!(msg = "Lowercased HTTP configuration format", from = trimmed, to = lower.as_str(), resource = resource);
                                } else if #[cfg(feature = "logging")] {
                                    log::debug!("msg=\"Lowercased HTTP configuration format\" from={trimmed} to={lower} resource={resource}");
                                }
                            }
                        }
                        Some(lower)
                    } else {
                        Some(trimmed.to_string())
                    }
                }
                None => None,
            };
            let payload = Payload {
                source: source.clone(),
                maybe_name: name,
                maybe_format: format,
                content: payload.content,
            };
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
                maybe_name: Some("demo".into()),
                maybe_format: Some("json".into()),
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
        assert_eq!(loaded[0].maybe_name, Some("demo".to_string()));
    }

    #[test]
    fn load_requires_resource() {
        let loader = Http::new(Box::new(|_, _, _, _| {
            Ok(vec![Payload {
                source: placeholder_source(),
                maybe_name: None,
                maybe_format: None,
                content: Vec::new(),
            }])
        }));
        let source = SourceBuilder::new().with_source("http").build().unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidResource { .. }));
    }

    #[test]
    fn name_and_supported_source_list() {
        let loader = Http::new(Box::new(|_, _, _, _| Ok(Vec::new())));
        assert_eq!(loader.name(), NAME);
        assert_eq!(loader.supported_source_list(), vec![SOURCE.to_string()]);
    }

    #[test]
    fn load_ignores_unknown_option() {
        let loader = Http::new(Box::new(|_, _, _, _| Ok(Vec::new())));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .with_option("bogus", true)
            .build()
            .unwrap();
        let loaded = loader.load(source).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_rejects_bad_headers_type() {
        let loader = Http::new(Box::new(|_, _, _, _| Ok(Vec::new())));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .with_option("headers", "not-a-map")
            .build()
            .unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { key, .. } if key == "headers"));
    }

    #[test]
    fn load_rejects_non_string_header_value() {
        let loader = Http::new(Box::new(|_, _, _, _| Ok(Vec::new())));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .with_option("headers", HashMap::from([("Authorization", 1_i64)]))
            .build()
            .unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { key, .. } if key == "headers"));
    }

    #[test]
    fn load_uses_default_timeout() {
        let loader = Http::new(Box::new(|_, _, timeout, _| {
            assert_eq!(timeout, Duration::from_secs(15));
            Ok(Vec::new())
        }));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .build()
            .unwrap();
        loader.load(source).unwrap();
    }

    #[test]
    fn load_rejects_non_positive_timeout() {
        let loader = Http::new(Box::new(|_, _, _, _| Ok(Vec::new())));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .with_option("timeout", 0_i64)
            .build()
            .unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { key, .. } if key == "timeout"));
    }

    #[test]
    fn load_rejects_bad_insecure_type() {
        let loader = Http::new(Box::new(|_, _, _, _| Ok(Vec::new())));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .with_option("insecure", "yes")
            .build()
            .unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidOption { key, .. } if key == "insecure"));
    }

    #[test]
    fn load_wraps_fetch_error() {
        let loader = Http::new(Box::new(|_, _, _, _| Err("network down".into())));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .build()
            .unwrap();
        let error = loader.load(source).unwrap_err();
        assert!(
            matches!(error, Error::Load { description, .. } if description == "fetch configuration")
        );
    }

    #[test]
    fn load_normalizes_trimmed_empty_name_and_format() {
        let loader = Http::new(Box::new(|_, _, _, _| {
            Ok(vec![Payload {
                source: placeholder_source(),
                maybe_name: Some("   ".into()),
                maybe_format: Some("\t".into()),
                content: Vec::new(),
            }])
        }));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .build()
            .unwrap();
        let loaded = loader.load(source).unwrap();
        assert_eq!(loaded[0].maybe_name, None);
        assert_eq!(loaded[0].maybe_format, None);
    }

    #[test]
    fn load_lowercases_name_and_format_by_default() {
        let loader = Http::new(Box::new(|_, _, _, _| {
            Ok(vec![Payload {
                source: placeholder_source(),
                maybe_name: Some(" Demo ".into()),
                maybe_format: Some(" JSON ".into()),
                content: Vec::new(),
            }])
        }));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .build()
            .unwrap();
        let loaded = loader.load(source).unwrap();
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("demo"));
        assert_eq!(loaded[0].maybe_format.as_deref(), Some("json"));
    }

    #[test]
    fn load_preserves_case_when_lowercase_disabled() {
        let loader = Http::new(Box::new(|_, _, _, _| {
            Ok(vec![Payload {
                source: placeholder_source(),
                maybe_name: Some("Demo".into()),
                maybe_format: Some("JSON".into()),
                content: Vec::new(),
            }])
        }));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com")
            .with_option("lowercase", false)
            .build()
            .unwrap();
        let loaded = loader.load(source).unwrap();
        assert_eq!(loaded[0].maybe_name.as_deref(), Some("Demo"));
        assert_eq!(loaded[0].maybe_format.as_deref(), Some("JSON"));
    }

    #[test]
    fn load_clones_source_onto_payloads() {
        let loader = Http::new(Box::new(|_, _, _, _| {
            Ok(vec![Payload {
                source: placeholder_source(),
                maybe_name: Some("app".into()),
                maybe_format: Some("json".into()),
                content: b"{}".to_vec(),
            }])
        }));
        let source = SourceBuilder::new()
            .with_source("http")
            .with_resource("https://example.com/x")
            .build()
            .unwrap();
        let loaded = loader.load(source.clone()).unwrap();
        assert_eq!(loaded[0].source.resource(), source.resource());
    }
}
