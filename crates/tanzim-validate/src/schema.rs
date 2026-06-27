//! Build validators from a self-describing schema document.
//!
//! A schema is an ordinary [`Value`] tree (parse it with serde via [`SchemaValue`], or hand
//! one over directly from `tanzim-parse`). Every node is a map with a `"type"` tag plus the
//! options for that validator; the [`Registry`] dispatches on the tag to a constructor.
//! Custom validator types can be added with [`Registry::register`].

use std::collections::HashMap;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::Segment;
use crate::Validator;
use crate::{
    Bool, Domain, DynamicMap, Either, Email, Enum, Float, Host, Integer, IpAddr, List, NonEmpty,
    Number, Path, PathKind, Percentage, Port, SocketAddr, StaticMap, Str,
};
use tanzim_value::{LocatedValue, Location, Map, Value};

/// Location used for values produced by the serde deserializer, which carry no source span.
fn schema_location() -> Location {
    Location::at("schema", "", None, None, None)
}

/// A [`Value`] that can be produced by any serde deserializer (e.g. `serde_json`).
///
/// This is the bridge between the serde world and tanzim's own [`Value`] type. Deserialize a
/// schema into a `SchemaValue`, then feed it to [`build_value`] or a [`Registry`].
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaValue(pub Value);

impl SchemaValue {
    pub fn value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

struct SchemaValueVisitor;

impl<'de> Visitor<'de> for SchemaValueVisitor {
    type Value = SchemaValue;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a configuration value (no null)")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(SchemaValue(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        match isize::try_from(value) {
            Ok(number) => Ok(SchemaValue(Value::Int(number))),
            Err(_) => Err(de::Error::custom("integer out of range")),
        }
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        match isize::try_from(value) {
            Ok(number) => Ok(SchemaValue(Value::Int(number))),
            Err(_) => Err(de::Error::custom("integer out of range")),
        }
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
        Ok(SchemaValue(Value::Float(value)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(SchemaValue(Value::String(value.to_string())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(SchemaValue(Value::String(value)))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Err(de::Error::custom("null is not supported in configuration"))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Err(de::Error::custom("null is not supported in configuration"))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(element) = seq.next_element::<SchemaValue>()? {
            items.push(LocatedValue {
                value: element.0,
                location: schema_location(),
            });
        }
        Ok(SchemaValue(Value::List(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut map = Map::new();
        while let Some((key, element)) = access.next_entry::<String, SchemaValue>()? {
            map.insert(
                key,
                LocatedValue {
                    value: element.0,
                    location: schema_location(),
                },
            );
        }
        Ok(SchemaValue(Value::Map(map)))
    }
}

impl<'de> Deserialize<'de> for SchemaValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SchemaValueVisitor)
    }
}

/// What went wrong while building a validator from a schema document.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaErrorKind {
    /// A validator node was not a map.
    NotMap,
    /// The `"type"` tag named a validator the registry does not know.
    UnknownType { tag: String },
    /// A required field was absent.
    MissingField { field: String },
    /// A field had the wrong value type.
    WrongType {
        field: String,
        expected: &'static str,
    },
    /// A field had a structurally valid but semantically invalid value.
    InvalidValue { field: String, message: String },
}

impl std::fmt::Display for SchemaErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotMap => write!(f, "validator schema must be a map"),
            Self::UnknownType { tag } => write!(f, "unknown validator type `{tag}`"),
            Self::MissingField { field } => write!(f, "missing field `{field}`"),
            Self::WrongType { field, expected } => {
                write!(f, "field `{field}` must be {expected}")
            }
            Self::InvalidValue { field, message } => write!(f, "field `{field}`: {message}"),
        }
    }
}

