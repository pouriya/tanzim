#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

mod impls;
mod parse;

pub use parse::{ParseError, parse};

#[cfg(feature = "serde")]
mod serde;

use std::fmt::{Debug, Display, Formatter};

/// Error from building or parsing a [`Source`].
#[derive(Debug)]
pub enum Error {
    /// Builder has no source identifier (missing or empty).
    MissingSource,
    /// Invalid configuration source string.
    Parse(ParseError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSource => write!(f, "configuration source is required"),
            // Transparent: forward Display (and its alternate form) to the wrapped error.
            Self::Parse(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::MissingSource => None,
        }
    }
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

/// A pipeline stage whose errors a [`Source`] can choose to tolerate.
///
/// Declared in the source string via the reserved `on_error` option, e.g.
/// `file(on_error=(load=skip,validate=skip)):/etc/app`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// The load stage (fetching the raw bytes).
    Load,
    /// The parse stage (turning bytes into values).
    Parse,
    /// The validate stage (checking values against a schema).
    Validate,
}

impl Stage {
    /// The reserved-option key name for this stage (`load` / `parse` / `validate`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Parse => "parse",
            Self::Validate => "validate",
        }
    }
}

impl Display for Stage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What to do when a stage fails for a given [`Source`].
///
/// The default for every stage is [`OnError::Fail`]; a source opts into tolerance per stage with
/// `on_error=(<stage>=skip)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OnError {
    /// Abort the pipeline with the error (default).
    #[default]
    Fail,
    /// Skip this source's contribution (load/parse) or fall back to a default (validate).
    Skip,
}

/// Kind of value stored in [`OptionValue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionValueType {
    /// A boolean value.
    Bool,
    /// A 64-bit signed integer.
    Integer,
    /// A 64-bit floating-point number.
    Float,
    /// A string value.
    String,
    /// A nested option map.
    Map,
    /// A list of values.
    List,
}

impl Display for OptionValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bool => "boolean",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::String => "string",
            Self::Map => "map",
            Self::List => "list",
        })
    }
}

/// Dynamically typed loader option or nested option map.
///
/// # Examples
///
/// ```rust
/// use tanzim_source::{OptionValue, OptionValueType};
///
/// let value = OptionValue::Integer(3);
/// assert_eq!(value.type_name(), OptionValueType::Integer);
/// assert_eq!(value.as_integer(), Some(3));
/// assert_eq!(value.as_bool(), None);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    /// A boolean value.
    Bool(bool),
    /// A 64-bit signed integer.
    Integer(i64),
    /// A 64-bit floating-point number.
    Float(f64),
    /// A string value.
    String(String),
    /// A list of values.
    List(Vec<OptionValue>),
    /// A nested option map.
    Map(Options),
}

impl OptionValue {
    /// Build an empty [`OptionValue::Map`].
    pub fn new_map() -> Self {
        Self::Map(Options::default())
    }

    /// Build an empty [`OptionValue::List`].
    pub fn new_list() -> Self {
        Self::List(Vec::new())
    }

    /// Build an empty [`OptionValue::String`].
    pub fn new_string() -> Self {
        Self::String(String::new())
    }

    /// `true` if this is a [`OptionValue::Bool`].
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// The boolean value, or `None` if this is not a [`OptionValue::Bool`].
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Convert into the boolean value, or `None` if this is not a [`OptionValue::Bool`].
    pub fn into_bool(self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    /// A mutable reference to the boolean value, or `None` if this is not a [`OptionValue::Bool`].
    pub fn bool_mut(&mut self) -> Option<&mut bool> {
        match self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`OptionValue::Integer`].
    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    /// The integer value, or `None` if this is not a [`OptionValue::Integer`].
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// Convert into the integer value, or `None` if this is not a [`OptionValue::Integer`].
    pub fn into_integer(self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }

    /// A mutable reference to the integer value, or `None` if this is not a [`OptionValue::Integer`].
    pub fn integer_mut(&mut self) -> Option<&mut i64> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`OptionValue::Float`].
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    /// The float value, or `None` if this is not a [`OptionValue::Float`].
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Convert into the float value, or `None` if this is not a [`OptionValue::Float`].
    pub fn into_float(self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    /// A mutable reference to the float value, or `None` if this is not a [`OptionValue::Float`].
    pub fn float_mut(&mut self) -> Option<&mut f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`OptionValue::String`].
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// The string value, or `None` if this is not a [`OptionValue::String`].
    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Convert into the string value, or `None` if this is not a [`OptionValue::String`].
    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// A mutable reference to the string value, or `None` if this is not a [`OptionValue::String`].
    pub fn string_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`OptionValue::List`].
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// The list value, or `None` if this is not a [`OptionValue::List`].
    pub fn as_list(&self) -> Option<&Vec<OptionValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// Convert into the list value, or `None` if this is not a [`OptionValue::List`].
    pub fn into_list(self) -> Option<Vec<OptionValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// A mutable reference to the list value, or `None` if this is not a [`OptionValue::List`].
    pub fn list_mut(&mut self) -> Option<&mut Vec<OptionValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`OptionValue::Map`].
    pub fn is_map(&self) -> bool {
        matches!(self, Self::Map(_))
    }

    /// The map value, or `None` if this is not a [`OptionValue::Map`].
    pub fn as_map(&self) -> Option<&Options> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// Convert into the map value, or `None` if this is not a [`OptionValue::Map`].
    pub fn into_map(self) -> Option<Options> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// A mutable reference to the map value, or `None` if this is not a [`OptionValue::Map`].
    pub fn map_mut(&mut self) -> Option<&mut Options> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// The [`OptionValueType`] kind of this value.
    pub fn type_name(&self) -> OptionValueType {
        match self {
            Self::Bool(_) => OptionValueType::Bool,
            Self::Integer(_) => OptionValueType::Integer,
            Self::Float(_) => OptionValueType::Float,
            Self::String(_) => OptionValueType::String,
            Self::List(_) => OptionValueType::List,
            Self::Map(_) => OptionValueType::Map,
        }
    }
}

