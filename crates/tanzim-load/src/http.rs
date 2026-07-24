//! HTTP loader (`http-closure` feature).
//!
//! Fetches configuration bytes through a user-provided closure so this crate does not depend on
//! any HTTP client library. You supply the actual transport (via [`Http::new`]); the loader
//! validates options, invokes your closure, and normalizes the returned entries.
//!
//! Enabling the feature alone does not fetch anything — there is no built-in client. Construct an
//! [`Http`] with your closure and pass that loader into whatever runs the load stage.
//!
//! **Source:** `http` (the resource is the URL and is required; an empty or unparseable resource
//! is rejected with [`Error::InvalidResource`])
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

pub use url::Url;

/// Loader name reported by [`Http::name`](crate::Load::name) and used in error messages.
pub const NAME: &str = "HTTP";
/// Source string handled by [`Http`] (see [`Load::supported_source_list`]).
pub const SOURCE: &str = "http";

/// The transport closure driving an [`Http`] loader — you implement the actual request here.
///
/// Called once per source with, in order:
///
/// - [`&Url`](Url) — the parsed resource URL.
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
    dyn Fn(Source, &Url, &HashMap<String, String>, Duration, bool) -> Result<Vec<Payload>, String>
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
/// use tanzim_load::{http::{Http, Url}, Load, Payload, Source};
/// use tanzim_source::SourceBuilder;
///
/// // A real fetch would call an HTTP client here; this canned closure needs no network.
/// let http = Http::new(Box::new(
///     |source: Source, url: &Url, _headers: &HashMap<String, String>, _timeout: Duration, _insecure: bool| {
///         // Fetch the configuration from the URL
///         Ok(vec![Payload {
///             source,
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
        let url = url::Url::parse(&resource).map_err(|error| Error::InvalidResource {
            loader: NAME.to_string(),
            resource: resource.clone(),
            reason: error.to_string(),
        })?;

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
            (self.fetch)(source.clone(), &url, &headers, timeout, insecure).map_err(|error| {
                Error::Load {
                    loader: NAME.to_string(),
                    resource: resource.to_string(),
                    description: "fetch configuration".into(),
                    source: error.into(),
                }
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
