//! Multi-configuration pipeline: load, parse, merge, validate.
//!
//! [`Multi`] keeps a map of named entries (`None` = the unnamed bucket). Everything needed to
//! build a pipeline is re-exported here, so `use tanzim::pipeline::multi::*;` is enough on its own.

use super::{Merged, Parsed};
use crate::source;
use cfg_if::cfg_if;
use tanzim_source::{OnError, Stage};

pub use crate::merger::{self, DeepMerge, LastWins, Merge};
pub use crate::source::Source;
#[cfg(feature = "validate-schema")]
pub use crate::validator;
pub use crate::{loader, parser};

/// Validation schemas keyed by merged entry name.
#[cfg(feature = "validate-schema")]
pub type Schemas = std::collections::HashMap<Option<String>, validator::Value>;

fn source_display(cs: &Source) -> String {
    let mut s = cs.source().to_string();
    if cs.resource_colon() || !cs.resource().is_empty() {
        s.push(':');
        s.push_str(cs.resource());
    }
    s
}

/// Errors produced by the multi-configuration pipeline.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no loaders registered")]
    NoLoaders,
    #[error("no parsers registered")]
    NoParsers,
    #[error("no merger registered")]
    NoMerger,
    #[error(transparent)]
    Source(source::ParseError),
    #[error(transparent)]
    Load(loader::Error),
    #[error(transparent)]
    Parse(tanzim_value::Error),
    #[error(transparent)]
    Merge(merger::Error),
    #[error(transparent)]
    Deserialize(tanzim_value::Error),
    #[error("no loader found for `{at}`")]
    NoLoader { at: String },
    #[error("no parser found for format `{format}` in `{at}`")]
    NoParser { format: String, at: String },

    #[cfg(feature = "validate-schema")]
    #[error("schema for `{name:?}` is invalid: {inner}")]
    Schema {
        name: Option<String>,
        inner: validator::SchemaError,
    },
    #[cfg(feature = "validate-schema")]
    #[error("configuration `{name:?}` failed validation: {inner}")]
    Validate {
        name: Option<String>,
        inner: validator::Error,
    },
}

/// Runs the load → parse → merge → validate pipeline for multiple named configuration entries.
///
/// Construct with [`Multi::default`] (all feature-enabled loaders + parsers, no merger) or
/// [`Multi::empty`] (nothing registered). There is no `new()`. Add a merger with
/// [`with_merger`](Self::with_merger) and sources with [`with_source`](Self::with_source) /
/// [`add_source`](Self::add_source), then call [`run`](Self::run) or
/// [`try_deserialize`](Self::try_deserialize).
pub struct Multi {
    sources: Vec<Source>,
    loaders: Vec<Box<dyn loader::Load>>,
    parsers: Vec<Box<dyn parser::Parse>>,
    merger: Option<Box<dyn merger::Merge>>,
    #[cfg(feature = "validate-schema")]
    schemas: Schemas,
}

impl Default for Multi {
    /// All feature-enabled loaders and parsers, but no merger and no sources.
    fn default() -> Self {
        Self::empty()
            .with_included_loaders()
            .with_included_parsers()
    }
}

impl Multi {
    /// An empty pipeline: no loaders, parsers, merger, or sources.
    pub fn empty() -> Self {
        Self {
            sources: Vec::new(),
            loaders: Vec::new(),
            parsers: Vec::new(),
            merger: None,
            #[cfg(feature = "validate-schema")]
            schemas: Schemas::new(),
        }
    }

    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    pub fn sources_mut(&mut self) -> &mut Vec<Source> {
        &mut self.sources
    }

    pub fn loaders(&self) -> &[Box<dyn loader::Load>] {
        &self.loaders
    }

    pub fn loaders_mut(&mut self) -> &mut Vec<Box<dyn loader::Load>> {
        &mut self.loaders
    }

    pub fn parsers(&self) -> &[Box<dyn parser::Parse>] {
        &self.parsers
    }

    pub fn parsers_mut(&mut self) -> &mut Vec<Box<dyn parser::Parse>> {
        &mut self.parsers
    }

    pub fn merger(&self) -> Option<&dyn merger::Merge> {
        self.merger.as_deref()
    }

    pub fn merger_mut(&mut self) -> &mut Option<Box<dyn merger::Merge>> {
        &mut self.merger
    }

    #[cfg(feature = "validate-schema")]
    pub fn schemas(&self) -> &Schemas {
        &self.schemas
    }

    #[cfg(feature = "validate-schema")]
    pub fn schemas_mut(&mut self) -> &mut Schemas {
        &mut self.schemas
    }

    /// Append a configuration source (in-place).
    pub fn add_source(&mut self, source: Source) -> &mut Self {
        self.sources.push(source);
        self
    }

    /// Append a configuration source (builder-style).
    pub fn with_source(mut self, source: Source) -> Self {
        self.sources.push(source);
        self
    }

    pub fn with_loader(mut self, loader: impl loader::Load + 'static) -> Self {
        self.loaders.push(Box::new(loader));
        self
    }

    pub fn with_parser(mut self, parser: impl parser::Parse + 'static) -> Self {
        self.parsers.push(Box::new(parser));
        self
    }

