//! The multi-configuration pipeline: load, parse, merge, validate.
//!
//! [`Pipeline`] keeps an [`Entries`] map of named configurations (plus an optional root /
//! unnamed bucket). Assemble it with a [`PipelineBuilder`]: [`Pipeline::builder`] starts a
//! simple-fold builder (add sources, optionally pick a merger), while [`Pipeline::from_plan`]
//! starts from an explicit [`MergePlan`] tree. Call [`build`](PipelineBuilder::build) for the
//! runnable [`Pipeline`], or the [`run`](PipelineBuilder::run) /
//! [`try_deserialize`](PipelineBuilder::try_deserialize) shortcuts.
//!
//! The builder is a **typestate** — [`PipelineBuilder<Sources>`](PipelineBuilder) exposes
//! [`with_source`](PipelineBuilder::with_source) / [`with_merger`](PipelineBuilder::with_merger),
//! [`PipelineBuilder<Plan>`](PipelineBuilder) does not — so mixing the two is a compile error.
//!
//! For the common single-configuration case, prefer [`Config`](crate::Config).

use crate::config::{BuilderState, Plan, Sources};
use crate::loader;
use crate::merger::plan::MergePlan;
use crate::merger::{self, Entries};
#[cfg(feature = "validate-schema")]
use crate::merger::{EntryName, EntryNameRef};
use crate::parser::{self, Parsed};
use crate::source::{self, Source};
use cfg_if::cfg_if;
use std::marker::PhantomData;
use tanzim_source::{OnError, Stage};

#[cfg(feature = "validate-schema")]
use crate::validator;

/// Validation schemas keyed by merged entry name. Each value is a
/// [`Validator`](validator::Validator) — build one fluently or via [`validator::build_value`].
///
/// Prefer [`insert_root`](Self::insert_root) / [`insert_named`](Self::insert_named) over thinking
/// about `Option<String>` keys.
#[cfg(feature = "validate-schema")]
pub struct Schemas {
    entries: std::collections::HashMap<Option<String>, Box<dyn validator::Validator + Send + Sync>>,
}

#[cfg(feature = "validate-schema")]
impl Default for Schemas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "validate-schema")]
impl Schemas {
    /// An empty schema map.
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    /// The number of registered schemas.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no registered schemas.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Register (or replace) the schema for the root entry.
    pub fn insert_root(
        &mut self,
        schema: impl Into<Box<dyn validator::Validator + Send + Sync>>,
    ) -> Option<Box<dyn validator::Validator + Send + Sync>> {
        self.entries.insert(None, schema.into())
    }

    /// Register (or replace) the schema for a named entry.
    pub fn insert_named(
        &mut self,
        name: impl Into<String>,
        schema: impl Into<Box<dyn validator::Validator + Send + Sync>>,
    ) -> Option<Box<dyn validator::Validator + Send + Sync>> {
        self.entries.insert(Some(name.into()), schema.into())
    }

    /// Register (or replace) the schema for `name`.
    pub fn insert(
        &mut self,
        name: EntryName,
        schema: impl Into<Box<dyn validator::Validator + Send + Sync>>,
    ) -> Option<Box<dyn validator::Validator + Send + Sync>> {
        self.entries.insert(name.into_option(), schema.into())
    }

    /// Iterate over schemas by entry name.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (EntryNameRef<'_>, &(dyn validator::Validator + Send + Sync))> {
        self.entries.iter().map(|(name, schema)| {
            let key = match name {
                None => EntryNameRef::Root,
                Some(name) => EntryNameRef::Named(name.as_str()),
            };
            (key, schema.as_ref())
        })
    }
}

#[cfg(feature = "validate-schema")]
impl IntoIterator for Schemas {
    type Item = (EntryName, Box<dyn validator::Validator + Send + Sync>);
    type IntoIter = SchemasIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        SchemasIntoIter {
            inner: self.entries.into_iter(),
        }
    }
}

