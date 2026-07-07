//! [`From`] and [`TryFrom`] conversions for [`Source`], [`SourceBuilder`], and [`OptionValue`].

use crate::{Error, OptionValue, Options, ParseError, Source, SourceBuilder};
use std::borrow::Cow;
use std::collections::HashMap;

impl From<Source> for SourceBuilder {
    fn from(value: Source) -> Self {
        Self {
            source: Some(value.source().to_string()),
            options: value.options().clone(),
            resource: value.resource().to_string(),
            resource_colon: value.resource_colon(),
        }
    }
}

impl TryFrom<&str> for SourceBuilder {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Source::parse(value)?.into())
    }
}

impl TryFrom<String> for SourceBuilder {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl TryFrom<&String> for SourceBuilder {
    type Error = Error;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl TryFrom<Cow<'_, str>> for SourceBuilder {
    type Error = Error;

    fn try_from(value: Cow<'_, str>) -> Result<Self, Self::Error> {
        Self::try_from(value.as_ref())
    }
}

impl From<bool> for OptionValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for OptionValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for OptionValue {
    fn from(value: i32) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<i16> for OptionValue {
    fn from(value: i16) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<i8> for OptionValue {
    fn from(value: i8) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<u64> for OptionValue {
    fn from(value: u64) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<u32> for OptionValue {
    fn from(value: u32) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<u16> for OptionValue {
    fn from(value: u16) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<u8> for OptionValue {
    fn from(value: u8) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<isize> for OptionValue {
    fn from(value: isize) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<usize> for OptionValue {
    fn from(value: usize) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<f64> for OptionValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<f32> for OptionValue {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl From<&str> for OptionValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for OptionValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&String> for OptionValue {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl<T: Into<OptionValue>> From<Vec<T>> for OptionValue {
    fn from(value: Vec<T>) -> Self {
        Self::List(value.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<OptionValue> + Clone> From<&[T]> for OptionValue {
    fn from(value: &[T]) -> Self {
        Self::List(value.iter().cloned().map(Into::into).collect())
    }
}

impl From<Options> for OptionValue {
    fn from(value: Options) -> Self {
        Self::Map(value)
    }
}

impl<K: Into<String>, V: Into<OptionValue>> From<HashMap<K, V>> for OptionValue {
    fn from(value: HashMap<K, V>) -> Self {
        let mut options = Options::default();
        for (key, value) in value {
            options.insert(key, value);
        }
        Self::Map(options)
    }
}

impl std::str::FromStr for Source {
    type Err = ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for Source {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for Source {
    type Error = ParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl TryFrom<&String> for Source {
    type Error = ParseError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<Cow<'_, str>> for Source {
    type Error = ParseError;

    fn try_from(value: Cow<'_, str>) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
