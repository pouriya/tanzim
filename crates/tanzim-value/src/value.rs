use std::fmt::{Debug, Display, Formatter};
use std::num::NonZeroU32;
use tanzim_source::Source;

/// Source and optional position of a configuration value.
///
/// Holds the full originating [`Source`] (name, options, resource — including any `on_error`
/// policy), so a value or error can be traced back to where and how it was declared. Positions are
/// 1-based and stored as [`NonZeroU32`]; [`crate::Error`] boxes the [`Location`] so results stay
/// small. Construct via [`Location::in_source`] (the real source) or [`Location::at`] (a bare
/// name/resource, for synthetic origins).
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub source: Source,
    pub line: Option<NonZeroU32>,
    pub column: Option<NonZeroU32>,
    /// UTF-8 character span length for error underlines; defaults to one caret.
    pub length: Option<NonZeroU32>,
}

/// Convert a 1-based `usize` position into the compact [`NonZeroU32`] storage.
///
/// Returns `None` for zero (treated as "no position") and clamps values larger
/// than [`u32::MAX`] to `u32::MAX` rather than overflowing.
fn position(value: usize) -> Option<NonZeroU32> {
    NonZeroU32::new(u32::try_from(value).unwrap_or(u32::MAX))
}

impl Location {
    /// Build a location from the full originating [`Source`].
    pub fn in_source(
        source: Source,
        line: Option<usize>,
        column: Option<usize>,
        length: Option<usize>,
    ) -> Self {
        Self {
            source,
            line: line.and_then(position),
            column: column.and_then(position),
            length: length.and_then(position),
        }
    }

    /// Build a location from a bare source name and resource (a synthetic [`Source`] with no
    /// options), for origins that do not come from parsing a real source string.
    pub fn at(
        source_name: &str,
        resource: &str,
        line: Option<usize>,
        column: Option<usize>,
        length: Option<usize>,
    ) -> Self {
        Self::in_source(
            Source::named(source_name).with_resource(resource),
            line,
            column,
            length,
        )
    }

    /// The originating source's name (loader kind).
    pub fn source_name(&self) -> &str {
        self.source.source()
    }

    /// The originating source's resource (address).
    pub fn resource(&self) -> &str {
        self.source.resource()
    }

    pub fn with_length(mut self, length: usize) -> Self {
        self.length = position(length);
        self
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let resource = self.source.resource();
        if resource.is_empty() {
            write!(f, "{}", self.source.source())?;
        } else {
            write!(f, "{}:{}", self.source.source(), resource)?;
        }
        match (self.line, self.column) {
            (Some(line), Some(column)) => write!(f, ":{line}:{column}"),
            (Some(line), None) => write!(f, ":{line}"),
            _ => Ok(()),
        }
    }
}

/// Kind of value stored in [`Value`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueType {
    Bool,
    Int,
    Float,
    String,
    List,
    Map,
    Null,
}

impl Display for ValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bool => "boolean",
            Self::Int => "integer",
            Self::Float => "float",
            Self::String => "string",
            Self::List => "list",
            Self::Map => "map",
            Self::Null => "null",
        })
    }
}

/// Ordered map of configuration keys to located values (last key wins on lookup).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map {
    entries: Vec<(String, LocatedValue)>,
}

impl Map {
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
        for index in (0..self.entries.len()).rev() {
            if self.entries[index].0 == key {
                return true;
            }
        }
        false
    }

    pub fn get(&self, key: &str) -> Option<&LocatedValue> {
        for index in (0..self.entries.len()).rev() {
            if self.entries[index].0 == key {
                return Some(&self.entries[index].1);
            }
        }
        None
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut LocatedValue> {
        let mut found = None;
        for index in (0..self.entries.len()).rev() {
            if self.entries[index].0 == key {
                found = Some(index);
                break;
            }
        }
        if let Some(index) = found {
            Some(&mut self.entries[index].1)
        } else {
            None
        }
    }

    pub fn insert(&mut self, key: String, value: LocatedValue) -> Option<LocatedValue> {
        let old = self.remove(&key);
        self.entries.push((key, value));
        old
    }

    pub fn remove(&mut self, key: &str) -> Option<LocatedValue> {
        let mut found = None;
        for index in (0..self.entries.len()).rev() {
            if self.entries[index].0 == key {
                found = Some(index);
                break;
            }
        }
        if let Some(index) = found {
            Some(self.entries.remove(index).1)
        } else {
            None
        }
    }

    pub fn entries(&self) -> &[(String, LocatedValue)] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut Vec<(String, LocatedValue)> {
        &mut self.entries
    }
}

impl Display for Map {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let alternate = f.alternate();
        let mut map = f.debug_map();
        for (key, value) in &self.entries {
            if alternate {
                map.entry(key, &format_args!("{:#}", value));
            } else {
                map.entry(key, &format_args!("{}", value));
            }
        }
        map.finish()
    }
}

/// Dynamically typed configuration value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(isize),
    Float(f64),
    String(String),
    List(Vec<LocatedValue>),
    Map(Map),
    Null,
}

