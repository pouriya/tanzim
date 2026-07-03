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
            ignore_errors: value.ignore_errors(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;
    use std::str::FromStr;

    #[test]
    fn try_from_str() {
        let source = Source::try_from("file:/tmp/x").unwrap();
        assert_eq!(source.resource(), "/tmp/x");
    }

    #[test]
    fn builder_try_from_str() {
        let builder = SourceBuilder::try_from("env(prefix=APP_)").unwrap();
        let source = builder.build().unwrap();
        assert_eq!(source.source(), "env");
        assert_eq!(
            source.options().get("prefix"),
            Some(&OptionValue::String("APP_".into()))
        );
    }

    #[test]
    fn builder_try_from_invalid_is_parse_error() {
        let error = SourceBuilder::try_from("env(prefix=)").unwrap_err();
        assert!(matches!(error, Error::Parse(ParseError::EmptyValue { .. })));
    }

    #[test]
    fn source_round_trips_through_builder_from() {
        let original = Source::try_from("file:/tmp/x").unwrap();
        let rebuilt = SourceBuilder::from(original.clone()).build().unwrap();
        assert_eq!(rebuilt.resource(), original.resource());
        assert_eq!(rebuilt.source(), original.source());
    }

    #[test]
    fn source_from_str_matches_try_from() {
        let from_str = Source::from_str("env(prefix=APP_)").unwrap();
        let try_from = Source::try_from("env(prefix=APP_)").unwrap();
        assert_eq!(from_str.source(), try_from.source());
        assert_eq!(from_str.resource(), try_from.resource());
    }

    #[test]
    fn source_try_from_string_and_cow() {
        let owned = Source::try_from("file:/a".to_string()).unwrap();
        let borrowed = Source::try_from(&"file:/a".to_string()).unwrap();
        let cow = Source::try_from(Cow::Borrowed("file:/a")).unwrap();
        assert_eq!(owned.resource(), "/a");
        assert_eq!(borrowed.resource(), "/a");
        assert_eq!(cow.resource(), "/a");
    }

    #[test]
    fn builder_try_from_string_and_cow() {
        let owned = SourceBuilder::try_from("env".to_string()).unwrap();
        let borrowed = SourceBuilder::try_from(&"env".to_string()).unwrap();
        let cow = SourceBuilder::try_from(Cow::Borrowed("env")).unwrap();
        assert_eq!(owned.build().unwrap().source(), "env");
        assert_eq!(borrowed.build().unwrap().source(), "env");
        assert_eq!(cow.build().unwrap().source(), "env");
    }

    #[test]
    fn option_value_from_scalars() {
        assert_eq!(OptionValue::from(true), OptionValue::Bool(true));
        assert_eq!(OptionValue::from(42_i8), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_i16), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_i32), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_i64), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_u8), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_u16), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_u32), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_u64), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_usize), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(42_isize), OptionValue::Integer(42));
        assert_eq!(OptionValue::from(1.5_f32), OptionValue::Float(1.5));
        assert_eq!(OptionValue::from(2.5_f64), OptionValue::Float(2.5));
    }

    #[test]
    fn option_value_from_strings_and_collections() {
        assert_eq!(
            OptionValue::from("hello"),
            OptionValue::String("hello".into())
        );
        assert_eq!(
            OptionValue::from("hello".to_string()),
            OptionValue::String("hello".into())
        );
        let text = "hello".to_string();
        assert_eq!(
            OptionValue::from(&text),
            OptionValue::String("hello".into())
        );
        assert_eq!(
            OptionValue::from(vec![1_i64, 2_i64]),
            OptionValue::List(vec![OptionValue::Integer(1), OptionValue::Integer(2)])
        );
        assert_eq!(
            OptionValue::from([1_i64, 2_i64].as_slice()),
            OptionValue::List(vec![OptionValue::Integer(1), OptionValue::Integer(2)])
        );
        let options = Options::default();
        assert_eq!(
            OptionValue::from(options.clone()),
            OptionValue::Map(options)
        );
        assert_eq!(
            OptionValue::from(HashMap::from([("k", "v")])),
            OptionValue::Map({
                let mut map = Options::default();
                map.insert("k", "v");
                map
            })
        );
    }
}
