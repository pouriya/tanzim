//! The multi-configuration pipeline: load, parse, merge, validate.
//!
//! [`Pipeline`] keeps a [`Merged`] map of named entries (`None` = the unnamed bucket). Assemble it
//! with a [`PipelineBuilder`]: [`Pipeline::builder`] starts a simple-fold builder (add sources,
//! optionally pick a merger), while [`Pipeline::from_plan`] starts from an explicit [`MergePlan`]
//! tree. Call [`build`](PipelineBuilder::build) for the runnable [`Pipeline`], or the
//! [`run`](PipelineBuilder::run) / [`try_deserialize`](PipelineBuilder::try_deserialize) shortcuts.
//!
//! The builder is a **typestate** — [`PipelineBuilder<Sources>`](PipelineBuilder) exposes
//! [`with_source`](PipelineBuilder::with_source) / [`with_merger`](PipelineBuilder::with_merger),
//! [`PipelineBuilder<Plan>`](PipelineBuilder) does not — so mixing the two is a compile error.
//!
//! For the common single-configuration case, prefer [`Config`](crate::Config).

use crate::config::{BuilderState, Plan, Sources};
use crate::loader;
use crate::merger::plan::MergePlan;
use crate::merger::{self, Merged};
use crate::parser::{self, Parsed};
use crate::source::{self, Source};
use cfg_if::cfg_if;
use std::marker::PhantomData;
use tanzim_source::{OnError, Stage};

#[cfg(feature = "validate-schema")]
use crate::validator;

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
#[derive(Debug)]
pub enum Error {
    NoLoaders,
    NoParsers,
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
        name: Option<String>,
        inner: validator::SchemaError,
    },
    #[cfg(feature = "validate-schema")]
    Validate {
        name: Option<String>,
        inner: validator::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoLoaders => write!(f, "no loaders registered"),
            Self::NoParsers => write!(f, "no parsers registered"),
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
            Self::Schema { name, inner } => {
                write!(f, "schema for `{name:?}` is invalid: ")?;
                std::fmt::Display::fmt(inner, f)
            }
            #[cfg(feature = "validate-schema")]
            Self::Validate { name, inner } => {
                write!(f, "configuration `{name:?}` failed validation: ")?;
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
            Self::Schema { inner, .. } => Some(inner),
            #[cfg(feature = "validate-schema")]
            Self::Validate { inner, .. } => Some(inner),
            Self::NoLoaders | Self::NoParsers | Self::NoLoader { .. } | Self::NoParser { .. } => {
                None
            }
        }
    }
}

impl From<source::ParseError> for Error {
    fn from(error: source::ParseError) -> Self {
        Error::Source(error)
    }
}

impl From<std::convert::Infallible> for Error {
    fn from(error: std::convert::Infallible) -> Self {
        match error {}
    }
}

/// Assembles a [`Pipeline`] for the multi-configuration pipeline.
///
/// The `State` typestate gates which builder methods exist:
///
/// - [`PipelineBuilder<Sources>`](PipelineBuilder) — from [`Pipeline::builder`]. Simple-fold mode:
///   add sources with [`with_source`](Self::with_source) / [`add_source`](Self::add_source), bind a
///   per-source merger with [`with_source_merged`](Self::with_source_merged), and pick the global
///   merger with [`with_merger`](Self::with_merger) (defaults to [`LastWins`](merger::LastWins)).
/// - [`PipelineBuilder<Plan>`](PipelineBuilder) — from [`Pipeline::from_plan`]. Carries an explicit
///   [`MergePlan`] tree and exposes none of the source builders.
///
/// Finish with [`build`](Self::build) (the runnable [`Pipeline`]) or the [`run`](Self::run) /
/// [`try_deserialize`](Self::try_deserialize) shortcuts.
pub struct PipelineBuilder<State: BuilderState> {
    plan: MergePlan,
    merger_set: bool,
    loaders: Vec<Box<dyn loader::Load + Send + Sync>>,
    parsers: Vec<Box<dyn parser::Parse + Send + Sync>>,
    #[cfg(feature = "validate-schema")]
    schemas: Schemas,
    _state: PhantomData<State>,
}

