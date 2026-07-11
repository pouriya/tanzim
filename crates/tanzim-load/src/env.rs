//! Environment-variable loader (`env` feature).
//!
//! Reads process environment variables and groups them into configuration entries using a
//! configurable `prefix` and an optional key `separator`.
//!
//! **Source:** `env` (the source resource must be empty — a non-empty resource is rejected with
//! [`Error::InvalidResource`])
//!
//! # Behaviour
//!
//! - Only variables whose name starts with `prefix` are considered. With `strip_prefix = true`
//!   (the default when a prefix is set) the prefix is removed from each key first.
//! - Without a `separator`, every matching variable becomes a `KEY="value"` line in a single
//!   unnamed entry (`maybe_name = None`) that merges into the configuration root.
//! - With a `separator`, each key is split once on its first occurrence: the left part is the
//!   entry name and the right part is the key within that entry. Keys that don't contain the
//!   separator (or whose split yields an empty side) are skipped.
//! - Every produced entry has `maybe_format = "env"`, so the `KEY="value"` lines are handed to
//!   the `env` parser. Entry names are lower-cased when `lowercase = true` (the default).
//!
//! # Options
//!
//! - `prefix` — string (default: detected from `CARGO_BIN_NAME`, `CARGO_CRATE_NAME`, or the
//!   executable name, with `_` suffix)
//! - `strip_prefix` — boolean (default `true`; only applies when `prefix` is non-empty)
//! - `separator` — string (no default; when set, splits keys into entry name and content key)
//! - `lowercase` — boolean (default `true`; whether to lowercase the entry names)
//!
//! # Example
//!
//! ```text
//! env
//! env(prefix=APP_NAME_,separator=.)
//! ```

use crate::{Error, Load, Payload, Source};
use cfg_if::cfg_if;
use std::{collections::HashMap, env};

/// Loader name reported by [`Env::name`](crate::Load::name) and used in error messages.
pub const NAME: &str = "Environment-Variables";
/// Source string handled by [`Env`] (see [`Load::supported_source_list`]).
pub const SOURCE: &str = "env";

/// Loader for the `env` source: reads process environment variables into configuration entries.
///
/// See the [module docs](self) for the grouping behaviour and options. Construct with
/// [`Env::new`], or pin a prefix with [`Env::with_prefix`] instead of relying on the
/// auto-detected default.
///
/// # Example
///
/// ```
/// use tanzim_load::{env::Env, Load};
/// use tanzim_source::SourceBuilder;
///
/// # tanzim_testing::environment::run(|env| {
/// #     env.set_env("MYAPP_DEBUG", "true")?;
/// // The process environment holds a single matching variable: `MYAPP_DEBUG=true`.
/// let source = SourceBuilder::new()
///     .with_source("env")
///     .with_option("prefix", "MYAPP_")
///     .build()
///     .unwrap();
///
/// let payloads = Env::new().load(source).unwrap();
/// let content = String::from_utf8_lossy(&payloads[0].content);
/// // The `MYAPP_` prefix is stripped, leaving `DEBUG="true"`.
/// assert!(content.contains(r#"DEBUG="true""#));
/// # Ok(())
/// # })
/// # .unwrap();
/// ```
#[derive(Debug, Default, Clone)]
pub struct Env {
    prefix_override: Option<String>,
}

impl Env {
    /// Create a loader whose prefix is taken from the source's `prefix` option, or
    /// auto-detected (see [`Env::detect_prefix`]) when that option is absent.
    pub fn new() -> Self {
        Default::default()
    }

    /// Detect the prefix from the environment variables.
    /// The prefix is the string that is prepended to the environment variable names.
    /// The default prefix is the name of the cargo bin `CARGO_BIN_NAME`, cargo crate `CARGO_CRATE_NAME`, or the executable name, with `_` suffix.
    pub fn detect_prefix() -> Option<String> {
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

        if prefix.is_empty() {
            None
        } else {
            Some(prefix)
        }
    }

    /// Set the prefix override when `maybe_prefix` is `Some`; otherwise leave it unchanged.
    pub fn set_maybe_prefix<P: Into<String>>(&mut self, maybe_prefix: Option<P>) {
        if let Some(prefix) = maybe_prefix {
            self.set_prefix(prefix);
        }
    }

