#![doc = include_str!("../README.md")]
#![doc(test(no_crate_inject))]

//! # tanzim
//!
//! Load, parse, and merge configuration from declarative configuration sources.
//!
//! Workspace crates:
//!
//! - [`source`] — [`tanzim_source`] ([`tanzim_source::Source`])
//! - [`loader`] — [`tanzim_load`] ([`tanzim_load::Load`])
//! - [`parser`] — [`tanzim_parse`] ([`tanzim_parse::Deserialize`])
//! - [`merge`] — [`tanzim_merge`] ([`tanzim_merge::Merge`])

pub use tanzim_load as loader;
pub use tanzim_merge as merge;
pub use tanzim_parse as parser;
pub use tanzim_source as source;
pub use tanzim_validate as validate;

#[doc(inline)]
pub use tanzim_source::Source;

pub mod ext {
    //! Re-exported dependency crates.

    pub extern crate tanzim_load;
    pub extern crate tanzim_merge;
    pub extern crate tanzim_parse;
    pub extern crate tanzim_source;
    pub extern crate tanzim_validate;
}

mod logging;

use cfg_if::cfg_if;
use std::collections::HashMap;

/// A validation schema keyed by merged entry name (the keys of [`Merged`]).
///
/// Each value is a schema document — a [`validate::Value`] tree, e.g. one deserialized from
/// JSON/YAML into a [`validate::SchemaValue`] and unwrapped with
/// [`validate::SchemaValue::into_value`]. It is turned into a validator and run against the
/// matching merged entry after the merge stage.
pub type Schemas = HashMap<String, validate::Value>;

/// A loaded payload paired with the value tree produced by parsing it.
pub type Parsed = (loader::Payload, parser::LocatedValue);

/// Merged configuration keyed by entry name.
///
/// Identical to [`merge::Merged`]; re-aliased here for the facade's public API.
pub type Merged = merge::Merged;

fn source_display(cs: &Source) -> String {
    let mut s = cs.source().to_string();
    if cs.ignore_errors() {
        s.push('?');
    }
    if cs.resource_colon() || !cs.resource().is_empty() {
        s.push(':');
        s.push_str(cs.resource());
    }
    s
}

/// Errors produced by [`Config`] and [`ConfigBuilder`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Source(source::ParseError),
    #[error(transparent)]
    Load(loader::Error),
    #[error(transparent)]
    Parse(tanzim_value::Error),
    #[error(transparent)]
    Merge(merge::Error),
    #[error("no loader found for `{at}`")]
    NoLoader { at: String },
    #[error("no parser found for format `{format}` in `{at}`")]
    NoParser { format: String, at: String },
    #[error("schema for `{name}` is invalid: {source}")]
    Schema {
        name: String,
        source: validate::SchemaError,
    },
    #[error("configuration `{name}` failed validation: {source}")]
    Validate {
        name: String,
        source: validate::Error,
    },
}

/// Builds a [`Config`] with a fluent API.
///
/// The default merger is [`merge::LastWins`].
pub struct ConfigBuilder {
    sources: Vec<Source>,
    loaders: Vec<Box<dyn loader::Load>>,
    parsers: Vec<Box<dyn parser::Deserialize>>,
    merger: Box<dyn merge::Merge>,
    schemas: Schemas,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            loaders: Vec::new(),
            parsers: Vec::new(),
            merger: Box::new(merge::LastWins),
            schemas: Schemas::new(),
        }
    }
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_source<S>(mut self, source: S) -> Result<Self, Error>
    where
        S: TryInto<Source, Error = source::ParseError>,
    {
        match source.try_into() {
            Ok(src) => {
                self.sources.push(src);
                Ok(self)
            }
            Err(e) => Err(Error::Source(e)),
        }
    }

    pub fn with_loader(mut self, loader: impl loader::Load + 'static) -> Self {
        self.loaders.push(Box::new(loader));
        self
    }

    pub fn with_parser(mut self, parser: impl parser::Deserialize + 'static) -> Self {
        self.parsers.push(Box::new(parser));
        self
    }

    pub fn with_merger(mut self, merger: impl merge::Merge + 'static) -> Self {
        self.merger = Box::new(merger);
        self
    }

    /// Register a validation schema for one merged entry name.
    pub fn with_schema(
        mut self,
        name: impl Into<String>,
        schema: impl Into<validate::Value>,
    ) -> Self {
        self.schemas.insert(name.into(), schema.into());
        self
    }

    /// Register validation schemas for several merged entry names at once.
    pub fn with_schemas(mut self, schemas: Schemas) -> Self {
        for (name, schema) in schemas {
            self.schemas.insert(name, schema);
        }
        self
    }

    pub fn build(self) -> Config {
        Config {
            sources: self.sources,
            loaders: self.loaders,
            parsers: self.parsers,
            merger: self.merger,
            schemas: self.schemas,
        }
    }
}

