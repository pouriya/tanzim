//! Single-configuration pipeline: load, parse, merge, unify, validate.
//!
//! [`Single`] collapses every source into one unified configuration value. Everything needed to
//! build a pipeline is re-exported here, so `use tanzim::pipeline::single::*;` is enough on its own.

use super::{Entry, Merged, Parsed};
use crate::source;
use cfg_if::cfg_if;
use tanzim_source::{OnError, Stage};

pub use crate::merger::{self, DeepMerge, LastWins, Merge};
pub use crate::source::Source;
#[cfg(feature = "validate-schema")]
pub use crate::validator;
pub use crate::{loader, parser};

fn source_display(cs: &Source) -> String {
    let mut s = cs.source().to_string();
    if cs.resource_colon() || !cs.resource().is_empty() {
        s.push(':');
        s.push_str(cs.resource());
    }
    s
}

/// Errors produced by the single-configuration pipeline.
#[derive(Debug)]
pub enum Error {
    NoLoaders,
    NoParsers,
    NoMerger,
    Source(source::ParseError),
    Load(loader::Error),
    Parse(tanzim_value::Error),
    Merge(merger::Error),
    Deserialize(tanzim_value::Error),
    NoLoader {
        at: String,
    },
    NoParser {
        format: String,
        at: String,
    },

    #[cfg(feature = "validate-schema")]
    Schema {
        inner: validator::SchemaError,
    },
    #[cfg(feature = "validate-schema")]
    Validate {
        inner: validator::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoLoaders => write!(f, "no loaders registered"),
            Self::NoParsers => write!(f, "no parsers registered"),
            Self::NoMerger => write!(f, "no merger registered"),
            // Transparent: forward Display (and its alternate form, so `{error:#}` reaches the
            // wrapped error's source snippet / caret) to the wrapped error.
            Self::Source(error) => std::fmt::Display::fmt(error, f),
            Self::Load(error) => std::fmt::Display::fmt(error, f),
            Self::Parse(error) => std::fmt::Display::fmt(error, f),
            Self::Merge(error) => std::fmt::Display::fmt(error, f),
            Self::Deserialize(error) => std::fmt::Display::fmt(error, f),
            Self::NoLoader { at } => write!(f, "no loader found for `{at}`"),
            Self::NoParser { format, at } => {
                write!(f, "no parser found for format `{format}` in `{at}`")
            }
            #[cfg(feature = "validate-schema")]
            Self::Schema { inner } => {
                write!(f, "schema is invalid: ")?;
                std::fmt::Display::fmt(inner, f)
            }
            #[cfg(feature = "validate-schema")]
            Self::Validate { inner } => {
                write!(f, "configuration failed validation: ")?;
                std::fmt::Display::fmt(inner, f)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Load(error) => Some(error),
            Self::Parse(error) | Self::Deserialize(error) => Some(error),
            Self::Merge(error) => Some(error),
            #[cfg(feature = "validate-schema")]
            Self::Schema { inner } => Some(inner),
            #[cfg(feature = "validate-schema")]
            Self::Validate { inner } => Some(inner),
            Self::NoLoaders
            | Self::NoParsers
            | Self::NoMerger
            | Self::NoLoader { .. }
            | Self::NoParser { .. } => None,
        }
    }
}

/// Runs the load → parse → merge → unify → validate pipeline for a single configuration value.
///
/// Construct with [`Single::default`] (all feature-enabled loaders + parsers, no merger) or
/// [`Single::empty`] (nothing registered). There is no `new()`. Add a merger with
/// [`with_merger`](Self::with_merger) and sources with [`with_source`](Self::with_source) /
/// [`add_source`](Self::add_source), then call [`run`](Self::run) or
/// [`try_deserialize`](Self::try_deserialize).
pub struct Single {
    sources: Vec<Source>,
    loaders: Vec<Box<dyn loader::Load>>,
    parsers: Vec<Box<dyn parser::Parse>>,
    merger: Option<Box<dyn merger::Merge>>,
    #[cfg(feature = "validate-schema")]
    schema: Option<validator::Value>,
}

impl Default for Single {
    /// All feature-enabled loaders and parsers, but no merger and no sources.
    fn default() -> Self {
        Self::empty()
            .with_included_loaders()
            .with_included_parsers()
    }
}