impl Pipeline {
    /// Start a simple-fold [`PipelineBuilder<Sources>`](PipelineBuilder) with nothing registered — no
    /// loaders, parsers, sources, or merger. Add the feature-enabled defaults with
    /// [`with_default_loaders`](PipelineBuilder::with_default_loaders) /
    /// [`with_default_parsers`](PipelineBuilder::with_default_parsers).
    pub fn builder() -> PipelineBuilder<Sources> {
        PipelineBuilder {
            plan: MergePlan::Merge {
                merger: Box::new(merger::LastWins),
                children: Vec::new(),
            },
            merger_set: false,
            loaders: Vec::new(),
            parsers: Vec::new(),
            #[cfg(feature = "validate-schema")]
            schemas: Schemas::new(),
            _state: PhantomData,
        }
    }

    /// Start a [`PipelineBuilder<Plan>`](PipelineBuilder) from an explicit [`MergePlan`] tree (see
    /// [`Config::from_plan`](crate::Config::from_plan) for the plan constructors). This mode does not
    /// expose the per-source builders — mixing the two is a compile error.
    pub fn from_plan(plan: MergePlan) -> PipelineBuilder<Plan> {
        PipelineBuilder {
            plan,
            merger_set: false,
            loaders: Vec::new(),
            parsers: Vec::new(),
            #[cfg(feature = "validate-schema")]
            schemas: Schemas::new(),
            _state: PhantomData,
        }
    }
}