impl Display for OptionValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value:?}"),
            Self::List(value) => {
                write!(f, "[")?;
                for (index, inner_value) in value.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{inner_value}")?;
                }
                write!(f, "]")
            }
            Self::Map(value) => write!(f, "{value}"),
        }
    }
}

/// Ordered map of loader options.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Options {
    entries: Vec<(String, OptionValue)>,
}

impl Options {
    /// Build an empty [`Options`] map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `true` if `key` is present.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(entry_key, _)| entry_key == key)
    }

    /// The value for `key`, or `None` if absent.
    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.entries
            .iter()
            .rfind(|(entry_key, _)| entry_key == key)
            .map(|(_, value)| value)
    }

    /// A mutable reference to the value for `key`, or `None` if absent.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut OptionValue> {
        let index = self
            .entries
            .iter()
            .rposition(|(entry_key, _)| entry_key == key)?;
        Some(&mut self.entries[index].1)
    }

    /// Insert `key` = `value`, returning the previous value if `key` was already present.
    ///
    /// A duplicate key replaces the earlier entry, matching the "last wins" rule for parsed
    /// source strings.
    pub fn insert<K: Into<String>, V: Into<OptionValue>>(
        &mut self,
        key: K,
        value: V,
    ) -> Option<OptionValue> {
        let key = key.into();
        let value = value.into();
        let old = self.remove(&key);
        self.entries.push((key, value));
        old
    }

    /// Remove and return the value for `key`, or `None` if absent.
    pub fn remove(&mut self, key: &str) -> Option<OptionValue> {
        let index = self
            .entries
            .iter()
            .rposition(|(entry_key, _)| entry_key == key)?;
        Some(self.entries.remove(index).1)
    }

    /// Iterate over `(key, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue)> {
        self.entries
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    /// Iterate over keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(key, _)| key.as_str())
    }

    /// Iterate over values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &OptionValue> {
        self.entries.iter().map(|(_, value)| value)
    }

    pub(crate) fn entries(&self) -> &[(String, OptionValue)] {
        &self.entries
    }
}

impl Display for Options {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        for (index, (key, value)) in self.entries.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{key:?}: {value}")?;
        }
        write!(f, "}}")
    }
}

/// One configuration source declaration.
///
/// A [`Source`] holds a loader name, its options, and an optional resource address, as parsed
/// from the `SOURCE[(OPTIONS)][:RESOURCE]` string format. See the
/// [crate-level documentation](crate) for the full format, parsing rules, and examples.
///
/// # Examples
///
/// ```rust
/// use tanzim_source::Source;
///
/// let source = Source::parse("env(prefix=APP_)")?;
/// assert_eq!(source.source(), "env");
/// assert_eq!(
///     source.options().get("prefix").unwrap().as_string().unwrap(),
///     "APP_"
/// );
/// assert_eq!(source.resource(), "");
/// assert_eq!(source.to_string(), "env(prefix=APP_)");
///
/// let file = Source::parse("file:app.toml")?;
/// assert_eq!(file.source(), "file");
/// assert_eq!(file.resource(), "app.toml");
/// # Ok::<(), tanzim_source::ParseError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub(crate) source: String,
    pub(crate) options: Options,
    pub(crate) resource: String,
    pub(crate) resource_colon: bool,
}