impl Single {
    /// An empty pipeline: no loaders, parsers, merger, or sources.
    pub fn empty() -> Self {
        Self {
            sources: Vec::new(),
            loaders: Vec::new(),
            parsers: Vec::new(),
            merger: None,
            #[cfg(feature = "validate-schema")]
            schema: None,
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
    pub fn schema(&self) -> Option<&validator::Value> {
        self.schema.as_ref()
    }

    #[cfg(feature = "validate-schema")]
    pub fn schema_mut(&mut self) -> &mut Option<validator::Value> {
        &mut self.schema
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
    pub fn with_schema(mut self, schema: impl Into<validator::Value>) -> Self {
        self.schema = Some(schema.into());
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

    /// Collapse all merge groups into one [`Entry`].
    ///
    /// Collects named groups (sorted alphabetically by name), then the unnamed group (key
    /// `None`), in that order. For each group, synthesises one `(Payload, LocatedValue)` pair
    /// — using the group's first payload with `maybe_name` set to `None` — so the configured
    /// merger sees all pairs as belonging to the same unnamed group and collapses them into a
    /// single entry. This reuses the user's merger for cross-group unification without adding
    /// a new abstraction.
    ///
    /// **Provenance note:** the returned [`Entry`]'s payloads contain one synthetic carrier per
    /// group (derived from each group's first payload), not the full list of contributing
    /// payloads from within-group merging. Callers who need complete provenance should call
    /// [`merge`](Self::merge) directly and inspect the [`Merged`] map.
    ///
    /// **`LastWins` note:** with `LastWins` as the configured merger, the cross-group pass
    /// keeps only the last group's value. Groups are ordered named-alphabetical then unnamed,
    /// so the unnamed bucket wins when present.
    pub fn unify(&self, merged: &Merged) -> Result<Entry, Error> {
        let merger = match &self.merger {
            Some(merger) => merger,
            None => return Err(Error::NoMerger),
        };
        let mut named_keys: Vec<String> = Vec::new();
        for name in merged.keys().flatten() {
            named_keys.push(name.clone());
        }
        named_keys.sort();

        let mut flat: Vec<(loader::Payload, parser::LocatedValue)> = Vec::new();
        for name in &named_keys {
            if let Some(entry) = merged.get(&Some(name.clone()))
                && let Some(payload) = entry.payloads().first()
            {
                let mut synthetic = payload.clone();
                synthetic.maybe_name = None;
                flat.push((synthetic, entry.value().clone()));
            }
        }
        if let Some(entry) = merged.get(&None)
            && let Some(payload) = entry.payloads().first()
        {
            let mut synthetic = payload.clone();
            synthetic.maybe_name = None;
            flat.push((synthetic, entry.value().clone()));
        }

        if flat.is_empty() {
            return Ok(Entry::new(
                Vec::new(),
                parser::LocatedValue::new(
                    tanzim_value::Value::Map(tanzim_value::Map::new()),
                    tanzim_value::Location::at("", "", None, None, None),
                ),
            ));
        }

        let mut unified = match merger.merge(&flat) {
            Ok(r) => r,
            Err(e) => return Err(Error::Merge(e)),
        };

        let result = match unified.remove(&None) {
            Some((payloads, value)) => Entry::new(payloads, value),
            None => Entry::new(
                Vec::new(),
                parser::LocatedValue::new(
                    tanzim_value::Value::Map(tanzim_value::Map::new()),
                    tanzim_value::Location::at("", "", None, None, None),
                ),
            ),
        };
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::info!(msg = "Configuration unify stage complete", group_count = named_keys.len() + 1);
            } else if #[cfg(feature = "logging")] {
                log::info!("msg=\"Configuration unify stage complete\" group_count={}", named_keys.len() + 1);
            }
        }
        Ok(result)
    }

    /// Validate (and coerce) the unified configuration against the registered schema.
    pub fn validate(&self, _value: &mut parser::LocatedValue) -> Result<(), Error> {
        #[cfg(feature = "validate-schema")]
        {
            let Some(schema) = &self.schema else {
                return Ok(());
            };
            let registry = validator::Registry::with_builtins();
            let validator = match registry.build_value(schema) {
                Ok(validator) => validator,
                Err(inner) => {
                    return Err(Error::Schema { inner });
                }
            };
            match validator::validate(validator.as_ref(), _value) {
                Ok(()) => {}
                Err(inner) => {
                    return Err(Error::Validate { inner });
                }
            }
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::info!(msg = "Configuration validation stage complete");
                } else if #[cfg(feature = "logging")] {
                    log::info!("msg=\"Configuration validation stage complete\"");
                }
            }
        }
        Ok(())
    }

    /// Run load → parse → merge → unify → validate in sequence.
    pub fn run(&self) -> Result<Entry, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Running single configuration pipeline", source_count = self.sources.len(), loader_count = self.loaders.len(), parser_count = self.parsers.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Running single configuration pipeline\" source_count={} loader_count={} parser_count={}", self.sources.len(), self.loaders.len(), self.parsers.len());
            }
        }
        let loaded = self.load()?;
        let parsed = self.parse(&loaded)?;
        let merged = self.merge(&parsed)?;
        let mut entry = self.unify(&merged)?;
        self.validate(entry.value_mut())?;
        Ok(entry)
    }

    /// Run the pipeline and deserialize the unified configuration into `T`. A type mismatch
    /// yields [`Error::Deserialize`] pointing at the offending value's source location; format
    /// it with `{error:#}` for a source snippet with a caret underline.
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<T, Error> {
        let entry = self.run()?;
        match entry.value().try_deserialize::<T>() {
            Ok(value) => Ok(value),
            Err(error) => Err(Error::Deserialize(error)),
        }
    }
}