/// Comment text attached to a [`LocatedValue`]: lines preceding the key and an optional
/// inline comment on the same line as the value.
///
/// Empty by default — callers check `.before().is_empty()` and `.after()` to detect absence.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Comment {
    before: Vec<String>,
    after: Option<String>,
}

impl Comment {
    pub fn new() -> Self {
        Self::default()
    }

    /// Comment lines preceding the key; empty when none.
    pub fn before(&self) -> &[String] {
        &self.before
    }

    pub fn before_mut(&mut self) -> &mut Vec<String> {
        &mut self.before
    }

    /// Inline comment on the same line as the value; `None` when absent.
    pub fn after(&self) -> Option<&str> {
        self.after.as_deref()
    }

    pub fn after_mut(&mut self) -> &mut Option<String> {
        &mut self.after
    }

    /// Builder: set the before-lines (replaces any existing).
    pub fn with_before(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.before = lines.into_iter().map(|l| l.into()).collect();
        self
    }

    /// Builder: set the inline after-comment.
    pub fn with_after(mut self, text: Option<impl Into<String>>) -> Self {
        self.after = text.map(|t| t.into());
        self
    }

    /// In-place setter for before-lines.
    pub fn set_before(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        self.before = lines.into_iter().map(|l| l.into()).collect();
    }

    /// In-place setter for the inline after-comment.
    pub fn set_after(&mut self, text: Option<impl Into<String>>) {
        self.after = text.map(|t| t.into());
    }
}

/// A [`Value`] with its [`Location`] and an optional [`Comment`].
///
/// Fields are private; build with [`LocatedValue::new`] and access through the provided
/// accessor methods. [`Display`] is compact by default; use `{value:#}` for a multiline dump
/// with `@source:resource:line:column` on the first line.
#[derive(Debug, Clone, PartialEq)]
pub struct LocatedValue {
    value: Value,
    location: Location,
    comment: Comment,
}

impl LocatedValue {
    /// Create a located value with an empty (default) comment.
    pub fn new(value: impl Into<Value>, location: impl Into<Location>) -> Self {
        Self {
            value: value.into(),
            location: location.into(),
            comment: Comment::new(),
        }
    }

    // --- value ---

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    pub fn with_value(mut self, value: impl Into<Value>) -> Self {
        self.value = value.into();
        self
    }

    pub fn set_value(&mut self, value: impl Into<Value>) {
        self.value = value.into();
    }

    // --- location ---

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn location_mut(&mut self) -> &mut Location {
        &mut self.location
    }

    pub fn with_location(mut self, location: impl Into<Location>) -> Self {
        self.location = location.into();
        self
    }

    pub fn set_location(&mut self, location: impl Into<Location>) {
        self.location = location.into();
    }

    // --- comment ---

    pub fn comment(&self) -> &Comment {
        &self.comment
    }

    pub fn comment_mut(&mut self) -> &mut Comment {
        &mut self.comment
    }

    pub fn with_comment(mut self, comment: Comment) -> Self {
        self.comment = comment;
        self
    }

    pub fn set_comment(&mut self, comment: Comment) {
        self.comment = comment;
    }
}

impl Display for LocatedValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            let mut map = f.debug_map();
            map.entry(&"value", &format_args!("{:#}", self.value));
            map.entry(
                &"location",
                &format_args!("{:?}", self.location.to_string()),
            );
            if !self.comment.before.is_empty() || self.comment.after.is_some() {
                map.entry(&"comment_before", &self.comment.before.as_slice());
                if let Some(after) = &self.comment.after {
                    map.entry(&"comment_after", &after.as_str());
                }
            }
            map.finish()
        } else {
            write!(f, "{}", self.value)
        }
    }
}

impl AsRef<Value> for Value {
    fn as_ref(&self) -> &Value {
        self
    }
}

impl AsRef<Value> for LocatedValue {
    fn as_ref(&self) -> &Value {
        &self.value
    }
}