    /// Pin the prefix, overriding the source's `prefix` option and the auto-detected default.
    pub fn set_prefix<P: Into<String>>(&mut self, prefix: P) {
        self.prefix_override = Some(prefix.into());
    }

    /// Builder form of [`Env::set_prefix`].
    pub fn with_prefix<P: Into<String>>(mut self, prefix: P) -> Self {
        self.set_prefix(prefix.into());
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

        let maybe_prefix = if let Some(prefix_override) = &self.prefix_override {
            Some(prefix_override.clone())
        } else {
            match options.get("prefix") {
                None => None,
                Some(value) => {
                    if let Some(prefix) = value.as_string() {
                        Some(prefix.into())
                    } else {
                        return Err(Error::InvalidOption {
                            loader: NAME.to_string(),
                            key: "prefix".to_string(),
                            reason: format!("expected string, found {}", value.type_name()),
                        });
                    }
                }
            }
        };

        let separator = match options.get("separator") {
            None => None,
            Some(value) => {
                if let Some(separator) = value.as_string() {
                    Some(separator.clone())
                } else {
                    return Err(Error::InvalidOption {
                        loader: NAME.to_string(),
                        key: "separator".to_string(),
                        reason: format!("expected string, found {}", value.type_name()),
                    });
                }
            }
        };

        let strip_prefix = if let Some(strip_prefix) = options.get("strip_prefix") {
            if let Some(strip_prefix) = strip_prefix.as_bool() {
                strip_prefix
            } else {
                if maybe_prefix.is_some() {
                    return Err(Error::InvalidOption {
                        loader: NAME.to_string(),
                        key: "strip_prefix".to_string(),
                        reason: format!("expected boolean, found {}", strip_prefix.type_name()),
                    });
                }
                false
            }
        } else {
            maybe_prefix.is_some()
        };

        let lowercase = if let Some(value) = options.get("lowercase") {
            if let Some(value) = value.as_bool() {
                value
            } else {
                return Err(Error::InvalidOption {
                    loader: NAME.to_string(),
                    key: "lowercase".to_string(),
                    reason: format!("expected boolean, found {}", value.type_name()),
                });
            }
        } else {
            true
        };

        let prefix = maybe_prefix.unwrap_or_default();

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Loading configuration from environment variables", prefix = prefix, strip_prefix = strip_prefix, separator = ?separator, lowercase = lowercase);
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Loading configuration from environment variables\" prefix={prefix} strip_prefix={strip_prefix} separator={separator:?} lowercase={lowercase}");
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
                    let first = parts.next().unwrap_or("").trim();
                    let Some(rest) = parts.next() else {
                        continue;
                    };
                    let rest = rest.trim();
                    if first.is_empty() || rest.is_empty() {
                        continue;
                    }
                    let entry_name = if lowercase {
                        let lower = first.to_lowercase();
                        if lower != first {
                            cfg_if! {
                                if #[cfg(feature = "tracing")] {
                                    tracing::debug!(msg = "Lowercased environment variable entry name", from = first, to = lower.as_str(), env_key = env_key);
                                } else if #[cfg(feature = "logging")] {
                                    log::debug!("msg=\"Lowercased environment variable entry name\" from={first} to={lower} env_key={env_key}");
                                }
                            }
                        }
                        lower
                    } else {
                        first.to_string()
                    };
                    (Some(entry_name), rest.to_string())
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

        let mut payload_list = Vec::with_capacity(grouped.len());
        for (maybe_name, content) in grouped {
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Detected configuration from environment variables", name = ?maybe_name.as_deref().unwrap_or("<empty>"), format = "env");
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Detected configuration from environment variables\" name={} format=\"env\"", maybe_name.as_deref().unwrap_or("<empty>"));
                }
            }
            payload_list.push(Payload {
                source: source.clone(),
                maybe_name,
                maybe_format: Some("env".into()),
                content,
            });
        }

        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Loaded configuration from environment variables", group_count = payload_list.len());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Loaded configuration from environment variables\" group_count={}", payload_list.len());
            }
        }

        Ok(payload_list)
    }
}
