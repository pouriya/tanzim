//! The single-configuration pipeline: load, parse, merge, unify, validate.
//!
//! [`Config`] collapses every source into one unified configuration value. Assemble it with a
//! [`ConfigBuilder`]: [`Config::builder`] starts a simple-fold builder (add sources, optionally pick
//! a merger), while [`Config::from_plan`] starts from an explicit [`MergePlan`] tree. Call
//! [`build`](ConfigBuilder::build) for the runnable [`Config`], or the [`run`](ConfigBuilder::run) /
//! [`try_deserialize`](ConfigBuilder::try_deserialize) shortcuts.
//!
//! The builder is a **typestate**: [`ConfigBuilder<Sources>`](ConfigBuilder) (from
//! [`builder`](Config::builder)) exposes [`with_source`](ConfigBuilder::with_source) /
//! [`with_merger`](ConfigBuilder::with_merger); [`ConfigBuilder<Plan>`](ConfigBuilder) (from
//! [`from_plan`](Config::from_plan)) does not. Mixing the two is a compile error, so there is no
//! runtime "plan conflict" to guard against.

use crate::entry::Entry;
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

mod sealed {
    pub trait Sealed {}
}

/// Builder typestate marker: **simple-fold** mode. A [`ConfigBuilder<Sources>`](ConfigBuilder)
/// exposes [`with_source`](ConfigBuilder::with_source) / [`add_source`](ConfigBuilder::add_source) /
/// [`with_source_merged`](ConfigBuilder::with_source_merged) / [`with_merger`](ConfigBuilder::with_merger).
pub struct Sources;

/// Builder typestate marker: **explicit merge-plan** mode. A [`ConfigBuilder<Plan>`](ConfigBuilder)
/// carries the [`MergePlan`] handed to [`Config::from_plan`] and exposes none of the source builders.
pub struct Plan;

impl sealed::Sealed for Sources {}
impl sealed::Sealed for Plan {}

/// The typestate parameter of [`ConfigBuilder`] — either [`Sources`] or [`Plan`]. Sealed: the crate
/// defines the only two modes.
pub trait BuilderState: sealed::Sealed {}
impl BuilderState for Sources {}
impl BuilderState for Plan {}

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

/// Assembles a [`Config`] for the single-configuration pipeline.
///
/// The `State` typestate gates which builder methods exist:
///
/// - [`ConfigBuilder<Sources>`](ConfigBuilder) — from [`Config::builder`]. Simple-fold mode: add
///   sources with [`with_source`](Self::with_source) / [`add_source`](Self::add_source), bind a
///   per-source merger with [`with_source_merged`](Self::with_source_merged), and pick the global
///   merger with [`with_merger`](Self::with_merger) (defaults to [`LastWins`](merger::LastWins)).
/// - [`ConfigBuilder<Plan>`](ConfigBuilder) — from [`Config::from_plan`]. Carries an explicit
///   [`MergePlan`] tree and exposes none of the source builders.
///
/// Loaders, parsers, and the schema are registered in either mode. Finish with [`build`](Self::build)
/// (the runnable [`Config`]) or the [`run`](Self::run) / [`try_deserialize`](Self::try_deserialize)
/// shortcuts.
pub struct ConfigBuilder<State: BuilderState> {
    plan: MergePlan,
    merger_set: bool,
    loaders: Vec<Box<dyn loader::Load + Send + Sync>>,
    parsers: Vec<Box<dyn parser::Parse + Send + Sync>>,
    #[cfg(feature = "validate-schema")]
    schema: Option<validator::Value>,
    _state: PhantomData<State>,
}

impl Config {
    /// Start a simple-fold [`ConfigBuilder<Sources>`](ConfigBuilder) with nothing registered — no
    /// loaders, parsers, sources, or merger. Add the feature-enabled defaults with
    /// [`with_default_loaders`](ConfigBuilder::with_default_loaders) /
    /// [`with_default_parsers`](ConfigBuilder::with_default_parsers).
    pub fn builder() -> ConfigBuilder<Sources> {
        ConfigBuilder {
            plan: MergePlan::Merge {
                merger: Box::new(merger::LastWins),
                children: Vec::new(),
            },
            merger_set: false,
            loaders: Vec::new(),
            parsers: Vec::new(),
            #[cfg(feature = "validate-schema")]
            schema: None,
            _state: PhantomData,
        }
    }

