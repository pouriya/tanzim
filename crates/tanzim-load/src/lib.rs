#![doc = include_str!("../README.md")]

use std::error::Error as StdError;

pub use tanzim_source::{OptionValue, Options, Source};

pub mod closure;
#[cfg(feature = "env")]
pub mod env;
#[cfg(feature = "file")]
pub mod file;
#[cfg(feature = "http")]
pub mod http;

/// Raw bytes for one configuration entry, with its declaring [`Source`].
///
/// `name` and `format` are lowercased by [Payload::normalize].
/// `format` selects the parser (`json`, `env`, …). `content` is unparsed bytes.
/// `name` is `None` for unnamed payloads; all `None`-named payloads merge together.
#[derive(Debug, Clone, PartialEq)]
pub struct Payload {
    pub source: Source,
    pub name: Option<String>,
    pub format: Option<String>,
    pub content: Vec<u8>,
}

impl Payload {
    pub fn normalize(mut self) -> Self {
        if let Some(name) = self.name {
            if name.is_empty() {
                self.name = None;
            } else {
                self.name = Some(name.to_lowercase());
            }
        }
        if let Some(format) = self.format {
            if format.is_empty() {
                self.format = None;
            } else {
                self.format = Some(format.to_lowercase());
            }
        }
        self
    }
}

/// Load error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{loader} configuration loader could not find {item} at `{resource}`")]
    NotFound {
        loader: String,
        resource: String,
        item: String,
    },
    #[error("{loader} configuration loader has no access to `{resource}`")]
    NoAccess {
        loader: String,
        resource: String,
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error(
        "{loader} configuration loader reached timeout `{timeout_in_seconds}s` for `{resource}`"
    )]
    Timeout {
        loader: String,
        resource: String,
        timeout_in_seconds: u64,
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("{loader} configuration loader invalid option `{key}`: {reason}")]
    InvalidOption {
        loader: String,
        key: String,
        reason: String,
    },
    #[error("{loader} configuration loader invalid resource `{resource}`: {reason}")]
    InvalidResource {
        loader: String,
        resource: String,
        reason: String,
    },
    #[error(
        "{loader} configuration loader found duplicate configurations `{resource}/{name}.({format_1}|{format_2})`"
    )]
    Duplicate {
        loader: String,
        resource: String,
        name: String,
        format_1: String,
        format_2: String,
    },
    #[error("{loader} configuration loader could not {description} `{resource}`")]
    Load {
        loader: String,
        resource: String,
        description: String,
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error(transparent)]
    Other(#[from] Box<dyn StdError + Send + Sync>),
}

/// Loads raw configuration bytes from a declared source.
///
/// Implement this to add a new source kind (protocol, service, database, …).
/// Each call takes ownership of a [`Source`] and returns one [`Payload`] per
/// configuration entry found. Set [`Payload::source`] on each entry to reflect the
/// exact resource that was loaded (e.g. a file path inside a directory). Call
/// [`Payload::normalize`] on each payload before returning.
///
/// # Example — custom in-memory loader
///
/// ```rust
/// use tanzim_load::{Error, Load, Payload, Source};
///
/// struct MemoryLoader {
///     entries: Vec<(&'static str, &'static [u8])>,
/// }
///
/// impl Load for MemoryLoader {
///     fn name(&self) -> &str { "memory" }
///     fn supported_source_list(&self) -> Vec<String> { vec!["memory".into()] }
///     fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
///         let mut result = Vec::new();
///         for (name, bytes) in &self.entries {
///             result.push(Payload {
///                 source: source.clone(),
///                 name: Some(name.to_string()),
///                 format: Some("json".into()),
///                 content: bytes.to_vec(),
///             }.normalize());
///         }
///         Ok(result)
///     }
/// }
/// ```
pub trait Load {
    /// Human-readable name used in error messages.
    fn name(&self) -> &str;
    /// Source strings this loader handles (e.g. `["file"]`, `["http", "https"]`).
    fn supported_source_list(&self) -> Vec<String>;
    /// Load raw bytes from the source. Returns one [`Payload`] per config entry found.
    fn load(&self, source: Source) -> Result<Vec<Payload>, Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanzim_source::SourceBuilder;

    fn test_source() -> Source {
        SourceBuilder::new().with_source("test").build().unwrap()
    }

    #[test]
    fn payload_normalize_clears_empty_name() {
        let payload = Payload {
            source: test_source(),
            name: Some(String::new()),
            format: Some("json".into()),
            content: b"{}".to_vec(),
        }
        .normalize();
        assert!(payload.name.is_none());
    }

    #[test]
    fn payload_normalize_lowercases_name() {
        let payload = Payload {
            source: test_source(),
            name: Some("FOO".into()),
            format: None,
            content: Vec::new(),
        }
        .normalize();
        assert_eq!(payload.name, Some("foo".into()));
    }
}