/// Owning iterator over [`Schemas`].
#[cfg(feature = "validate-schema")]
pub struct SchemasIntoIter {
    inner: std::collections::hash_map::IntoIter<
        Option<String>,
        Box<dyn validator::Validator + Send + Sync>,
    >,
}

#[cfg(feature = "validate-schema")]
impl Iterator for SchemasIntoIter {
    type Item = (EntryName, Box<dyn validator::Validator + Send + Sync>);

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some((name, schema)) => Some((EntryName::from_option(name), schema)),
            None => None,
        }
    }
}

fn source_display(cs: &Source) -> String {
    let mut s = cs.source().to_string();
    if cs.resource_colon() || !cs.resource().is_empty() {
        s.push(':');
        s.push_str(cs.resource());
    }
    s
}

/// A builder failure deferred until [`Pipeline::run`] so source/defaults builders stay infallible.
#[derive(Debug, Clone)]
enum DeferredError {
    Source(source::ParseError),
    Serialize(tanzim_value::Error),
}

/// Errors produced by the multi-configuration pipeline.
#[derive(Debug)]
pub enum Error {
    /// No loaders are registered, so no source can be loaded.
    NoLoaders,
    /// No parsers are registered, so no payload can be parsed.
    NoParsers,
    /// A source string failed to parse.
    Source(source::ParseError),
    /// Serializing programmatic defaults / a value layer into the configuration tree failed.
    Serialize(tanzim_value::Error),
    /// Loading a source failed.
    Load(loader::Error),
    /// Parsing a loaded payload failed.
    Parse(tanzim_value::Error),
    /// Merging the parsed payloads failed.
    Merge(merger::Error),
    /// Deserializing a merged entry into the target type failed.
    Deserialize(tanzim_value::Error),
    /// No registered loader supports a configured source.
    NoLoader {
        /// The display form of the source that no loader matched.
        at: String,
    },
    /// No registered parser supports a loaded payload's format.
    NoParser {
        /// The format that no parser matched (or `"unknown"` if none was declared).
        format: String,
        /// The display form of the source the payload came from.
        at: String,
    },

    /// A merged entry failed schema validation.
    #[cfg(feature = "validate-schema")]
    Validate {
        /// The name of the entry that failed validation.
        name: EntryName,
        /// The underlying validation error.
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
            Self::Serialize(error) => std::fmt::Display::fmt(error, f),
            Self::Load(error) => std::fmt::Display::fmt(error, f),
            Self::Parse(error) => std::fmt::Display::fmt(error, f),
            Self::Merge(error) => std::fmt::Display::fmt(error, f),
            Self::Deserialize(error) => std::fmt::Display::fmt(error, f),
            Self::NoLoader { at } => write!(f, "no loader found for `{at}`"),
            Self::NoParser { format, at } => {
                write!(f, "no parser found for format `{format}` in `{at}`")
            }
            #[cfg(feature = "validate-schema")]
            Self::Validate { name, inner } => {
                write!(f, "configuration `{name}` failed validation: ")?;
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
            Self::Serialize(error) | Self::Parse(error) | Self::Deserialize(error) => Some(error),
            Self::Merge(error) => Some(error),
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
///   add layers with [`with_defaults`](Self::with_defaults) / [`with_source`](Self::with_source) /
///   [`add_source`](Self::add_source), bind a per-source merger with
///   [`with_source_merged`](Self::with_source_merged), and pick the global merger with
///   [`with_merger`](Self::with_merger) (defaults to [`LastWins`](merger::LastWins)).
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
    /// The first builder failure (bad source string or defaults serialize), stashed so builders stay
    /// infallible. Surfaced when the pipeline runs.
    deferred_error: Option<DeferredError>,
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
            deferred_error: None,
            _state: PhantomData,
        }
    }

    /// Start a [`PipelineBuilder<Sources>`](PipelineBuilder) pre-loaded with every feature-enabled
    /// default loader and parser — the common starting point. Add sources with
    /// [`with_source`](PipelineBuilder::with_source) and finish with
    /// [`try_deserialize`](PipelineBuilder::try_deserialize).
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> PipelineBuilder<Sources> {
        Self::builder()
            .with_default_loaders()
            .with_default_parsers()
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
            deferred_error: None,
            _state: PhantomData,
        }
    }
}