    pub fn with_merger(mut self, merger: impl merger::Merge + 'static) -> Self {
        self.merger = Some(Box::new(merger));
        self
    }

    #[allow(unused_mut)]
    pub fn with_included_loaders(mut self) -> Self {
        cfg_if! {
            if #[cfg(feature = "load-env")] {
                self.loaders.push(Box::new(loader::env::Env::new()));
            }
        }
        cfg_if! {
            if #[cfg(feature = "load-file")] {
                self.loaders.push(Box::new(loader::file::File::new()));
            }
        }
        // `http` requires a user-supplied fetch closure and is not auto-included.
        self
    }

    pub fn set_included_loaders(mut self) -> Self {
        self.loaders.clear();
        self.with_included_loaders()
    }

    #[allow(unused_mut)]
    pub fn with_included_parsers(mut self) -> Self {
        cfg_if! {
            if #[cfg(feature = "parse-env")] {
                self.parsers.push(Box::new(parser::env::Env::new()));
            }
        }
        cfg_if! {
            if #[cfg(feature = "parse-json")] {
                self.parsers.push(Box::new(parser::json::Json::new()));
            }
        }
        cfg_if! {
            if #[cfg(feature = "parse-yaml")] {
                self.parsers.push(Box::new(parser::yaml::Yaml::new()));
            }
        }
        cfg_if! {
            if #[cfg(feature = "parse-toml")] {
                self.parsers.push(Box::new(parser::toml::Toml::new()));
            }
        }
        self
    }

    pub fn set_included_parsers(mut self) -> Self {
        self.parsers.clear();
        self.with_included_parsers()
    }

    #[cfg(feature = "validate-schema")]
    pub fn with_schema(
        mut self,
        name: Option<String>,
        schema: impl Into<validator::Value>,
    ) -> Self {
        self.schemas.insert(name, schema.into());
        self
    }

    #[cfg(feature = "validate-schema")]
    pub fn with_schemas(mut self, schemas: Schemas) -> Self {
        for (name, schema) in schemas {
            self.schemas.insert(name, schema);
        }
        self
    }

    pub fn load(&self) -> Result<Vec<loader::Payload>, Error> {
        if self.loaders.is_empty() {
            return Err(Error::NoLoaders);
        }
        let mut result = Vec::new();
        for config_source in &self.sources {
            let source_name = config_source.source();
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::debug!(msg = "Loading configuration source", source = source_name, resource = config_source.resource());
                } else if #[cfg(feature = "logging")] {
                    log::debug!("msg=\"Loading configuration source\" source={source_name} resource={}", config_source.resource());
                }
            }
            let mut found_loader = None;
            for loader in &self.loaders {
                let supported = loader.supported_source_list();
                let mut matches = false;
                for s in &supported {
                    if s.as_str() == source_name {
                        matches = true;
                        break;
                    }
                }
                if matches {
                    found_loader = Some(loader);
                    break;
                }
            }
            let loader = match found_loader {
                Some(l) => l,
                None => {
                    return Err(Error::NoLoader {
                        at: source_display(config_source),
                    });
                }
            };
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Found loader for configuration source", loader = loader.name(), source = source_name);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Found loader for configuration source\" loader={} source={source_name}", loader.name());
                }
            }
            let payloads = match loader.load(config_source.clone()) {
                Ok(payloads) => payloads,
                Err(e) => {
                    if config_source.on_error(Stage::Load) == OnError::Skip {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::warn!(msg = "Skipped load error for source", source = source_display(config_source), error = ?e);
                            } else if #[cfg(feature = "logging")] {
                                let display = source_display(config_source);
                                log::warn!("msg=\"Skipped load error for source\" source={display} error={e:?}");
                            }
                        }
                        continue;
                    }
                    return Err(Error::Load(e));
                }
            };
            for payload in payloads {
                result.push(payload);
            }
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Configuration load stage complete", payload_count = result.len());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Configuration load stage complete\" payload_count={}", result.len());
            }
        }
        Ok(result)
    }

    pub fn parse(&self, loaded: &[loader::Payload]) -> Result<Vec<Parsed>, Error> {
        if self.parsers.is_empty() {
            return Err(Error::NoParsers);
        }
        let mut result = Vec::new();
        for payload in loaded {
            let config_source = &payload.source;
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::debug!(msg = "Parsing configuration payload", source = %payload.source, format = payload.maybe_format.as_deref().unwrap_or("auto"));
                } else if #[cfg(feature = "logging")] {
                    let fmt = payload.maybe_format.as_deref().unwrap_or("auto");
                    log::debug!("msg=\"Parsing configuration payload\" source={} format={fmt}", payload.source);
                }
            }
            let mut found_parser = None;
            if let Some(format) = &payload.maybe_format {
                for parser in &self.parsers {
                    let supported = parser.supported_format_list();
                    let mut matches = false;
                    for s in &supported {
                        if s.as_str() == format.as_str() {
                            matches = true;
                            break;
                        }
                    }
                    if matches {
                        found_parser = Some(parser);
                        break;
                    }
                }
            }
            if found_parser.is_none() {
                for parser in &self.parsers {
                    if let Some(true) = parser.is_format_supported(&payload.content) {
                        found_parser = Some(parser);
                        break;
                    }
                }
            }
            let parser = match found_parser {
                Some(p) => p,
                None => {
                    return Err(Error::NoParser {
                        format: payload
                            .maybe_format
                            .as_deref()
                            .unwrap_or("unknown")
                            .to_string(),
                        at: source_display(config_source),
                    });
                }
            };
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::trace!(msg = "Found parser for configuration payload", parser = parser.name(), source = %payload.source);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Found parser for configuration payload\" parser={} source={}", parser.name(), payload.source);
                }
            }
            let value = match parser.parse(&payload.source, &payload.content, &self.sources) {
                Ok(v) => v,
                Err(e) => {
                    if config_source.on_error(Stage::Parse) == OnError::Skip {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::warn!(msg = "Skipped parse error for payload", source = %payload.source, error = ?e);
                            } else if #[cfg(feature = "logging")] {
                                log::warn!("msg=\"Skipped parse error for payload\" source={} error={e:?}", payload.source);
                            }
                        }
                        continue;
                    }
                    return Err(Error::Parse(e));
                }
            };
            result.push(Parsed::new(payload.clone(), value));
        }
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Configuration parse stage complete", parsed_count = result.len());
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Configuration parse stage complete\" parsed_count={}", result.len());
            }
        }
        Ok(result)
    }

    pub fn merge(&self, parsed: &[Parsed]) -> Result<Merged, Error> {
        let merger = match &self.merger {
            Some(merger) => merger,
            None => return Err(Error::NoMerger),
        };
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Starting configuration merge stage", entry_count = parsed.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Starting configuration merge stage\" entry_count={}", parsed.len());
            }
        }
        let mut tuples: Vec<(loader::Payload, parser::LocatedValue)> =
            Vec::with_capacity(parsed.len());
        for item in parsed {
            tuples.push((item.payload().clone(), item.value().clone()));
        }
        match merger.merge(&tuples) {
            Ok(raw) => {
                let merged = Merged::from_raw(raw);
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::info!(msg = "Configuration merge stage complete", group_count = merged.len());
                    } else if #[cfg(feature = "logging")] {
                        log::info!("msg=\"Configuration merge stage complete\" group_count={}", merged.len());
                    }
                }
                Ok(merged)
            }
            Err(e) => Err(Error::Merge(e)),
        }
    }

    /// Validate (and coerce) the merged configuration against the registered schemas.
    pub fn validate(&self, _merged: &mut Merged) -> Result<(), Error> {
        #[cfg(feature = "validate-schema")]
        {
            if self.schemas.is_empty() {
                return Ok(());
            }
            let registry = validator::Registry::with_builtins();
            for (name, schema) in &self.schemas {
                let validator = match registry.build_value(schema) {
                    Ok(validator) => validator,
                    Err(inner) => {
                        return Err(Error::Schema {
                            name: name.clone(),
                            inner,
                        });
                    }
                };
                match _merged.get_mut(name) {
                    Some(entry) => match validator::validate(validator.as_ref(), entry.value_mut())
                    {
                        Ok(()) => {}
                        Err(inner) => {
                            return Err(Error::Validate {
                                name: name.clone(),
                                inner,
                            });
                        }
                    },
                    None => {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::warn!(msg = "Schema has no matching merged entry", name = ?name);
                            } else if #[cfg(feature = "logging")] {
                                log::warn!("msg=\"Schema has no matching merged entry\" name={name:?}");
                            }
                        }
                    }
                }
            }
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::info!(msg = "Configuration validation stage complete", schema_count = self.schemas.len());
                } else if #[cfg(feature = "logging")] {
                    log::info!("msg=\"Configuration validation stage complete\" schema_count={}", self.schemas.len());
                }
            }
        }
        Ok(())
    }

    /// Run load → parse → merge → validate in sequence.
    pub fn run(&self) -> Result<Merged, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Running multi configuration pipeline", source_count = self.sources.len(), loader_count = self.loaders.len(), parser_count = self.parsers.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Running multi configuration pipeline\" source_count={} loader_count={} parser_count={}", self.sources.len(), self.loaders.len(), self.parsers.len());
            }
        }
        let loaded = self.load()?;
        let parsed = self.parse(&loaded)?;
        let mut merged = self.merge(&parsed)?;
        self.validate(&mut merged)?;
        Ok(merged)
    }

    /// Run the pipeline and deserialize each named entry's configuration into `T`, keyed by
    /// entry name (the first failure aborts).
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<std::collections::HashMap<Option<String>, T>, Error> {
        let merged = self.run()?;
        let mut out = std::collections::HashMap::with_capacity(merged.len());
        for (name, entry) in merged.iter() {
            let deserialized = match entry.value().try_deserialize::<T>() {
                Ok(value) => value,
                Err(error) => {
                    return Err(Error::Deserialize(crate::attach_source_text(
                        error,
                        entry.payloads(),
                    )));
                }
            };
            out.insert(name.clone(), deserialized);
        }
        Ok(out)
    }
}
