#![doc = include_str!("../README.md")]

mod impls;
mod parse;

pub use parse::{ParseError, parse};

#[cfg(feature = "serde")]
mod serde;

use std::fmt::{Debug, Display, Formatter};

/// Error from building or parsing a [`Source`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Builder has no source identifier (missing or empty).
    #[error("configuration source is required")]
    MissingSource,
    /// Invalid configuration source string.
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// Kind of value stored in [`OptionValue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionValueType {
    Bool,
    Integer,
    Float,
    String,
    Map,
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
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<OptionValue>),
    Map(Options),
}

impl OptionValue {
    pub fn new_map() -> Self {
        Self::Map(Options::default())
    }

    pub fn new_list() -> Self {
        Self::List(Vec::new())
    }

    pub fn new_string() -> Self {
        Self::String(String::new())
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn into_bool(self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub fn bool_mut(&mut self) -> Option<&mut bool> {
        match self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer(_))
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn into_integer(self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn integer_mut(&mut self) -> Option<&mut i64> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn into_float(self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    pub fn float_mut(&mut self) -> Option<&mut f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn string_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    pub fn as_list(&self) -> Option<&Vec<OptionValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_list(self) -> Option<Vec<OptionValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    pub fn list_mut(&mut self) -> Option<&mut Vec<OptionValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_map(&self) -> bool {
        matches!(self, Self::Map(_))
    }

    pub fn as_map(&self) -> Option<&Options> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_map(self) -> Option<Options> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    pub fn map_mut(&mut self) -> Option<&mut Options> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(entry_key, _)| entry_key == key)
    }

    pub fn get(&self, key: &str) -> Option<&OptionValue> {
        self.entries
            .iter()
            .rfind(|(entry_key, _)| entry_key == key)
            .map(|(_, value)| value)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut OptionValue> {
        let index = self
            .entries
            .iter()
            .rposition(|(entry_key, _)| entry_key == key)?;
        Some(&mut self.entries[index].1)
    }

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

    pub fn remove(&mut self, key: &str) -> Option<OptionValue> {
        let index = self
            .entries
            .iter()
            .rposition(|(entry_key, _)| entry_key == key)?;
        Some(self.entries.remove(index).1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue)> {
        self.entries
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(key, _)| key.as_str())
    }

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
/// See the [crate-level documentation](crate) for the source string format, parsing rules, and
/// examples.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub(crate) source: String,
    pub(crate) options: Options,
    pub(crate) resource: String,
    pub(crate) ignore_errors: bool,
    pub(crate) resource_colon: bool,
}

impl Source {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        parse::parse(input)
    }

    pub fn source(&self) -> &str {
        self.source.as_str()
    }

    pub fn source_mut(&mut self) -> &mut String {
        &mut self.source
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = source.into();
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.options
    }

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    pub fn set_option<K: Into<String>, V: Into<OptionValue>>(&mut self, key: K, value: V) {
        self.options.insert(key, value);
    }

    pub fn with_option<K: Into<String>, V: Into<OptionValue>>(mut self, key: K, value: V) -> Self {
        self.options.insert(key, value);
        self
    }

    pub fn resource(&self) -> &str {
        self.resource.as_str()
    }

    pub fn resource_mut(&mut self) -> &mut String {
        &mut self.resource
    }

    pub fn set_resource(&mut self, resource: impl Into<String>) {
        self.resource = resource.into();
        if !self.resource.is_empty() {
            self.resource_colon = true;
        }
    }

    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = resource.into();
        if !self.resource.is_empty() {
            self.resource_colon = true;
        }
        self
    }

    pub fn ignore_errors(&self) -> bool {
        self.ignore_errors
    }

    pub fn set_ignore_errors(&mut self, ignore_errors: bool) {
        self.ignore_errors = ignore_errors;
    }

    pub fn with_ignore_errors(mut self, ignore_errors: bool) -> Self {
        self.ignore_errors = ignore_errors;
        self
    }

    pub fn resource_colon(&self) -> bool {
        self.resource_colon
    }

    pub fn set_resource_colon(&mut self, resource_colon: bool) {
        self.resource_colon = resource_colon;
    }

    pub fn with_resource_colon(mut self, resource_colon: bool) -> Self {
        self.resource_colon = resource_colon;
        self
    }
}

/// Builds a [`Source`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceBuilder {
    source: Option<String>,
    options: Options,
    resource: String,
    ignore_errors: bool,
    resource_colon: bool,
}

impl SourceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_resource(mut self, resource: impl Into<String>) -> Self {
        self.resource = resource.into();
        self
    }

    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    pub fn with_option<K: Into<String>, V: Into<OptionValue>>(mut self, key: K, value: V) -> Self {
        self.options.insert(key, value);
        self
    }

    pub fn with_ignore_errors(mut self, ignore_errors: bool) -> Self {
        self.ignore_errors = ignore_errors;
        self
    }

    pub fn with_resource_colon(mut self, resource_colon: bool) -> Self {
        self.resource_colon = resource_colon;
        self
    }

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
            ignore_errors: self.ignore_errors,
            resource_colon,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_requires_source() {
        let error = SourceBuilder::new().build().unwrap_err();
        assert!(matches!(error, Error::MissingSource));

        let error = SourceBuilder::new().with_source("").build().unwrap_err();
        assert!(matches!(error, Error::MissingSource));
    }

    #[test]
    fn builder_with_option_and_into_string() {
        let source = SourceBuilder::new()
            .with_source("env")
            .with_resource("")
            .with_option("prefix", "APP")
            .with_option("timeout", 30_i64)
            .with_option("retry", true)
            .build()
            .unwrap();

        assert_eq!(source.source(), "env");
        assert_eq!(source.resource(), "");
        assert_eq!(
            source.options().get("prefix"),
            Some(&OptionValue::String("APP".into()))
        );
        assert_eq!(
            source.options().get("timeout"),
            Some(&OptionValue::Integer(30))
        );
        assert_eq!(
            source.options().get("retry"),
            Some(&OptionValue::Bool(true))
        );
    }

    #[test]
    fn options_last_key_wins() {
        let mut options = Options::new();
        options.insert("prefix", "OLD");
        options.insert("prefix", "NEW");
        assert_eq!(options.len(), 1);
        assert_eq!(
            options.get("prefix"),
            Some(&OptionValue::String("NEW".into()))
        );
    }

    #[test]
    fn option_value_accessors_and_type_name() {
        let value = OptionValue::from(vec!["a", "b"]);
        assert!(value.is_list());
        assert_eq!(value.type_name(), OptionValueType::List);
        assert_eq!(value.as_list().unwrap().len(), 2);

        let mut map = OptionValue::new_map();
        map.map_mut()
            .unwrap()
            .insert("enabled", OptionValue::from(true));
        assert_eq!(map.type_name(), OptionValueType::Map);
    }

    #[test]
    fn config_source_setters() {
        let mut source = SourceBuilder::new()
            .with_source("file")
            .with_resource("/etc/app")
            .build()
            .unwrap();

        source.set_source("http");
        source.set_resource("https://example.com/config.json");
        source.set_option("timeout", 5_u32);

        assert_eq!(source.source(), "http");
        assert_eq!(source.resource(), "https://example.com/config.json");
        assert_eq!(
            source.options().get("timeout"),
            Some(&OptionValue::Integer(5))
        );
    }
}