/// A schema-construction failure, with a breadcrumb path and (when known) source location.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaError {
    pub kind: SchemaErrorKind,
    pub path: Vec<Segment>,
    /// Boxed to keep the error small (`clippy::result_large_err`).
    pub location: Option<Box<Location>>,
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (position, segment) in self.path.iter().enumerate() {
            match segment {
                Segment::Key(key) => {
                    if position > 0 {
                        write!(f, ".")?;
                    }
                    write!(f, "{key}")?;
                }
                Segment::Index(index) => write!(f, "[{index}]")?,
            }
        }
        if !self.path.is_empty() {
            write!(f, ": ")?;
        }
        write!(f, "{}", self.kind)?;
        if let Some(location) = &self.location {
            write!(f, " at {location}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SchemaError {}

/// A validator node: the map of options plus what's needed to read them and recurse.
///
/// Passed to each [`Registry`] constructor. Custom constructors use its readers
/// (`opt_int`, `flag`, `child`, …) to pull options and build nested validators.
pub struct Node<'a> {
    registry: &'a Registry,
    map: &'a Map,
    location: &'a Location,
    path: Vec<Segment>,
}

impl Node<'_> {
    /// Build an error anchored at this node.
    pub fn error(&self, kind: SchemaErrorKind) -> SchemaError {
        SchemaError {
            kind,
            path: self.path.clone(),
            location: Some(Box::new(self.location.clone())),
        }
    }

    fn missing(&self, field: &str) -> SchemaError {
        self.error(SchemaErrorKind::MissingField {
            field: field.to_string(),
        })
    }

    fn wrong(&self, field: &str, expected: &'static str) -> SchemaError {
        self.error(SchemaErrorKind::WrongType {
            field: field.to_string(),
            expected,
        })
    }

    /// Read a required string field.
    pub fn req_str(&self, field: &str) -> Result<&str, SchemaError> {
        match self.opt_str(field)? {
            Some(text) => Ok(text),
            None => Err(self.missing(field)),
        }
    }

    /// Read an optional string field.
    pub fn opt_str(&self, field: &str) -> Result<Option<&str>, SchemaError> {
        match self.map.get(field) {
            None => Ok(None),
            Some(entry) => match &entry.value {
                Value::String(text) => Ok(Some(text)),
                _ => Err(self.wrong(field, "a string")),
            },
        }
    }

    /// Read an optional integer field.
    pub fn opt_int(&self, field: &str) -> Result<Option<isize>, SchemaError> {
        match self.map.get(field) {
            None => Ok(None),
            Some(entry) => match &entry.value {
                Value::Int(number) => Ok(Some(*number)),
                _ => Err(self.wrong(field, "an integer")),
            },
        }
    }

    /// Read an optional non-negative integer field as a `usize`.
    pub fn opt_usize(&self, field: &str) -> Result<Option<usize>, SchemaError> {
        match self.opt_int(field)? {
            None => Ok(None),
            Some(number) => match usize::try_from(number) {
                Ok(value) => Ok(Some(value)),
                Err(_) => Err(self.error(SchemaErrorKind::InvalidValue {
                    field: field.to_string(),
                    message: "must be non-negative".to_string(),
                })),
            },
        }
    }

    /// Read an optional number field (integer or float) as an `f64`.
    pub fn opt_f64(&self, field: &str) -> Result<Option<f64>, SchemaError> {
        match self.map.get(field) {
            None => Ok(None),
            Some(entry) => match &entry.value {
                Value::Float(number) => Ok(Some(*number)),
                Value::Int(number) => Ok(Some(*number as f64)),
                _ => Err(self.wrong(field, "a number")),
            },
        }
    }

    /// Read an optional boolean field.
    pub fn opt_bool(&self, field: &str) -> Result<Option<bool>, SchemaError> {
        match self.map.get(field) {
            None => Ok(None),
            Some(entry) => match &entry.value {
                Value::Bool(value) => Ok(Some(*value)),
                _ => Err(self.wrong(field, "a boolean")),
            },
        }
    }

    /// Read a boolean field, defaulting to `false` when absent.
    pub fn flag(&self, field: &str) -> Result<bool, SchemaError> {
        match self.opt_bool(field)? {
            Some(value) => Ok(value),
            None => Ok(false),
        }
    }

    /// Read a list field as raw values (used by `enum`). Absent → empty.
    pub fn values(&self, field: &str) -> Result<Vec<Value>, SchemaError> {
        match self.map.get(field) {
            None => Ok(Vec::new()),
            Some(entry) => match &entry.value {
                Value::List(items) => {
                    let mut out = Vec::new();
                    for item in items {
                        out.push(item.value.clone());
                    }
                    Ok(out)
                }
                _ => Err(self.wrong(field, "a list")),
            },
        }
    }

    /// Read a list-of-strings field (used by `path.extensions`, `url.schemes`). Absent → empty.
    pub fn str_list(&self, field: &str) -> Result<Vec<String>, SchemaError> {
        match self.map.get(field) {
            None => Ok(Vec::new()),
            Some(entry) => match &entry.value {
                Value::List(items) => {
                    let mut out = Vec::new();
                    for item in items {
                        match &item.value {
                            Value::String(text) => out.push(text.clone()),
                            _ => return Err(self.wrong(field, "a list of strings")),
                        }
                    }
                    Ok(out)
                }
                _ => Err(self.wrong(field, "a list of strings")),
            },
        }
    }

    /// Build a required nested validator from a sub-schema field.
    pub fn child(&self, field: &str) -> Result<Box<dyn Validator>, SchemaError> {
        match self.map.get(field) {
            Some(entry) => self.build_sub(entry, field),
            None => Err(self.missing(field)),
        }
    }

    /// Build an optional nested validator from a sub-schema field.
    pub fn opt_child(&self, field: &str) -> Result<Option<Box<dyn Validator>>, SchemaError> {
        match self.map.get(field) {
            Some(entry) => Ok(Some(self.build_sub(entry, field)?)),
            None => Ok(None),
        }
    }

    fn build_sub(
        &self,
        entry: &LocatedValue,
        field: &str,
    ) -> Result<Box<dyn Validator>, SchemaError> {
        let mut path = self.path.clone();
        path.push(Segment::Key(field.to_string()));
        let node = self.registry.node(entry, path)?;
        self.registry.build_node(&node)
    }
}

