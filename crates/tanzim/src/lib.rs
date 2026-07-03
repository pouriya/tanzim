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
//! - [`parser`] — [`tanzim_parse`] ([`tanzim_parse::Parse`])
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

/// Single-configuration pipeline: load, parse, merge, unify, validate.
pub mod single {
    #[cfg(feature = "validate-schema")]
    use crate::validate;
    use crate::{loader, merge, parser, source};
    use cfg_if::cfg_if;
    use tanzim_source::{OnError, Source, Stage};

    /// A loaded payload paired with the value tree produced by parsing it.
    pub type Parsed = (loader::Payload, parser::LocatedValue);

    /// Merged configuration keyed by entry name.
    pub type Merged = merge::Merged;

    fn source_display(cs: &Source) -> String {
        let mut s = cs.source().to_string();
        if cs.resource_colon() || !cs.resource().is_empty() {
            s.push(':');
            s.push_str(cs.resource());
        }
        s
    }

    /// Errors produced by the single-configuration pipeline.
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
        Merge(merge::Error),
        #[error("no loader found for `{at}`")]
        NoLoader { at: String },
        #[error("no parser found for format `{format}` in `{at}`")]
        NoParser { format: String, at: String },

        #[cfg(feature = "validate-schema")]
        #[error("schema is invalid: {inner}")]
        Schema { inner: validate::SchemaError },
        #[cfg(feature = "validate-schema")]
        #[error("configuration failed validation: {inner}")]
        Validate { inner: validate::Error },
    }

    /// Builds a [`PipelineSingle`] with a fluent API.
    pub struct PipelineSingleBuilder {
        sources: Vec<Source>,
        loaders: Vec<Box<dyn loader::Load>>,
        parsers: Vec<Box<dyn parser::Parse>>,
        merger: Option<Box<dyn merge::Merge>>,
        #[cfg(feature = "validate-schema")]
        schema: Option<validate::Value>,
    }

    impl Default for PipelineSingleBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    impl PipelineSingleBuilder {
        pub fn new() -> Self {
            Self {
                sources: Vec::new(),
                loaders: Vec::new(),
                parsers: Vec::new(),
                merger: None,
                #[cfg(feature = "validate-schema")]
                schema: None,
            }
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

        pub fn with_parser(mut self, parser: impl parser::Parse + 'static) -> Self {
            self.parsers.push(Box::new(parser));
            self
        }

        pub fn with_merger(mut self, merger: impl merge::Merge + 'static) -> Self {
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
        pub fn with_schema(mut self, schema: impl Into<validate::Value>) -> Self {
            self.schema = Some(schema.into());
            self
        }

        pub fn build(self) -> Result<PipelineSingle, Error> {
            if self.loaders.is_empty() {
                return Err(Error::NoLoaders);
            }
            if self.parsers.is_empty() {
                return Err(Error::NoParsers);
            }
            let merger = match self.merger {
                Some(merger) => merger,
                None => return Err(Error::NoMerger),
            };
            Ok(PipelineSingle {
                sources: self.sources,
                loaders: self.loaders,
                parsers: self.parsers,
                merger,
                #[cfg(feature = "validate-schema")]
                schema: self.schema,
            })
        }
    }

    /// Runs the load → parse → merge → unify pipeline for a single configuration value.
    pub struct PipelineSingle {
        sources: Vec<Source>,
        loaders: Vec<Box<dyn loader::Load>>,
        parsers: Vec<Box<dyn parser::Parse>>,
        merger: Box<dyn merge::Merge>,
        #[cfg(feature = "validate-schema")]
        schema: Option<validate::Value>,
    }

    impl PipelineSingle {
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

        pub fn merger(&self) -> &dyn merge::Merge {
            &*self.merger
        }

        pub fn merger_mut(&mut self) -> &mut Box<dyn merge::Merge> {
            &mut self.merger
        }

        #[cfg(feature = "validate-schema")]
        pub fn schema(&self) -> Option<&validate::Value> {
            self.schema.as_ref()
        }

        #[cfg(feature = "validate-schema")]
        pub fn schema_mut(&mut self) -> &mut Option<validate::Value> {
            &mut self.schema
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

        pub fn with_parser(mut self, parser: impl parser::Parse + 'static) -> Self {
            self.parsers.push(Box::new(parser));
            self
        }

        pub fn with_merger(mut self, merger: impl merge::Merge + 'static) -> Self {
            self.merger = Box::new(merger);
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
        pub fn with_schema(mut self, schema: impl Into<validate::Value>) -> Self {
            self.schema = Some(schema.into());
            self
        }

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
                let value = match parser.parse(&payload.source, &payload.content) {
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

        /// Collapse all merge groups into one value.
        ///
        /// Collects named groups (sorted alphabetically by name), then the unnamed group (key
        /// `None`), in that order. For each group, synthesises one `(Payload, LocatedValue)` pair
        /// — using the group's first payload with `maybe_name` set to `None` — so the configured
        /// merger sees all pairs as belonging to the same unnamed group and collapses them into a
        /// single entry. This reuses the user's merger for cross-group unification without adding
        /// a new abstraction.
        ///
        /// **Provenance note:** the returned `Vec<Payload>` contains one synthetic carrier per
        /// group (derived from each group's first payload), not the full list of contributing
        /// payloads from within-group merging. Callers who need complete provenance should call
        /// [`merge`](Self::merge) directly and inspect the `Merged` map.
        ///
        /// **`LastWins` note:** with `LastWins` as the configured merger, the cross-group pass
        /// keeps only the last group's value. Groups are ordered named-alphabetical then unnamed,
        /// so the unnamed bucket wins when present.
        pub fn unify(
            &self,
            merged: &Merged,
        ) -> Result<(Vec<loader::Payload>, parser::LocatedValue), Error> {
            let mut named_keys: Vec<String> = Vec::new();
            for name in merged.keys().flatten() {
                named_keys.push(name.clone());
            }
            named_keys.sort();

            let mut flat: Vec<(loader::Payload, parser::LocatedValue)> = Vec::new();
            for name in &named_keys {
                if let Some((payloads, lv)) = merged.get(&Some(name.clone()))
                    && let Some(payload) = payloads.first()
                {
                    let mut synthetic = payload.clone();
                    synthetic.maybe_name = None;
                    flat.push((synthetic, lv.clone()));
                }
            }
            if let Some((payloads, lv)) = merged.get(&None)
                && let Some(payload) = payloads.first()
            {
                let mut synthetic = payload.clone();
                synthetic.maybe_name = None;
                flat.push((synthetic, lv.clone()));
            }

            if flat.is_empty() {
                return Ok((
                    Vec::new(),
                    parser::LocatedValue {
                        value: tanzim_value::Value::Map(tanzim_value::Map::new()),
                        location: tanzim_value::Location::at("", "", None, None, None),
                    },
                ));
            }

            let mut unified = match self.merger.merge(&flat) {
                Ok(r) => r,
                Err(e) => return Err(Error::Merge(e)),
            };

            let result = match unified.remove(&None) {
                Some(r) => r,
                None => (
                    Vec::new(),
                    parser::LocatedValue {
                        value: tanzim_value::Value::Map(tanzim_value::Map::new()),
                        location: tanzim_value::Location::at("", "", None, None, None),
                    },
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
                let registry = validate::Registry::with_builtins();
                let validator = match registry.build_value(schema) {
                    Ok(validator) => validator,
                    Err(inner) => {
                        return Err(Error::Schema { inner });
                    }
                };
                match validate::validate(validator.as_ref(), _value) {
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
        pub fn run(&self) -> Result<(Vec<loader::Payload>, parser::LocatedValue), Error> {
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
            let (payloads, mut value) = self.unify(&merged)?;
            self.validate(&mut value)?;
            Ok((payloads, value))
        }
    }
}

/// Multi-configuration pipeline: load, parse, merge, validate.
pub mod multi {
    #[cfg(feature = "validate-schema")]
    use crate::validate;
    use crate::{loader, merge, parser, source};
    use cfg_if::cfg_if;
    use tanzim_source::{OnError, Source, Stage};

    /// A loaded payload paired with the value tree produced by parsing it.
    pub type Parsed = (loader::Payload, parser::LocatedValue);

    /// Merged configuration keyed by entry name.
    pub type Merged = merge::Merged;

    /// Validation schemas keyed by merged entry name.
    #[cfg(feature = "validate-schema")]
    pub type Schemas = std::collections::HashMap<Option<String>, validate::Value>;

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
        Merge(merge::Error),
        #[error("no loader found for `{at}`")]
        NoLoader { at: String },
        #[error("no parser found for format `{format}` in `{at}`")]
        NoParser { format: String, at: String },

        #[cfg(feature = "validate-schema")]
        #[error("schema for `{name:?}` is invalid: {inner}")]
        Schema {
            name: Option<String>,
            inner: validate::SchemaError,
        },
        #[cfg(feature = "validate-schema")]
        #[error("configuration `{name:?}` failed validation: {inner}")]
        Validate {
            name: Option<String>,
            inner: validate::Error,
        },
    }

    /// Builds a [`PipelineMulti`] with a fluent API.
    pub struct PipelineMultiBuilder {
        sources: Vec<Source>,
        loaders: Vec<Box<dyn loader::Load>>,
        parsers: Vec<Box<dyn parser::Parse>>,
        merger: Option<Box<dyn merge::Merge>>,
        #[cfg(feature = "validate-schema")]
        schemas: Schemas,
    }

    impl Default for PipelineMultiBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    impl PipelineMultiBuilder {
        pub fn new() -> Self {
            Self {
                sources: Vec::new(),
                loaders: Vec::new(),
                parsers: Vec::new(),
                merger: None,
                #[cfg(feature = "validate-schema")]
                schemas: Schemas::new(),
            }
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

        pub fn with_parser(mut self, parser: impl parser::Parse + 'static) -> Self {
            self.parsers.push(Box::new(parser));
            self
        }

        pub fn with_merger(mut self, merger: impl merge::Merge + 'static) -> Self {
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
            schema: impl Into<validate::Value>,
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

        pub fn build(self) -> Result<PipelineMulti, Error> {
            if self.loaders.is_empty() {
                return Err(Error::NoLoaders);
            }
            if self.parsers.is_empty() {
                return Err(Error::NoParsers);
            }
            let merger = match self.merger {
                Some(merger) => merger,
                None => return Err(Error::NoMerger),
            };
            Ok(PipelineMulti {
                sources: self.sources,
                loaders: self.loaders,
                parsers: self.parsers,
                merger,
                #[cfg(feature = "validate-schema")]
                schemas: self.schemas,
            })
        }
    }

    /// Runs the load → parse → merge pipeline for multiple named configuration entries.
    pub struct PipelineMulti {
        sources: Vec<Source>,
        loaders: Vec<Box<dyn loader::Load>>,
        parsers: Vec<Box<dyn parser::Parse>>,
        merger: Box<dyn merge::Merge>,
        #[cfg(feature = "validate-schema")]
        schemas: Schemas,
    }

    impl PipelineMulti {
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

        pub fn merger(&self) -> &dyn merge::Merge {
            &*self.merger
        }

        pub fn merger_mut(&mut self) -> &mut Box<dyn merge::Merge> {
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

        pub fn with_parser(mut self, parser: impl parser::Parse + 'static) -> Self {
            self.parsers.push(Box::new(parser));
            self
        }

        pub fn with_merger(mut self, merger: impl merge::Merge + 'static) -> Self {
            self.merger = Box::new(merger);
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
            schema: impl Into<validate::Value>,
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
                let value = match parser.parse(&payload.source, &payload.content) {
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
        pub fn validate(&self, _merged: &mut Merged) -> Result<(), Error> {
            #[cfg(feature = "validate-schema")]
            {
                if self.schemas.is_empty() {
                    return Ok(());
                }
                let registry = validate::Registry::with_builtins();
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
                        Some((_payloads, value)) => {
                            match validate::validate(validator.as_ref(), value) {
                                Ok(()) => {}
                                Err(inner) => {
                                    return Err(Error::Validate {
                                        name: name.clone(),
                                        inner,
                                    });
                                }
                            }
                        }
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
    }
}
