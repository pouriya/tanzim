//! Environment-variable loader (`env` feature).
//!
//! Groups process environment variables by name using a configurable prefix
//! and optional key separator.
//!
//! **Source:** `env`
//!
//! # Options
//!
//! - `prefix` — string (default: detected from `CARGO_BIN_NAME`, `CARGO_CRATE_NAME`, or the
//!   executable name, with `_` suffix)
//! - `strip_prefix` — boolean (default `true`; only applies when `prefix` is non-empty)
//! - `separator` — string (no default; when set, splits keys into entry name and content key)
//!
//! # Example
//!
//! ```text
//! env
//! env(prefix=APP_NAME,separator=.)
//! ```

use crate::{Error, Load, Payload, Source};
use cfg_if::cfg_if;
use std::{collections::HashMap, env};

pub const NAME: &str = "Environment-Variables";
pub const SOURCE: &str = "env";

const ALLOWED_OPTIONS: &[&str] = &["prefix", "strip_prefix", "separator"];

#[derive(Debug, Default, Clone)]
pub struct Env {
    prefix_override: Option<String>,
}

impl Env {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn detect_prefix() -> String {
        let mut prefix = option_env!("CARGO_BIN_NAME").unwrap_or("").to_string();
        if prefix.is_empty() {
            prefix = option_env!("CARGO_CRATE_NAME").unwrap_or("").to_string();
        }
        if prefix.is_empty()
            && let Ok(path) = env::current_exe()
            && let Some(file_name) = path.file_name().and_then(|name| name.to_str())
        {
            prefix = file_name.to_string();
            #[cfg(windows)]
            if prefix.len() >= 4
                && prefix.as_bytes()[prefix.len() - 4..].eq_ignore_ascii_case(b".exe")
            {
                prefix.truncate(prefix.len() - 4);
            }
        }
        if !prefix.is_empty() {
            prefix.push('_');
        }
        prefix
    }

    pub fn set_prefix<P: AsRef<str>>(&mut self, prefix: P) {
        self.prefix_override = Some(prefix.as_ref().to_string());
    }

    pub fn with_prefix<P: AsRef<str>>(mut self, prefix: P) -> Self {
        self.set_prefix(prefix);
        self
    }
}

impl Load for Env {
    fn name(&self) -> &str {
        NAME
    }

    fn supported_source_list(&self) -> Vec<String> {
        vec![SOURCE.to_string()]
    }

    fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
        let options = source.options().clone();
        let resource = source.resource().to_string();

        if !resource.is_empty() {
            return Err(Error::InvalidResource {
                loader: NAME.to_string(),
                resource: resource.to_string(),
                reason: "resource must be empty".into(),
            });
        }