impl Source {
    /// Parse a `SOURCE[(OPTIONS)][:RESOURCE]` string into a [`Source`].
    ///
    /// Equivalent to the free function [`parse`]. See the [crate-level documentation](crate) for
    /// the format and rules.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse::parse(input)
    }

    /// Build a bare source with just a name — no options, no resource — infallibly.
    ///
    /// Used for synthetic origins (e.g. `tanzim_value::Location`s that do not come from parsing a
    /// real source string). Unlike [`Source::parse`] this performs no validation, so the name may
    /// even be empty for a placeholder origin.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            source: name.into(),
            options: Options::default(),
            resource: String::new(),
            resource_colon: false,
        }
    }

    /// The error policy this source declares for `stage`, via its reserved `on_error` option.
    ///
    /// Defaults to [`OnError::Fail`] when the option is absent, malformed, or does not mention the
    /// stage. `on_error=(load=skip,validate=skip)` yields [`OnError::Skip`] for those two stages.
    pub fn on_error(&self, stage: Stage) -> OnError {
        let Some(OptionValue::Map(map)) = self.options.get("on_error") else {
            return OnError::Fail;
        };
        match map.get(stage.as_str()) {
            Some(OptionValue::String(value)) if value.eq_ignore_ascii_case("skip") => OnError::Skip,
            _ => OnError::Fail,
        }
    }

    /// The loader name (e.g. `env`, `file`, `http`).
    pub fn source(&self) -> &str {
        self.source.as_str()
    }

    /// A mutable reference to the loader name.
    pub fn source_mut(&mut self) -> &mut String {
        &mut self.source
    }

    /// Set the loader name.
    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    /// Set the loader name, builder-style.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// The loader options.
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// A mutable reference to the loader options.
    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.options
    }

    /// Replace the loader options.
    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    /// Replace the loader options, builder-style.
    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    /// Set a single loader option.
    pub fn set_option<K: Into<String>, V: Into<OptionValue>>(&mut self, key: K, value: V) {
        self.options.insert(key, value);
    }

    /// Set a single loader option, builder-style.
    pub fn with_option<K: Into<String>, V: Into<OptionValue>>(mut self, key: K, value: V) -> Self {
        self.options.insert(key, value);
        self
    }

    /// The resource address (path, URL, …), or empty if none was given.
    pub fn resource(&self) -> &str {
        self.resource.as_str()
    }

    /// A mutable reference to the resource address.
    pub fn resource_mut(&mut self) -> &mut String {
        &mut self.resource
    }

    /// Set the resource address; a non-empty value also sets [`Source::resource_colon`].
    pub fn set_resource(&mut self, resource: impl Into<String>) {
        self.resource = resource.into();
        if !self.resource.is_empty() {
            self.resource_colon = true;
        }
    }

    /// Set the resource address, builder-style; a non-empty value also sets
    /// [`Source::resource_colon`].
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = resource.into();
        if !self.resource.is_empty() {
            self.resource_colon = true;
        }
        self
    }

    /// `true` if the source string had a `:` separator before the resource, even when the
    /// resource itself is empty (e.g. `file:`).
    pub fn resource_colon(&self) -> bool {
        self.resource_colon
    }

    /// Set whether the `:` separator is present before the resource.
    pub fn set_resource_colon(&mut self, resource_colon: bool) {
        self.resource_colon = resource_colon;
    }

    /// Set whether the `:` separator is present before the resource, builder-style.
    pub fn with_resource_colon(mut self, resource_colon: bool) -> Self {
        self.resource_colon = resource_colon;
        self
    }
}

/// Builds a [`Source`].
///
/// # Examples
///
/// ```rust
/// use tanzim_source::SourceBuilder;
///
/// let source = SourceBuilder::new()
///     .with_source("env")
///     .with_option("prefix", "APP_")
///     .build()?;
///
/// assert_eq!(source.to_string(), "env(prefix=APP_)");
/// # Ok::<(), tanzim_source::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceBuilder {
    source: Option<String>,
    options: Options,
    resource: String,
    resource_colon: bool,
}

impl SourceBuilder {
    /// Build an empty [`SourceBuilder`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the loader name.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the resource address.
    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = resource.into();
        self
    }

    /// Replace the loader options.
    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    /// Set a single loader option.
    pub fn with_option<K: Into<String>, V: Into<OptionValue>>(mut self, key: K, value: V) -> Self {
        self.options.insert(key, value);
        self
    }

    /// Set whether the `:` separator is present before the resource.
    pub fn with_resource_colon(mut self, resource_colon: bool) -> Self {
        self.resource_colon = resource_colon;
        self
    }

    /// Build the [`Source`], failing if no (non-empty) source name was set.
    pub fn build(self) -> Result<Source, Error> {
        let source = self.source.ok_or(Error::MissingSource)?;
        if source.is_empty() {
            return Err(Error::MissingSource);
        }
        let resource_colon = self.resource_colon || !self.resource.is_empty();
        Ok(Source {
            source,
            options: self.options,
            resource: self.resource,
            resource_colon,
        })
    }
}