impl<State: BuilderState> PipelineBuilder<State> {
    /// The configured configuration sources, in declared order.
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        merger::leaves(&self.plan).into_iter()
    }

    /// The registered loaders, in declared order.
    pub fn loaders(&self) -> &[Box<dyn loader::Load + Send + Sync>] {
        &self.loaders
    }

    /// Mutable access to the registered loaders.
    pub fn loaders_mut(&mut self) -> &mut Vec<Box<dyn loader::Load + Send + Sync>> {
        &mut self.loaders
    }

    /// The registered parsers, in declared order.
    pub fn parsers(&self) -> &[Box<dyn parser::Parse + Send + Sync>] {
        &self.parsers
    }

    /// Mutable access to the registered parsers.
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

    /// The registered validation schemas, keyed by entry name.
    #[cfg(feature = "validate-schema")]
    pub fn schemas(&self) -> &Schemas {
        &self.schemas
    }

    /// Mutable access to the registered validation schemas.
    #[cfg(feature = "validate-schema")]
    pub fn schemas_mut(&mut self) -> &mut Schemas {
        &mut self.schemas
    }

    /// Append a single loader (builder-style).
    pub fn with_loader(mut self, loader: impl loader::Load + Send + Sync + 'static) -> Self {
        self.loaders.push(Box::new(loader));
        self
    }

    /// Append a single parser (builder-style).
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

    /// Register a validator for the root (unnamed) entry. Pass any
    /// [`Validator`](validator::Validator) — build one fluently, or from a declarative schema
    /// document with [`validator::build_value`].
    #[cfg(feature = "validate-schema")]
    pub fn with_root_schema(
        mut self,
        schema: impl Into<Box<dyn validator::Validator + Send + Sync>>,
    ) -> Self {
        self.schemas.insert_root(schema);
        self
    }

    /// Register a validator for a named entry. Pass any
    /// [`Validator`](validator::Validator) — build one fluently, or from a declarative schema
    /// document with [`validator::build_value`].
    #[cfg(feature = "validate-schema")]
    pub fn with_named_schema(
        mut self,
        name: impl Into<String>,
        schema: impl Into<Box<dyn validator::Validator + Send + Sync>>,
    ) -> Self {
        self.schemas.insert_named(name, schema);
        self
    }

    /// Register multiple validators at once (builder-style), merging them into any already set.
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
            deferred_error: self.deferred_error,
        }
    }

    /// Shortcut for [`build`](Self::build) then [`Pipeline::run`].
    pub fn run(self) -> Result<Entries, Error> {
        self.build().run()
    }

    /// Shortcut for [`build`](Self::build) then [`Pipeline::try_deserialize`].
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(self) -> Result<Entries<T>, Error> {
        self.build().try_deserialize()
    }
}

impl PipelineBuilder<Sources> {
    /// Append programmatic defaults (builder-style).
    ///
    /// Serializes `defaults` into a value tree stamped with
    /// [`Location::at`](crate::value::Location::at) `"defaults"`, then appends it as a merge layer
    /// in the unnamed entry bucket. Infallible: a serialize failure is deferred and surfaced as
    /// [`Error::Serialize`] when the pipeline runs. For a named entry, use
    /// [`merger::plan::named_value`] in an explicit plan.
    pub fn with_defaults<T: serde::Serialize>(mut self, defaults: T) -> Self {
        self.add_defaults(defaults);
        self
    }

    /// Append programmatic defaults (in-place). See [`with_defaults`](Self::with_defaults).
    pub fn add_defaults<T: serde::Serialize>(&mut self, defaults: T) -> &mut Self {
        let location = crate::value::Location::at("defaults", "", None, None, None);
        match crate::value::LocatedValue::try_from_serialize(&defaults, location) {
            Ok(value) => merger::push_child(&mut self.plan, merger::plan::value(value)),
            Err(error) => self.record_deferred(DeferredError::Serialize(error)),
        }
        self
    }