/// Constructs one validator kind from its [`Node`].
pub type Constructor = Box<dyn Fn(&Node) -> Result<Box<dyn Validator>, SchemaError>>;

/// Maps `"type"` tags to validator constructors.
pub struct Registry {
    constructors: HashMap<String, Constructor>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl Registry {
    /// An empty registry with no constructors.
    pub fn empty() -> Self {
        Self {
            constructors: HashMap::new(),
        }
    }

    /// Register (or replace) the constructor for `tag`.
    pub fn register(
        &mut self,
        tag: impl Into<String>,
        constructor: impl Fn(&Node) -> Result<Box<dyn Validator>, SchemaError> + 'static,
    ) {
        self.constructors.insert(tag.into(), Box::new(constructor));
    }

    /// Build a validator from a located schema node, seeding source locations into errors.
    pub fn build(&self, value: &LocatedValue) -> Result<Box<dyn Validator>, SchemaError> {
        let node = self.node(value, Vec::new())?;
        self.build_node(&node)
    }

    /// Build a validator from a bare [`Value`] (errors carry no source location).
    pub fn build_value(&self, value: &Value) -> Result<Box<dyn Validator>, SchemaError> {
        let located = LocatedValue {
            value: value.clone(),
            location: schema_location(),
        };
        self.build(&located)
    }

    fn node<'a>(
        &'a self,
        value: &'a LocatedValue,
        path: Vec<Segment>,
    ) -> Result<Node<'a>, SchemaError> {
        match &value.value {
            Value::Map(map) => Ok(Node {
                registry: self,
                map,
                location: &value.location,
                path,
            }),
            _ => Err(SchemaError {
                kind: SchemaErrorKind::NotMap,
                path,
                location: Some(Box::new(value.location.clone())),
            }),
        }
    }

    fn build_node(&self, node: &Node) -> Result<Box<dyn Validator>, SchemaError> {
        let tag = node.req_str("type")?;
        match self.constructors.get(tag) {
            Some(constructor) => constructor(node),
            None => Err(node.error(SchemaErrorKind::UnknownType {
                tag: tag.to_string(),
            })),
        }
    }

    /// A registry pre-loaded with every built-in validator type.
    pub fn with_builtins() -> Self {
        let mut registry = Self::empty();

        registry.register("bool", |_node| Ok(Box::new(Bool::new())));
        registry.register("non_empty", |_node| Ok(Box::new(NonEmpty::new())));
        registry.register("percentage", |_node| Ok(Box::new(Percentage::new())));

        registry.register("integer", |node| {
            let mut validator = Integer::new();
            if let Some(min) = node.opt_int("min")? {
                validator = validator.min(min);
            }
            if let Some(max) = node.opt_int("max")? {
                validator = validator.max(max);
            }
            if node.flag("positive")? {
                validator = validator.positive();
            }
            if node.flag("non_negative")? {
                validator = validator.non_negative();
            }
            if node.flag("negative")? {
                validator = validator.negative();
            }
            if node.flag("non_positive")? {
                validator = validator.non_positive();
            }
            Ok(Box::new(validator))
        });

        registry.register("float", |node| {
            let mut validator = Float::new();
            if let Some(min) = node.opt_f64("min")? {
                validator = validator.min(min);
            }
            if let Some(max) = node.opt_f64("max")? {
                validator = validator.max(max);
            }
            if node.flag("positive")? {
                validator = validator.positive();
            }
            if node.flag("non_negative")? {
                validator = validator.non_negative();
            }
            if node.flag("negative")? {
                validator = validator.negative();
            }
            if node.flag("non_positive")? {
                validator = validator.non_positive();
            }
            Ok(Box::new(validator))
        });

        registry.register("number", |node| {
            let mut validator = Number::new();
            if let Some(min) = node.opt_f64("min")? {
                validator = validator.min(min);
            }
            if let Some(max) = node.opt_f64("max")? {
                validator = validator.max(max);
            }
            if node.flag("positive")? {
                validator = validator.positive();
            }
            if node.flag("non_negative")? {
                validator = validator.non_negative();
            }
            if node.flag("negative")? {
                validator = validator.negative();
            }
            if node.flag("non_positive")? {
                validator = validator.non_positive();
            }
            Ok(Box::new(validator))
        });

        registry.register("string", |node| {
            let mut validator = Str::new();
            if let Some(min) = node.opt_usize("min_chars")? {
                validator = validator.min_chars(min);
            }
            if let Some(max) = node.opt_usize("max_chars")? {
                validator = validator.max_chars(max);
            }
            #[cfg(feature = "regex")]
            if let Some(pattern) = node.opt_str("regex")? {
                validator = match validator.regex(pattern) {
                    Ok(validator) => validator,
                    Err(message) => {
                        return Err(node.error(SchemaErrorKind::InvalidValue {
                            field: "regex".to_string(),
                            message,
                        }));
                    }
                };
            }
            Ok(Box::new(validator))
        });

        registry.register("list", |node| {
            let mut validator = List::new();
            if let Some(min) = node.opt_usize("min_len")? {
                validator = validator.min_len(min);
            }
            if let Some(max) = node.opt_usize("max_len")? {
                validator = validator.max_len(max);
            }
            if node.flag("unique")? {
                validator = validator.unique();
            }
            if let Some(items) = node.opt_child("items")? {
                validator = validator.items(items);
            }
            Ok(Box::new(validator))
        });

        registry.register("dynamic_map", |node| {
            let mut validator = DynamicMap::new();
            if let Some(min) = node.opt_usize("min_len")? {
                validator = validator.min_len(min);
            }
            if let Some(max) = node.opt_usize("max_len")? {
                validator = validator.max_len(max);
            }
            if let Some(values) = node.opt_child("values")? {
                validator = validator.values(values);
            }
            Ok(Box::new(validator))
        });

        registry.register("static_map", |node| {
            let mut validator = StaticMap::new();
            if node.flag("allow_unknown")? {
                validator = validator.allow_unknown();
            }
            if let Some(entry) = node.map.get("fields") {
                let fields = match &entry.value {
                    Value::Map(map) => map,
                    _ => return Err(node.wrong("fields", "a map")),
                };
                for (key, field_entry) in fields.entries() {
                    let mut path = node.path.clone();
                    path.push(Segment::Key("fields".to_string()));
                    path.push(Segment::Key(key.clone()));
                    let field_node = node.registry.node(field_entry, path)?;
                    let required = field_node.flag("required")?;
                    let field_validator = field_node.opt_child("validator")?;
                    validator = match (required, field_validator) {
                        (true, Some(inner)) => validator.required(key.clone(), inner),
                        (true, None) => validator.required_any(key.clone()),
                        (false, Some(inner)) => validator.optional(key.clone(), inner),
                        (false, None) => validator.optional_any(key.clone()),
                    };
                }
            }
            Ok(Box::new(validator))
        });

        registry.register("enum", |node| {
            let mut validator = Enum::new(node.values("values")?);
            if node.flag("case_insensitive")? {
                validator = validator.case_insensitive();
            }
            Ok(Box::new(validator))
        });

        registry.register("either", |node| {
            let first = node.child("first")?;
            let second = node.child("second")?;
            Ok(Box::new(Either::new(first, second)))
        });

        registry.register("host", |_node| Ok(Box::new(Host::new())));
        registry.register("email", |_node| Ok(Box::new(Email::new())));
        registry.register("socket_addr", |_node| Ok(Box::new(SocketAddr::new())));

        registry.register("domain", |node| {
            let mut validator = Domain::new();
            if node.flag("require_dot")? {
                validator = validator.require_dot();
            }
            Ok(Box::new(validator))
        });

        registry.register("port", |node| {
            let mut validator = Port::new();
            if node.flag("allow_zero")? {
                validator = validator.allow_zero();
            }
            if let Some(privileged) = node.opt_bool("privileged_ok")? {
                validator = validator.privileged_ok(privileged);
            }
            Ok(Box::new(validator))
        });

        registry.register("ip_addr", |node| {
            let mut validator = IpAddr::new();
            if node.flag("v4_only")? {
                validator = validator.v4_only();
            }
            if node.flag("v6_only")? {
                validator = validator.v6_only();
            }
            Ok(Box::new(validator))
        });

        registry.register("path", |node| {
            let mut validator = Path::new();
            if node.flag("absolute")? {
                validator = validator.absolute();
            }
            if node.flag("relative")? {
                validator = validator.relative();
            }
            for extension in node.str_list("extensions")? {
                validator = validator.extension(extension);
            }
            if node.flag("must_exist")? {
                validator = validator.must_exist();
            }
            if let Some(kind) = node.opt_str("kind")? {
                let kind = match kind {
                    "dir" => PathKind::Dir,
                    "file" => PathKind::File,
                    "symlink" => PathKind::Symlink,
                    other => {
                        return Err(node.error(SchemaErrorKind::InvalidValue {
                            field: "kind".to_string(),
                            message: format!("unknown kind `{other}`"),
                        }));
                    }
                };
                validator = validator.kind(kind);
            }
            if node.flag("readable")? {
                validator = validator.readable();
            }
            if node.flag("writable")? {
                validator = validator.writable();
            }
            Ok(Box::new(validator))
        });

        #[cfg(feature = "regex")]
        registry.register("regex_pattern", |_node| {
            Ok(Box::new(crate::RegexPattern::new()))
        });

        #[cfg(feature = "url")]
        registry.register("url", |node| {
            let mut validator = crate::Url::new();
            let schemes = node.str_list("schemes")?;
            if !schemes.is_empty() {
                validator = validator.schemes(schemes);
            }
            if node.flag("require_host")? {
                validator = validator.require_host();
            }
            Ok(Box::new(validator))
        });

        #[cfg(feature = "cidr")]
        registry.register("cidr", |_node| Ok(Box::new(crate::Cidr::new())));

        #[cfg(feature = "uuid")]
        registry.register("uuid", |_node| Ok(Box::new(crate::Uuid::new())));

        #[cfg(feature = "semver")]
        registry.register("semver", |_node| Ok(Box::new(crate::Semver::new())));

        #[cfg(feature = "encoding")]
        {
            registry.register("base64", |_node| Ok(Box::new(crate::Base64::new())));
            registry.register("hex", |_node| Ok(Box::new(crate::Hex::new())));
        }

        #[cfg(feature = "duration")]
        registry.register("duration", |node| {
            let mut validator = crate::Duration::new();
            if node.flag("millis")? {
                validator = validator.millis();
            }
            Ok(Box::new(validator))
        });

        #[cfg(feature = "bytesize")]
        registry.register("bytesize", |_node| Ok(Box::new(crate::ByteSize::new())));

        #[cfg(feature = "datetime")]
        {
            registry.register("datetime", |_node| Ok(Box::new(crate::DateTime::new())));
            registry.register("date", |_node| Ok(Box::new(crate::Date::new())));
        }

        registry
    }
}