    /// Start a [`ConfigBuilder<Plan>`](ConfigBuilder) from an explicit [`MergePlan`] tree. Build the
    /// tree with the [`plan`](merger::plan) constructors ([`src`](merger::plan::src),
    /// [`deep`](merger::plan::deep), [`last_wins`](merger::plan::last_wins),
    /// [`merge_with`](merger::plan::merge_with)) for arbitrary folds such as
    /// `last_wins(vec![deep(vec![a, b]), c])`. The plan's [`Source`] leaves become the sources to
    /// load. This mode does not expose the per-source builders — mixing the two is a compile error.
    ///
    /// A [`Plan`]-mode builder has no `with_source` (the plan is the source list):
    ///
    /// ```compile_fail
    /// use tanzim::Config;
    /// use tanzim::merger::plan::{last_wins, src};
    /// Config::from_plan(last_wins(vec![src("mock:a").unwrap()]))
    ///     .with_source("mock:b"); // no such method in `Plan` mode
    /// ```
    ///
    /// …and no `with_merger` (the plan carries its own mergers):
    ///
    /// ```compile_fail
    /// use tanzim::Config;
    /// use tanzim::merger::{LastWins, plan::{last_wins, src}};
    /// Config::from_plan(last_wins(vec![src("mock:a").unwrap()]))
    ///     .with_merger(LastWins); // no such method in `Plan` mode
    /// ```
    pub fn from_plan(plan: MergePlan) -> ConfigBuilder<Plan> {
        ConfigBuilder {
            plan,
            merger_set: false,
            loaders: Vec::new(),
            parsers: Vec::new(),
            #[cfg(feature = "validate-schema")]
            schema: None,
            _state: PhantomData,
        }
    }
}

impl<State: BuilderState> ConfigBuilder<State> {
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

    /// The global merger chosen via [`with_merger`](ConfigBuilder::with_merger), if any. `None` when
    /// merging falls back to [`LastWins`](merger::LastWins), or in [`Plan`] mode (an explicit tree
    /// has no single global merger).
    pub fn merger(&self) -> Option<&dyn merger::Merge> {
        self.merger_set
            .then(|| merger::root_merger(&self.plan))
            .flatten()
    }

    #[cfg(feature = "validate-schema")]
    pub fn schema(&self) -> Option<&validator::Value> {
        self.schema.as_ref()
    }