    /// Append a pre-built value layer (builder-style). Lower-level escape hatch for
    /// [`with_defaults`](Self::with_defaults) when the caller already has a
    /// [`LocatedValue`](crate::value::LocatedValue).
    pub fn with_value(mut self, value: crate::value::LocatedValue) -> Self {
        self.add_value(value);
        self
    }

    /// Append a pre-built value layer (in-place). See [`with_value`](Self::with_value).
    pub fn add_value(&mut self, value: crate::value::LocatedValue) -> &mut Self {
        merger::push_child(&mut self.plan, merger::plan::value(value));
        self
    }

    /// Append a configuration source (builder-style). `source` may be a [`Source`] or any string form
    /// (e.g. `"file:app.toml"`), parsed now. This method is infallible: an invalid source string is
    /// stashed and surfaced later as [`Error::Source`] when the pipeline runs, so calls keep chaining.
    pub fn with_source<S>(mut self, source: S) -> Self
    where
        S: TryInto<Source>,
        Error: From<S::Error>,
    {
        self.add_source(source);
        self
    }

    /// Append a configuration source (in-place). See [`with_source`](Self::with_source).
    pub fn add_source<S>(&mut self, source: S) -> &mut Self
    where
        S: TryInto<Source>,
        Error: From<S::Error>,
    {
        match source.try_into() {
            Ok(source) => merger::push_child(&mut self.plan, MergePlan::Source(source)),
            Err(error) => {
                if let Error::Source(parse_error) = Error::from(error) {
                    self.record_deferred(DeferredError::Source(parse_error));
                }
            }
        }
        self
    }

    /// Append a source bound to its own `merger`, applied to that source's payloads before the
    /// global merger folds everything (builder-style). Infallible like [`with_source`](Self::with_source):
    /// an invalid source string is deferred to [`run`](Pipeline::run).
    pub fn with_source_merged<S>(
        mut self,
        source: S,
        merger: impl merger::Merge + Send + Sync + 'static,
    ) -> Self
    where
        S: TryInto<Source>,
        Error: From<S::Error>,
    {
        match source.try_into() {
            Ok(source) => merger::push_child(
                &mut self.plan,
                MergePlan::Merge {
                    merger: Box::new(merger),
                    children: vec![MergePlan::Source(source)],
                },
            ),
            Err(error) => {
                if let Error::Source(parse_error) = Error::from(error) {
                    self.record_deferred(DeferredError::Source(parse_error));
                }
            }
        }
        self
    }

