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
/// `maybe_format` selects the parser (`json`, `env`, …). `content` is unparsed bytes.
/// `maybe_name` is `None` for unnamed payloads; all `None`-named payloads merge together.
/// Built-in loaders lower-case `maybe_name` and `maybe_format` by default (their Source `lowercase=true` option).
#[derive(Debug, Clone, PartialEq)]
pub struct Payload {
    pub source: Source,
    pub maybe_name: Option<String>,
    pub maybe_format: Option<String>,
    pub content: Vec<u8>,
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
/// exact resource that was loaded (e.g. a file path inside a directory).
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
///                 maybe_name: Some(name.to_string()),
///                 maybe_format: Some("json".into()),
///                 content: bytes.to_vec(),
///             });
///         }
///         Ok(result)
///     }
/// }
/// ```
pub trait Load {
    /// Human-readable name used in error messages.
    fn name(&self) -> &str;
    /// Source strings this loader handles (e.g. ["env"], `["file"]`, `["http", "https"]`).
    fn supported_source_list(&self) -> Vec<String>;
    /// Load raw bytes from the source. Returns one [`Payload`] per config entry found.
    fn load(&self, source: Source) -> Result<Vec<Payload>, Error>;
}