impl Value {
    pub fn new_map() -> Self {
        Self::Map(Map::new())
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

    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int(_))
    }

    pub fn as_int(&self) -> Option<isize> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn into_int(self) -> Option<isize> {
        match self {
            Self::Int(value) => Some(value),
            _ => None,
        }
    }

    pub fn int_mut(&mut self) -> Option<&mut isize> {
        match self {
            Self::Int(value) => Some(value),
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

    pub fn as_list(&self) -> Option<&Vec<LocatedValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_list(self) -> Option<Vec<LocatedValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    pub fn list_mut(&mut self) -> Option<&mut Vec<LocatedValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_map(&self) -> bool {
        matches!(self, Self::Map(_))
    }

    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    pub fn into_map(self) -> Option<Map> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    pub fn map_mut(&mut self) -> Option<&mut Map> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn type_name(&self) -> ValueType {
        match self {
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::Float(_) => ValueType::Float,
            Self::String(_) => ValueType::String,
            Self::List(_) => ValueType::List,
            Self::Map(_) => ValueType::Map,
            Self::Null => ValueType::Null,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value:?}"),
            Self::List(values) => {
                let alternate = f.alternate();
                let mut list = f.debug_list();
                for value in values {
                    if alternate {
                        list.entry(&format_args!("{:#}", value));
                    } else {
                        list.entry(&format_args!("{}", value));
                    }
                }
                list.finish()
            }
            Self::Map(value) => Display::fmt(value, f),
            Self::Null => f.write_str("null"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn located_string(text: &str) -> LocatedValue {
        LocatedValue::new(
            Value::String(text.to_string()),
            Location::at("file", "test", None, None, None),
        )
    }

    #[test]
    fn as_ref_value_accepts_all_forms() {
        fn take<V: AsRef<Value>>(value: V) -> Value {
            value.as_ref().clone()
        }
        let value = Value::Int(7);
        let located = LocatedValue::new(
            Value::Int(7),
            Location::at("file", "test", None, None, None),
        );
        assert_eq!(take(value.clone()), value);
        assert_eq!(take(&value), value);
        assert_eq!(take(located.clone()), value);
        assert_eq!(take(&located), value);
    }

    #[test]
    fn last_key_wins() {
        let mut map = Map::new();
        map.insert("foo".to_string(), located_string("first"));
        map.insert("foo".to_string(), located_string("second"));
        assert_eq!(
            map.get("foo").unwrap().value().as_string().unwrap(),
            "second"
        );
    }

    #[test]
    fn default_display_is_compact() {
        let value = LocatedValue::new(
            Value::String("hello".to_string()),
            Location::at("file", "config.yaml", Some(2), Some(5), None),
        );
        let message = value.to_string();
        assert!(!message.contains('\n'));
        assert!(!message.starts_with('@'));
        assert_eq!(message, "\"hello\"");
    }

    #[test]
    fn alternate_display_shows_location_and_multiline() {
        let value = LocatedValue::new(
            Value::String("hello".to_string()),
            Location::at("file", "config.yaml", Some(2), Some(5), None),
        );
        let message = format!("{value:#}");
        assert_eq!(
            message,
            "{\n    \"value\": \"hello\",\n    \"location\": \"file:config.yaml:2:5\",\n}"
        );
        assert!(!message.contains('@'));
    }

    #[test]
    fn value_accessors_and_constructors() {
        let mut value = Value::Bool(true);
        assert!(value.is_bool());
        assert_eq!(value.as_bool(), Some(true));
        assert_eq!(value.type_name(), ValueType::Bool);
        if let Some(flag) = value.bool_mut() {
            *flag = false;
        }
        assert_eq!(value.into_bool(), Some(false));

        let list = Value::new_list();
        assert!(list.is_list());
        let map = Value::new_map();
        assert!(map.is_map());
        let text = Value::new_string();
        assert!(text.is_string());
    }

    #[test]
    fn map_remove_get_mut_and_display() {
        let mut map = Map::new();
        map.insert("a".to_string(), located_string("one"));
        map.insert("b".to_string(), located_string("two"));
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("a"));
        assert!(map.get_mut("b").is_some());
        let removed = map.remove("a");
        assert!(removed.is_some());
        assert!(!map.contains_key("a"));

        let compact = format!("{map}");
        assert!(compact.contains("b"));
        let detailed = format!("{map:#}");
        assert!(detailed.contains("location"));
    }

    #[test]
    fn location_display_and_with_length() {
        let location = Location::at("file", "", Some(1), Some(2), None).with_length(3);
        assert_eq!(location.to_string(), "file:1:2");
        let resourceful = Location::at("file", "cfg.yml", Some(4), None, None);
        assert_eq!(resourceful.to_string(), "file:cfg.yml:4");
    }

    #[test]
    fn comment_attached_to_located_value() {
        let lv = LocatedValue::new(
            Value::String("debug".into()),
            Location::at("file", "baz.toml", Some(4), Some(9), None),
        )
        .with_comment(
            Comment::new()
                .with_before(["# log level: debug, info, warn, error"])
                .with_after(Some("# inline")),
        );
        assert_eq!(
            lv.comment().before(),
            &["# log level: debug, info, warn, error"]
        );
        assert_eq!(lv.comment().after(), Some("# inline"));
        assert_eq!(lv.value().as_string().unwrap(), "debug");
    }

    #[test]
    fn comment_alternate_display_shows_comment_fields() {
        let lv = LocatedValue::new(
            Value::String("debug".into()),
            Location::at("file", "baz.toml", Some(4), Some(9), None),
        )
        .with_comment(Comment::new().with_before(["# level comment"]));
        let text = format!("{lv:#}");
        assert!(text.contains("\"comment_before\""));
        assert!(text.contains("level comment"));
    }

    #[test]
    fn value_list_and_map_display_modes() {
        let list = Value::List(vec![located_string("a"), located_string("b")]);
        assert!(format!("{list}").contains("a"));
        assert!(format!("{list:#}").contains("location"));

        let mut map = Map::new();
        map.insert("k".to_string(), located_string("v"));
        let map_value = Value::Map(map);
        assert!(format!("{map_value}").contains("k"));
    }
}