    /// Stash the first builder failure, keeping builders infallible. Later failures are dropped —
    /// the first one wins and is returned by [`run`](Pipeline::run).
    fn record_deferred(&mut self, error: DeferredError) {
        if self.deferred_error.is_none() {
            self.deferred_error = Some(error);
        }
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
///
/// # Example
///
/// Where [`Config`](crate::Config) collapses everything into one value, a `Pipeline` keeps a map of
/// named entries. An env `separator` splits `APP_web__port` into entry `web`, key `port`; each entry
/// deserializes on its own. The sandbox sets the environment variables the example reads.
///
/// ```rust
/// # #[cfg(all(feature = "load-env", feature = "parse-env"))]
/// # tanzim_testing::environment::run(|env| {
/// use serde::Deserialize;
/// use tanzim::merger::Entries;
/// use tanzim::pipeline::Pipeline;
/// # env.set_env("APP_WEB__PORT", "8080")?;
/// # env.set_env("APP_DB__PORT", "5432")?;
///
/// #[derive(Deserialize)]
/// struct Service {
///     port: String, // the env parser keeps every value as a string
/// }
///
/// let services: Entries<Service> = Pipeline::default()
///     .with_source("env(prefix=APP_,separator=__)")
///     .try_deserialize()
///     .unwrap();
///
/// assert_eq!(services.named("web").unwrap().port, "8080");
/// assert_eq!(services.named("db").unwrap().port, "5432");
/// # Ok(())
/// # })
/// # .unwrap();
/// ```
pub struct Pipeline {
    plan: MergePlan,
    merger_set: bool,
    loaders: Vec<Box<dyn loader::Load + Send + Sync>>,
    parsers: Vec<Box<dyn parser::Parse + Send + Sync>>,
    #[cfg(feature = "validate-schema")]
    schemas: Schemas,
    /// A builder failure deferred so source/defaults builders stay infallible. Returned by
    /// [`run`](Pipeline::run) before any work.
    deferred_error: Option<DeferredError>,
}

impl Pipeline {
    /// The configured configuration sources, in declared order.
    pub fn sources(&self) -> impl Iterator<Item = &Source> {
        merger::leaves(&self.plan).into_iter()
    }

    /// The registered loaders, in declared order.
    pub fn loaders(&self) -> &[Box<dyn loader::Load + Send + Sync>] {
        &self.loaders
    }

    /// The registered parsers, in declared order.
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

    /// The registered validation schemas, keyed by entry name.
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
    pub fn run(&self) -> Result<Entries, Error> {
        if let Some(error) = &self.deferred_error {
            return Err(match error {
                DeferredError::Source(error) => Error::Source(error.clone()),
                DeferredError::Serialize(error) => Error::Serialize(error.clone()),
            });
        }
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

    /// Run the pipeline and deserialize each entry's configuration into `T` (the first failure
    /// aborts).
    pub fn try_deserialize<T: serde::de::DeserializeOwned>(&self) -> Result<Entries<T>, Error> {
        let merged = self.run()?;
        let mut out = Entries::new();
        for (name, entry) in merged.iter() {
            let deserialized = match entry.value().try_deserialize::<T>() {
                Ok(value) => value,
                Err(error) => {
                    return Err(Error::Deserialize(error));
                }
            };
            out.insert(name.to_owned(), deserialized);
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
    /// Run the load stage: read every configured source into raw payloads.
    ///
    /// When the plan has no [`Source`](MergePlan::Source) leaves (value-only / empty plans), this
    /// returns an empty list without requiring registered loaders.
    pub fn load(&self) -> Result<Vec<loader::Payload>, Error> {
        let pipeline = self.pipeline;
        let sources = merger::leaves(&pipeline.plan);
        if sources.is_empty() {
            return Ok(Vec::new());
        }
        if pipeline.loaders.is_empty() {
            return Err(Error::NoLoaders);
        }
        let mut result = Vec::new();
        for config_source in sources {
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

    /// Run the parse stage: turn every loaded payload into a value tree.
    ///
    /// When `loaded` is empty (no source payloads), this returns an empty list without requiring
    /// registered parsers — value leaves in the merge plan still participate at merge time.
    pub fn parse(&self, loaded: &[loader::Payload]) -> Result<Vec<Parsed>, Error> {
        let pipeline = self.pipeline;
        if loaded.is_empty() {
            return Ok(Vec::new());
        }
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
    pub fn merge(&self, parsed: &[Parsed]) -> Result<Entries, Error> {
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
                let merged = Entries::from_raw(raw);
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
    pub fn validate(&self, _merged: &mut Entries) -> Result<(), Error> {
        #[cfg(feature = "validate-schema")]
        {
            let schemas = &self.pipeline.schemas;
            if schemas.is_empty() {
                return Ok(());
            }
            for (name, schema) in schemas.iter() {
                let owned_name = name.to_owned();
                match _merged.get_mut(&owned_name) {
                    Some(entry) => match validator::validate(schema, entry.value_mut()) {
                        Ok(()) => {}
                        Err(inner) => {
                            return Err(Error::Validate {
                                name: owned_name,
                                inner,
                            });
                        }
                    },
                    None => {
                        cfg_if! {
                            if #[cfg(feature = "tracing")] {
                                tracing::warn!(msg = "Schema has no matching merged entry", name = %name);
                            } else if #[cfg(feature = "logging")] {
                                log::warn!("msg=\"Schema has no matching merged entry\" name={name}");
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