/// Runs the load → parse → merge pipeline for configuration.
///
/// Construct via [`ConfigBuilder`] or add/modify components with the `with_*` setters.
pub struct Config {
    sources: Vec<Source>,
    loaders: Vec<Box<dyn loader::Load>>,
    parsers: Vec<Box<dyn parser::Deserialize>>,
    merger: Box<dyn merge::Merge>,
    schemas: Schemas,
}

impl Config {
    // ── Getters ──────────────────────────────────────────────────────────────

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

    pub fn parsers(&self) -> &[Box<dyn parser::Deserialize>] {
        &self.parsers
    }

    pub fn parsers_mut(&mut self) -> &mut Vec<Box<dyn parser::Deserialize>> {
        &mut self.parsers
    }

    pub fn merger(&self) -> &dyn merge::Merge {
        &*self.merger
    }

    pub fn merger_mut(&mut self) -> &mut Box<dyn merge::Merge> {
        &mut self.merger
    }

    pub fn schemas(&self) -> &Schemas {
        &self.schemas
    }

    pub fn schemas_mut(&mut self) -> &mut Schemas {
        &mut self.schemas
    }

    // ── Setters (return Self) ─────────────────────────────────────────────────

    pub fn with_source<S>(mut self, source: S) -> Result<Self, Error>
    where
        S: TryInto<Source, Error = source::ParseError>,
    {
        match source.try_into() {
            Ok(src) => {
                self.sources.push(src);
                Ok(self)
            }
            Err(e) => Err(Error::Source(e)),
        }
    }

    pub fn with_loader(mut self, loader: impl loader::Load + 'static) -> Self {
        self.loaders.push(Box::new(loader));
        self
    }

    pub fn with_parser(mut self, parser: impl parser::Deserialize + 'static) -> Self {
        self.parsers.push(Box::new(parser));
        self
    }

    pub fn with_merger(mut self, merger: impl merge::Merge + 'static) -> Self {
        self.merger = Box::new(merger);
        self
    }

    /// Register a validation schema for one merged entry name.
    pub fn with_schema(
        mut self,
        name: impl Into<String>,
        schema: impl Into<validate::Value>,
    ) -> Self {
        self.schemas.insert(name.into(), schema.into());
        self
    }

    /// Register validation schemas for several merged entry names at once.
    pub fn with_schemas(mut self, schemas: Schemas) -> Self {
        for (name, schema) in schemas {
            self.schemas.insert(name, schema);
        }
        self
    }

    // ── Pipeline ──────────────────────────────────────────────────────────────

