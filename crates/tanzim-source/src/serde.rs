//! [`serde`] support (`serde` Cargo feature).
//!
//! [`Source`] serializes and deserializes as its canonical string form.

use crate::Source;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};
use std::fmt;

impl Serialize for Source {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(SourceVisitor)
    }
}

struct SourceVisitor;

impl<'de> Visitor<'de> for SourceVisitor {
    type Value = Source;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a configuration source string such as `env(prefix=APP)`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Source::parse(value).map_err(|error| E::custom(error.to_string()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        self.visit_str(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OptionValue;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct App {
        source: Source,
    }

    #[test]
    fn deserializes_config_source_from_string() {
        let app: App = serde_json::from_str(r#"{ "source": "env(prefix=APP_)" }"#).unwrap();
        assert_eq!(app.source.source(), "env");
        assert_eq!(
            app.source.options().get("prefix"),
            Some(&OptionValue::String("APP_".into()))
        );
    }

    #[test]
    fn serializes_config_source_as_string() {
        let app = App {
            source: Source::parse("file?:.env").unwrap(),
        };
        let json = serde_json::to_string(&app).unwrap();
        assert_eq!(json, r#"{"source":"file?:.env"}"#);
    }

    #[test]
    fn deserializes_invalid_config_source_with_parse_error() {
        let error = serde_json::from_str::<App>(r#"{ "source": "env(prefix=)" }"#).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("configuration source option value cannot be empty"));
        assert!(!message.contains('\n'));
    }
}