impl<State: BuilderState> PipelineBuilder<State> {
    /// The configured configuration sources, in declared order.
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        merger::leaves(&self.plan).into_iter()
    }

    pub fn loaders(&self) -> &[Box<dyn loader::Load + Send + Sync>] {
        &self.loaders
    }

    pub fn loaders_mut(&mut self) -> &mut Vec<Box<dyn loader::Load + Send + Sync>> {
        &mut self.loaders
    }

    pub fn parsers(&self) -> &[Box<dyn parser::Parse + Send + Sync>] {
        &self.parsers
    }

    pub fn parsers_mut(&mut self) -> &mut Vec<Box<dyn parser::Parse + Send + Sync>> {
        &mut self.parsers
    }

    /// The global merger chosen via [`with_merger`](Self::with_merger), if any. `None` when merging
    /// falls back to [`LastWins`](merger::LastWins), or in [`Plan`] mode.
    pub fn merger(&self) -> Option<&dyn merger::Merge> {
        self.merger_set
            .then(|| merger::root_merger(&self.plan))
            .flatten()
    }

    #[cfg(feature = "validate-schema")]
    pub fn schemas(&self) -> &Schemas {
        &self.schemas
    }

    #[cfg(feature = "validate-schema")]
    pub fn schemas_mut(&mut self) -> &mut Schemas {
        &mut self.schemas
    }

    pub fn with_loader(mut self, loader: impl loader::Load + Send + Sync + 'static) -> Self {
        self.loaders.push(Box::new(loader));
        self
    }

    pub fn with_parser(mut self, parser: impl parser::Parse + Send + Sync + 'static) -> Self {
        self.parsers.push(Box::new(parser));
        self
    }

    /// Append the feature-enabled default loaders (`env`, `file`; `http` needs a user closure and is
    /// not auto-included).
    #[allow(unused_mut)]
    pub fn with_default_loaders(mut self) -> Self {
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
        self
    }

    /// Replace the registered loaders with the feature-enabled defaults.
    pub fn set_default_loaders(mut self) -> Self {
        self.loaders.clear();
        self.with_default_loaders()
    }

    /// Append the feature-enabled default parsers (`env`, `json`, `yaml`, `toml`).
    #[allow(unused_mut)]
    pub fn with_default_parsers(mut self) -> Self {
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

    /// Replace the registered parsers with the feature-enabled defaults.
    pub fn set_default_parsers(mut self) -> Self {
        self.parsers.clear();
        self.with_default_parsers()
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

    /// Finish assembling and produce the runnable [`Pipeline`].
    pub fn build(self) -> Pipeline {
        Pipeline {
            plan: self.plan,
            merger_set: self.merger_set,
            loaders: self.loaders,
            parsers: self.parsers,
            #[cfg(feature = "validate-schema")]
            schemas: self.schemas,
        }
    }

    /// Shortcut for [`build`](Self::build) then [`Pipeline::run`].
    pub fn run(self) -> Result<Merged, Error> {
        self.build().run()
    }

    /// Shortcut for [`build`](Self::build) then [`Pipeline::try_deserialize`].
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(
        self,
    ) -> Result<std::collections::HashMap<Option<String>, T>, Error> {
        self.build().try_deserialize()
    }
}

impl PipelineBuilder<Sources> {
    /// Append a configuration source (builder-style). `source` may be a [`Source`] or any string form
    /// (e.g. `"file:app.toml"`), parsed now — an invalid source yields [`Error::Source`].
    pub fn with_source<S>(mut self, source: S) -> Result<Self, Error>
    where
        S: TryInto<Source>,
        Error: From<S::Error>,
    {
        self.add_source(source)?;
        Ok(self)
    }

    /// Append a configuration source (in-place). See [`with_source`](Self::with_source).
    pub fn add_source<S>(&mut self, source: S) -> Result<&mut Self, Error>
    where
        S: TryInto<Source>,
        Error: From<S::Error>,
    {
        let source = source.try_into()?;
        merger::push_child(&mut self.plan, MergePlan::Source(source));
        Ok(self)
    }

    /// Append a source bound to its own `merger`, applied to that source's payloads before the
    /// global merger folds everything (builder-style).
    pub fn with_source_merged<S>(
        mut self,
        source: S,
        merger: impl merger::Merge + Send + Sync + 'static,
    ) -> Result<Self, Error>
    where
        S: TryInto<Source>,
        Error: From<S::Error>,
    {
        let source = source.try_into()?;
        merger::push_child(
            &mut self.plan,
            MergePlan::Merge {
                merger: Box::new(merger),
                children: vec![MergePlan::Source(source)],
            },
        );
        Ok(self)
    }

    /// Set the global merger that folds all sources together. Defaults to
    /// [`LastWins`](merger::LastWins) when never set.
    pub fn with_merger(mut self, merger: impl merger::Merge + Send + Sync + 'static) -> Self {
        merger::set_root_merger(&mut self.plan, Box::new(merger));
        self.merger_set = true;
        self
    }
}

/// Runs the load → parse → merge → validate pipeline for multiple named configuration entries.
///
/// Assemble one with [`Pipeline::builder`] (simple-fold) or [`Pipeline::from_plan`] (explicit tree),
/// then [`build`](PipelineBuilder::build). Call [`run`](Self::run) /
/// [`try_deserialize`](Self::try_deserialize) for the whole pipeline, or reach the individual stages
/// through [`stages`](Self::stages).
pub struct Pipeline {
    plan: MergePlan,
    merger_set: bool,
    loaders: Vec<Box<dyn loader::Load + Send + Sync>>,
    parsers: Vec<Box<dyn parser::Parse + Send + Sync>>,
    #[cfg(feature = "validate-schema")]
    schemas: Schemas,
}

impl Pipeline {
    /// The configured configuration sources, in declared order.
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        merger::leaves(&self.plan).into_iter()
    }

    pub fn loaders(&self) -> &[Box<dyn loader::Load + Send + Sync>] {
        &self.loaders
    }

    pub fn parsers(&self) -> &[Box<dyn parser::Parse + Send + Sync>] {
        &self.parsers
    }

    /// The global merger chosen via [`PipelineBuilder::with_merger`], if any. `None` when merging
    /// falls back to [`LastWins`](merger::LastWins), or for a [`from_plan`](Self::from_plan) tree.
    pub fn merger(&self) -> Option<&dyn merger::Merge> {
        self.merger_set
            .then(|| merger::root_merger(&self.plan))
            .flatten()
    }

    #[cfg(feature = "validate-schema")]
    pub fn schemas(&self) -> &Schemas {
        &self.schemas
    }

    /// The individual pipeline stages (`load` → `parse` → `merge` → `validate`), for running or
    /// inspecting them one at a time. The terminal [`run`](Self::run) /
    /// [`try_deserialize`](Self::try_deserialize) shortcuts cover the common path.
    pub fn stages(&self) -> PipelineStages<'_> {
        PipelineStages { pipeline: self }
    }

    /// Run load → parse → merge → validate in sequence.
    pub fn run(&self) -> Result<Merged, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Running multi configuration pipeline", source_count = merger::leaves(&self.plan).len(), loader_count = self.loaders.len(), parser_count = self.parsers.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Running multi configuration pipeline\" source_count={} loader_count={} parser_count={}", merger::leaves(&self.plan).len(), self.loaders.len(), self.parsers.len());
            }
        }
        let stages = self.stages();
        let loaded = stages.load()?;
        let parsed = stages.parse(&loaded)?;
        let mut merged = stages.merge(&parsed)?;
        stages.validate(&mut merged)?;
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
                    return Err(Error::Deserialize(error));
                }
            };
            out.insert(name.clone(), deserialized);
        }
        Ok(out)
    }
}