        for key in options.keys() {
            if !ALLOWED_OPTIONS.contains(&key) {
                return Err(Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: key.to_string(),
                    reason: "unknown option".into(),
                });
            }
        }

        let prefix = if let Some(prefix_override) = &self.prefix_override {
            prefix_override.clone()
        } else {
            match options.get("prefix") {
                None => Self::detect_prefix(),
                Some(value) => value
                    .as_string()
                    .cloned()
                    .ok_or_else(|| Error::InvalidOption {
                        loader: NAME.to_string(),
                        key: "prefix".to_string(),
                        reason: format!("expected string, found {}", value.type_name()),
                    })?,
            }
        };

        let strip_prefix = if prefix.is_empty() {
            false
        } else {
            match options.get("strip_prefix") {
                None => true,
                Some(value) => value.as_bool().ok_or_else(|| Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: "strip_prefix".to_string(),
                    reason: format!("expected boolean, found {}", value.type_name()),
                })?,
            }
        };

        let separator = match options.get("separator") {
            None => None,
            Some(value) => {
                let separator = value
                    .as_string()
                    .cloned()
                    .ok_or_else(|| Error::InvalidOption {
                        loader: NAME.to_string(),
                        key: "separator".to_string(),
                        reason: format!("expected string, found {}", value.type_name()),
                    })?;
                if separator.is_empty() {
                    None
                } else {
                    Some(separator)
                }
            }
        };

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Loading configuration from environment variables", prefix = prefix, strip_prefix = strip_prefix, separator = ?separator);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Loading configuration from environment variables\" prefix={prefix} strip_prefix={strip_prefix} separator={separator:?}");
            }
        }

        let mut grouped: HashMap<Option<String>, Vec<u8>> = HashMap::new();

        for (key, value) in env::vars() {
            if !prefix.is_empty() && !key.starts_with(&prefix) {
                continue;
            }

            let mut env_key = key;
            if strip_prefix {
                env_key = env_key.chars().skip(prefix.chars().count()).collect();
            }
            if env_key.is_empty() {
                continue;
            }

            let (name, content_key) = match &separator {
                None => (None, env_key),
                Some(separator) => {
                    let mut parts = env_key.splitn(2, separator.as_str());
                    let first = parts.next().unwrap_or("");
                    let Some(rest) = parts.next() else {
                        continue;
                    };
                    if first.is_empty() || rest.is_empty() {
                        continue;
                    }
                    (Some(first.to_lowercase()), rest.to_string())
                }
            };

            let line = format!("{content_key}={value:?}");
            if let Some(content) = grouped.get_mut(&name) {
                content.push(b'\n');
                content.extend_from_slice(line.as_bytes());
            } else {
                grouped.insert(name, line.into_bytes());
            }
        }

        let mut payloads = Vec::with_capacity(grouped.len());
        for (name, content) in grouped {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Detected configuration from environment variables", name = ?name, format = "env");
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Detected configuration from environment variables\" name={name:?} format=\"env\"");
                }
            }
            payloads.push(Payload {
                source: source.clone(),
                name,
                format: Some("env".into()),
                content,
            });
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Loaded configuration from environment variables", group_count = payloads.len());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Loaded configuration from environment variables\" group_count={}", payloads.len());
            }
        }

        Ok(payloads)
    }
}

#[cfg(all(test, feature = "env"))]
mod tests {
    use super::*;
    use std::env;
    use tanzim_source::{Options, SourceBuilder};

    fn make_source_with_options(options: Options) -> Source {
        let mut builder = SourceBuilder::new().with_source("env");
        builder = builder.with_options(options);
        builder.build().unwrap()
    }

    #[test]
    fn load_groups_environment_variables_by_name() {
        // SAFETY: test-only; single-threaded test env vars.
        unsafe {
            env::set_var("TANZIM_TEST__FOO__BAR", "baz");
            env::set_var("TANZIM_TEST__QUX__ABC", "123");
        }

        let mut options = Options::new();
        options.insert("prefix", "TANZIM_TEST__");
        options.insert("separator", "__");
        let loaded = Env::new().load(make_source_with_options(options)).unwrap();

        let mut foo = None;
        let mut qux = None;
        for payload in &loaded {
            if payload.name == Some("foo".to_string()) {
                foo = Some(payload);
            } else if payload.name == Some("qux".to_string()) {
                qux = Some(payload);
            }
        }

        let foo = foo.expect("foo payload");
        assert_eq!(foo.format, Some("env".to_string()));
        assert!(String::from_utf8_lossy(&foo.content).contains("BAR=\"baz\""));

        let qux = qux.expect("qux payload");
        assert!(String::from_utf8_lossy(&qux.content).contains("ABC=\"123\""));
    }

    #[test]
    fn load_without_separator_puts_all_keys_in_one_payload() {
        // SAFETY: test-only; single-threaded test env vars.
        unsafe {
            env::set_var("TANZIM_FLAT__FOO", "1");
            env::set_var("TANZIM_FLAT__BAR", "2");
        }

        let mut options = Options::new();
        options.insert("prefix", "TANZIM_FLAT__");
        let loaded = Env::new().load(make_source_with_options(options)).unwrap();

        assert_eq!(loaded.len(), 1);
        let payload = &loaded[0];
        assert!(payload.name.is_none());
        let content = String::from_utf8_lossy(&payload.content);
        assert!(content.contains("FOO=\"1\""));
        assert!(content.contains("BAR=\"2\""));
    }

    #[test]
    fn load_rejects_non_empty_resource() {
        let source = SourceBuilder::new()
            .with_source("env")
            .with_resource("oops")
            .build()
            .unwrap();
        let error = Env::new().load(source).unwrap_err();
        assert!(matches!(error, Error::InvalidResource { .. }));
    }
}