    #[cfg(feature = "validate-schema")]
    pub fn schema_mut(&mut self) -> &mut Option<validator::Value> {
        &mut self.schema
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
    pub fn with_schema(mut self, schema: impl Into<validator::Value>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Finish assembling and produce the runnable [`Config`].
    pub fn build(self) -> Config {
        Config {
            plan: self.plan,
            merger_set: self.merger_set,
            loaders: self.loaders,
            parsers: self.parsers,
            #[cfg(feature = "validate-schema")]
            schema: self.schema,
        }
    }

    /// Shortcut for [`build`](Self::build) then [`Config::run`].
    pub fn run(self) -> Result<Entry, Error> {
        self.build().run()
    }

    /// Shortcut for [`build`](Self::build) then [`Config::try_deserialize`].
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(self) -> Result<T, Error> {
        self.build().try_deserialize()
    }
}

impl ConfigBuilder<Sources> {
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

/// Runs the load → parse → merge → unify → validate pipeline for a single configuration value.
///
/// Assemble one with [`Config::builder`] (simple-fold) or [`Config::from_plan`] (explicit tree), then
/// [`build`](ConfigBuilder::build). Call [`run`](Self::run) / [`try_deserialize`](Self::try_deserialize)
/// for the whole pipeline, or reach the individual stages through [`stages`](Self::stages).
pub struct Config {
    plan: MergePlan,
    merger_set: bool,
    loaders: Vec<Box<dyn loader::Load + Send + Sync>>,
    parsers: Vec<Box<dyn parser::Parse + Send + Sync>>,
    #[cfg(feature = "validate-schema")]
    schema: Option<validator::Value>,
}

impl Config {
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

    /// The global merger chosen via [`ConfigBuilder::with_merger`], if any. `None` when merging falls
    /// back to [`LastWins`](merger::LastWins), or for a [`from_plan`](Self::from_plan) tree.
    pub fn merger(&self) -> Option<&dyn merger::Merge> {
        self.merger_set
            .then(|| merger::root_merger(&self.plan))
            .flatten()
    }

    #[cfg(feature = "validate-schema")]
    pub fn schema(&self) -> Option<&validator::Value> {
        self.schema.as_ref()
    }

    /// The individual pipeline stages (`load` → `parse` → `merge` → `unify` → `validate`), for
    /// running or inspecting them one at a time. The terminal [`run`](Self::run) /
    /// [`try_deserialize`](Self::try_deserialize) shortcuts cover the common path.
    pub fn stages(&self) -> ConfigStages<'_> {
        ConfigStages { config: self }
    }

    /// Run load → parse → merge → unify → validate in sequence.
    pub fn run(&self) -> Result<Entry, Error> {
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Running single configuration pipeline", source_count = merger::leaves(&self.plan).len(), loader_count = self.loaders.len(), parser_count = self.parsers.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Running single configuration pipeline\" source_count={} loader_count={} parser_count={}", merger::leaves(&self.plan).len(), self.loaders.len(), self.parsers.len());
            }
        }
        let stages = self.stages();
        let loaded = stages.load()?;
        let parsed = stages.parse(&loaded)?;
        let merged = stages.merge(&parsed)?;
        let mut entry = stages.unify(&merged)?;
        stages.validate(entry.value_mut())?;
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

/// The pipeline stages of a [`Config`], reached through [`Config::stages`].
///
/// Each method runs one stage in isolation so callers can inspect intermediate results:
/// [`load`](Self::load) → [`parse`](Self::parse) → [`merge`](Self::merge) → [`unify`](Self::unify)
/// → [`validate`](Self::validate).
pub struct ConfigStages<'a> {
    config: &'a Config,
}

impl ConfigStages<'_> {
    pub fn load(&self) -> Result<Vec<loader::Payload>, Error> {
        let config = self.config;
        if config.loaders.is_empty() {
            return Err(Error::NoLoaders);
        }
        let mut result = Vec::new();
        for config_source in merger::leaves(&config.plan) {
            let source_name = config_source.source();
            cfg_if! {
                if #[cfg(feature = "tracing")] {
                    tracing::debug!(msg = "Loading configuration source", source = source_name, resource = config_source.resource());
                } else if #[cfg(feature = "logging")] {
                    log::debug!("msg=\"Loading configuration source\" source={source_name} resource={}", config_source.resource());
                }
            }
            let mut found_loader = None;
            for loader in &config.loaders {
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
        let config = self.config;
        if config.parsers.is_empty() {
            return Err(Error::NoParsers);
        }
        let all_sources: Vec<Source> = merger::leaves(&config.plan).into_iter().cloned().collect();
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
                for parser in &config.parsers {
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
                for parser in &config.parsers {
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
    /// merger pre-merging its own payloads first; a [`from_plan`](Config::from_plan) tree evaluates
    /// directly.
    pub fn merge(&self, parsed: &[Parsed]) -> Result<Merged, Error> {
        let config = self.config;
        cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::debug!(msg = "Starting configuration merge stage", entry_count = parsed.len());
            } else if #[cfg(feature = "logging")] {
                log::debug!("msg=\"Starting configuration merge stage\" entry_count={}", parsed.len());
            }
        }
        let groups = merger::group_by_source(&merger::leaves(&config.plan), parsed);
        match merger::plan::evaluate(&config.plan, &groups) {
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
    /// **`LastWins` note:** with [`LastWins`](merger::LastWins) as the configured merger, the
    /// cross-group pass keeps only the last group's value. Groups are ordered named-alphabetical
    /// then unnamed, so the unnamed bucket wins when present.
    pub fn unify(&self, merged: &Merged) -> Result<Entry, Error> {
        let config = self.config;
        let fallback = merger::LastWins;
        let merger: &dyn merger::Merge = config.merger().unwrap_or(&fallback);
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
            let Some(schema) = &self.config.schema else {
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
}
