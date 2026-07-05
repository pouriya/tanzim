//! A [`config`](https://docs.rs/config)-inspired configuration facade.
//!
//! [`Config`] wires an opinionated [`single`](crate::pipeline::single) pipeline: all
//! feature-enabled loaders and parsers, plus [`DeepMerge`] as the merge
//! strategy. You declare where configuration comes from with the [`File`] and [`Environment`]
//! source helpers, `build()` the pipeline, then read values by dotted key or deserialize the whole
//! thing into your own type.
//!
//! ```no_run
//! use tanzim::opt_in::config::{Config, Environment, File};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::builder()
//!     .add_source(File::with_name("config").required(false))
//!     .add_source(Environment::with_prefix("APP").separator("."))
//!     .build()?;
//!
//! let port: u16 = config.get("server.port")?;
//! let name = config.get_string("server.name")?;
//! # let _ = (port, name);
//! # Ok(())
//! # }
//! ```

mod env;
mod file;

pub use env::Environment;
pub use file::File;

use crate::merger::DeepMerge;
use crate::pipeline::Entry;
use crate::pipeline::single::{Error as PipelineError, Single};
use crate::source::Source;
use tanzim_value::{LocatedValue, ValueType};

/// Errors produced by [`Config`].
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A pipeline stage failed while building the configuration.
    #[error(transparent)]
    Pipeline(Box<PipelineError>),
    /// A value could not be deserialized into the requested type.
    #[error(transparent)]
    Deserialize(tanzim_value::Error),
    /// No value exists at the requested dotted key.
    #[error("configuration key `{key}` not found")]
    NotFound { key: String },
    /// The value at the requested key is not of the expected primitive type.
    #[error("configuration key `{key}` is `{actual}`, expected `{expected}`")]
    TypeMismatch {
        key: String,
        expected: ValueType,
        actual: ValueType,
    },
}

/// Builds a [`Config`] from one or more sources.
///
/// Seeded with all feature-enabled loaders and parsers and the
/// [`DeepMerge`] strategy. Add sources with [`add_source`](Self::add_source)
/// then call [`build`](Self::build).
pub struct ConfigBuilder {
    pipeline: Single,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            pipeline: Single::default().with_merger(DeepMerge),
        }
    }
}

impl ConfigBuilder {
    /// Add a configuration source. Accepts the [`File`] and [`Environment`] helpers (both convert
    /// into a [`Source`]).
    pub fn add_source(mut self, source: impl Into<Source>) -> Self {
        self.pipeline = self.pipeline.with_source(source.into());
        self
    }

    /// Access the underlying [`Single`] pipeline for advanced configuration (e.g. registering a
    /// custom loader, parser, or merger).
    pub fn pipeline_mut(&mut self) -> &mut Single {
        &mut self.pipeline
    }

    /// Run the pipeline and capture the unified configuration.
    pub fn build(self) -> Result<Config, ConfigError> {
        match self.pipeline.run() {
            Ok(entry) => Ok(Config { entry }),
            Err(error) => Err(ConfigError::Pipeline(Box::new(error))),
        }
    }
}

/// A built configuration: the unified value tree plus its source provenance.
///
/// Construct one with [`Config::builder`].
pub struct Config {
    entry: Entry,
}

impl Config {
    /// Start building a configuration.
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Deserialize the whole configuration into `T`.
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, ConfigError> {
        match self.entry.value().try_deserialize::<T>() {
            Ok(value) => Ok(value),
            Err(error) => Err(ConfigError::Deserialize(crate::attach_source_text(
                error,
                self.entry.payloads(),
            ))),
        }
    }

    /// Deserialize the value at a dotted `key` (e.g. `"server.port"`) into `T`.
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        let located = self.lookup(key)?;
        match located.try_deserialize::<T>() {
            Ok(value) => Ok(value),
            Err(error) => Err(ConfigError::Deserialize(crate::attach_source_text(
                error,
                self.entry.payloads(),
            ))),
        }
    }

    /// Read the string at a dotted `key`.
    pub fn get_string(&self, key: &str) -> Result<String, ConfigError> {
        let located = self.lookup(key)?;
        match located.value().as_string() {
            Some(value) => Ok(value.clone()),
            None => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: ValueType::String,
                actual: located.value().type_name(),
            }),
        }
    }

    /// Read the integer at a dotted `key`.
    pub fn get_int(&self, key: &str) -> Result<i64, ConfigError> {
        let located = self.lookup(key)?;
        match located.value().as_int() {
            Some(value) => Ok(value as i64),
            None => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: ValueType::Int,
                actual: located.value().type_name(),
            }),
        }
    }

    /// Read the boolean at a dotted `key`.
    pub fn get_bool(&self, key: &str) -> Result<bool, ConfigError> {
        let located = self.lookup(key)?;
        match located.value().as_bool() {
            Some(value) => Ok(value),
            None => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: ValueType::Bool,
                actual: located.value().type_name(),
            }),
        }
    }

    /// Read the float at a dotted `key`.
    pub fn get_float(&self, key: &str) -> Result<f64, ConfigError> {
        let located = self.lookup(key)?;
        match located.value().as_float() {
            Some(value) => Ok(value),
            None => Err(ConfigError::TypeMismatch {
                key: key.to_string(),
                expected: ValueType::Float,
                actual: located.value().type_name(),
            }),
        }
    }

    /// The unified configuration value tree.
    pub fn value(&self) -> &LocatedValue {
        self.entry.value()
    }

    /// Walk the dotted `key` through nested maps to the located value it names.
    fn lookup(&self, key: &str) -> Result<&LocatedValue, ConfigError> {
        let mut current = self.entry.value();
        for segment in key.split('.') {
            let map = match current.value().as_map() {
                Some(map) => map,
                None => {
                    return Err(ConfigError::NotFound {
                        key: key.to_string(),
                    });
                }
            };
            match map.get(segment) {
                Some(next) => current = next,
                None => {
                    return Err(ConfigError::NotFound {
                        key: key.to_string(),
                    });
                }
            }
        }
        Ok(current)
    }
}