/// The pipeline stages of a [`Pipeline`], reached through [`Pipeline::stages`].
///
/// Each method runs one stage in isolation so callers can inspect intermediate results:
/// [`load`](Self::load) → [`parse`](Self::parse) → [`merge`](Self::merge) → [`validate`](Self::validate).
pub struct PipelineStages<'a> {
    pipeline: &'a Pipeline,
}

impl PipelineStages<'_> {
    pub fn load(&self) -> Result<Vec<loader::Payload>, Error> {
        let pipeline = self.pipeline;
        if pipeline.loaders.is_empty() {
            return Err(Error::NoLoaders);
        }
        let mut result = Vec::new();
        for config_source in merger::leaves(&pipeline.plan) {
            let source_name = config_source.source();
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::debug!(msg = "Loading configuration source", source = source_name, resource = config_source.resource());
                } else if #[cfg(feature = "logging")] {
                    log::debug!("msg=\"Loading configuration source\" source={source_name} resource={}", config_source.resource());
                }
            }
            let mut found_loader = None;
            for loader in &pipeline.loaders {
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
        let pipeline = self.pipeline;
        if pipeline.parsers.is_empty() {
            return Err(Error::NoParsers);
        }
        let all_sources: Vec<Source> = merger::leaves(&pipeline.plan)
            .into_iter()
            .cloned()
            .collect();
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
                for parser in &pipeline.parsers {
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
                for parser in &pipeline.parsers {
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
            let value = match parser.parse(&payload.source, &payload.content, &all_sources) {
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

    /// Merge the parsed payloads into named groups.
    ///
    /// Evaluates the configured [`MergePlan`]: the simple builders fold every source (in declared
    /// order) with the global merger, defaulting to [`LastWins`](merger::LastWins), each per-source
    /// merger pre-merging its own payloads first; a [`from_plan`](Pipeline::from_plan) tree evaluates
    /// directly.
    pub fn merge(&self, parsed: &[Parsed]) -> Result<Merged, Error> {
        let pipeline = self.pipeline;
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Starting configuration merge stage", entry_count = parsed.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Starting configuration merge stage\" entry_count={}", parsed.len());
            }
        }
        let groups = merger::group_by_source(&merger::leaves(&pipeline.plan), parsed);
        match merger::plan::evaluate(&pipeline.plan, &groups) {
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
            let schemas = &self.pipeline.schemas;
            if schemas.is_empty() {
                return Ok(());
            }
            let registry = validator::Registry::with_builtins();
            for (name, schema) in schemas {
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
                    tracing::info!(msg = "Configuration validation stage complete", schema_count = self.pipeline.schemas.len());
                } else if #[cfg(feature = "logging")] {
                    log::info!("msg=\"Configuration validation stage complete\" schema_count={}", self.pipeline.schemas.len());
                }
            }
        }
        Ok(())
    }
}