    /// Load raw bytes from all sources using the registered loaders.
    ///
    /// Sources with `ignore_errors` set swallow load failures silently.
    pub fn load(&self) -> Result<Vec<loader::Payload>, Error> {
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
                    if config_source.ignore_errors() {
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

    /// Deserialize loaded bytes into [`parser::LocatedValue`] trees.
    ///
    /// Parser selection: if `payload.maybe_format` is set, the first parser that lists that
    /// format wins; otherwise parsers are probed via `is_format_supported`. Sources
    /// with `ignore_errors` skip payloads that fail to parse.
    pub fn parse(&self, loaded: &[loader::Payload]) -> Result<Vec<Parsed>, Error> {
        let mut result = Vec::new();
        for payload in loaded {
            let config_source = &payload.source;
            let resource = match (&payload.maybe_name, &payload.maybe_format) {
                (Some(name), Some(format)) => format!("{name}.{format}"),
                _ => {
                    let r = config_source.resource();
                    if r.is_empty() {
                        config_source.to_string()
                    } else {
                        r.to_string()
                    }
                }
            };
            let source_name = config_source.source();
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::debug!(msg = "Parsing configuration payload", source = source_name, resource = resource, format = payload.maybe_format.as_deref().unwrap_or("auto"));
                } else if #[cfg(feature = "logging")] {
                    let fmt = payload.maybe_format.as_deref().unwrap_or("auto");
                    log::debug!("msg=\"Parsing configuration payload\" source={source_name} resource={resource} format={fmt}");
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
                    tracing::trace!(msg = "Found parser for configuration payload", parser = parser.name(), resource = resource);
                } else if #[cfg(feature = "logging")] {
                    log::trace!("msg=\"Found parser for configuration payload\" parser={} resource={resource}", parser.name());
                }
            }
            let value = match parser.parse(source_name, &resource, &payload.content) {
                Ok(v) => v,
                Err(e) => {
                    if config_source.ignore_errors() {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::warn!(msg = "Skipped parse error for payload", source = source_display(config_source), resource = resource, error = ?e);
                            } else if #[cfg(feature = "logging")] {
                                let display = source_display(config_source);
                                log::warn!("msg=\"Skipped parse error for payload\" source={display} resource={resource} error={e:?}");
                            }
                        }
                        continue;
                    }
                    return Err(Error::Parse(e));
                }
            };
            result.push((payload.clone(), value));
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

    /// Merge parsed values using the registered merger.
    ///
    /// Payloads with the same `maybe_name` are combined; `None`-named payloads share the `""` key.
    pub fn merge(&self, parsed: &[Parsed]) -> Result<Merged, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Starting configuration merge stage", entry_count = parsed.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Starting configuration merge stage\" entry_count={}", parsed.len());
            }
        }
        match self.merger.merge(parsed) {
            Ok(r) => {
                cfg_if! {
                    if #[cfg(feature = "tracing")] {
                        tracing::info!(msg = "Configuration merge stage complete", group_count = r.len());
                    } else if #[cfg(feature = "logging")] {
                        log::info!("msg=\"Configuration merge stage complete\" group_count={}", r.len());
                    }
                }
                Ok(r)
            }
            Err(e) => Err(Error::Merge(e)),
        }
    }

    /// Validate (and coerce) the merged configuration against the registered schemas.
    ///
    /// Each schema is built into a validator with [`validate::Registry::with_builtins`] and
    /// run against the merged entry of the same name. Validators may coerce values in place
    /// (e.g. a numeric string into an integer), so `merged` is taken by `&mut`. A schema with
    /// no matching merged entry is skipped with a warning. Does nothing when no schemas are
    /// registered.
    pub fn validate(&self, merged: &mut Merged) -> Result<(), Error> {
        if self.schemas.is_empty() {
            return Ok(());
        }
        let registry = validate::Registry::with_builtins();
        for (name, schema) in &self.schemas {
            let validator = match registry.build_value(schema) {
                Ok(validator) => validator,
                Err(source) => {
                    return Err(Error::Schema {
                        name: name.clone(),
                        source,
                    });
                }
            };
            match merged.get_mut(name) {
                Some((_payloads, value)) => match validate::validate(validator.as_ref(), value) {
                    Ok(()) => {}
                    Err(source) => {
                        return Err(Error::Validate {
                            name: name.clone(),
                            source,
                        });
                    }
                },
                None => {
                    cfg_if! {
                        if #[cfg(feature = "tracing")] {
                            tracing::warn!(msg = "Schema has no matching merged entry", name = name);
                        } else if #[cfg(feature = "logging")] {
                            log::warn!("msg=\"Schema has no matching merged entry\" name={name}");
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
        Ok(())
    }

    /// Run load → parse → merge → validate in sequence.
    pub fn run(&self) -> Result<Merged, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Running configuration pipeline", source_count = self.sources.len(), loader_count = self.loaders.len(), parser_count = self.parsers.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Running configuration pipeline\" source_count={} loader_count={} parser_count={}", self.sources.len(), self.loaders.len(), self.parsers.len());
            }
        }
        let loaded = self.load()?;
        let parsed = self.parse(&loaded)?;
        let mut merged = self.merge(&parsed)?;
        self.validate(&mut merged)?;
        Ok(merged)
    }
}