/// Build a validator from a located schema node using a default [`Registry`].
pub fn build(value: &LocatedValue) -> Result<Box<dyn Validator>, SchemaError> {
    Registry::with_builtins().build(value)
}

/// Build a validator from a bare [`Value`] using a default [`Registry`].
pub fn build_value(value: &Value) -> Result<Box<dyn Validator>, SchemaError> {
    Registry::with_builtins().build_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Value {
        let schema: SchemaValue = serde_json::from_str(json).unwrap();
        schema.into_value()
    }

    fn build_err(json: &str) -> SchemaError {
        match build_value(&parse(json)) {
            Ok(_) => panic!("expected a schema error"),
            Err(error) => error,
        }
    }

    #[test]
    fn builds_nested_schema_and_validates() {
        let schema = parse(
            r#"{
                "type": "static_map",
                "fields": {
                    "host": {"required": true,  "validator": {"type": "host"}},
                    "port": {"required": false, "validator": {"type": "port"}},
                    "tags": {"required": false, "validator": {
                        "type": "list", "unique": true,
                        "items": {"type": "string", "min_chars": 1}
                    }},
                    "mode": {"required": true, "validator": {
                        "type": "either",
                        "first":  {"type": "enum", "values": ["auto", "manual"]},
                        "second": {"type": "integer", "min": 0}
                    }}
                }
            }"#,
        );
        let validator = build_value(&schema).unwrap();

        let mut config = LocatedValue {
            value: parse(
                r#"{"host": "localhost", "port": "8080", "tags": ["a", "b"], "mode": "auto"}"#,
            ),
            location: schema_location(),
        };
        crate::validate(validator.as_ref(), &mut config).unwrap();

        // port string was coerced to an integer
        let port = config.value.as_map().unwrap().get("port").unwrap();
        assert_eq!(port.value, Value::Int(8080));
    }

    #[test]
    fn unknown_type_is_reported() {
        let error = build_err(r#"{"type": "nope"}"#);
        assert!(matches!(error.kind, SchemaErrorKind::UnknownType { .. }));
    }

    #[test]
    fn wrong_option_type_is_reported() {
        let error = build_err(r#"{"type": "integer", "min": "x"}"#);
        assert!(matches!(error.kind, SchemaErrorKind::WrongType { .. }));
    }

    #[test]
    fn missing_type_is_reported() {
        let error = build_err(r#"{"min": 1}"#);
        assert!(matches!(error.kind, SchemaErrorKind::MissingField { .. }));
    }

    #[test]
    fn nested_error_carries_path() {
        let error = build_err(r#"{"type": "list", "items": {"type": "integer", "min": "x"}}"#);
        assert_eq!(error.path, vec![Segment::Key("items".to_string())]);
    }

    #[test]
    fn custom_validator_can_be_registered() {
        let mut registry = Registry::with_builtins();
        registry.register("yes", |_node| Ok(Box::new(Bool::new())));
        let validator = registry.build_value(&parse(r#"{"type": "yes"}"#)).unwrap();
        assert!(validator.validate(&mut Value::Bool(true)).is_ok());
    }

    #[test]
    fn empty_registry_knows_nothing() {
        let error = match Registry::empty().build_value(&parse(r#"{"type": "bool"}"#)) {
            Ok(_) => panic!("expected a schema error"),
            Err(error) => error,
        };
        assert!(matches!(error.kind, SchemaErrorKind::UnknownType { .. }));
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn feature_gated_tag_round_trips() {
        let validator = build_value(&parse(r#"{"type": "uuid"}"#)).unwrap();
        assert!(
            validator
                .validate(&mut Value::String(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8".into()
                ))
                .is_ok()
        );
    }
}
