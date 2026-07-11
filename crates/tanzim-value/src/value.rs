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
    /// The originating [`Source`] (name, options, resource, `on_error` policy).
    pub source: Source,
    /// 1-based line number, when known.
    pub line: Option<NonZeroU32>,
    /// 1-based column number, when known.
    pub column: Option<NonZeroU32>,
    /// UTF-8 character span length for error underlines; defaults to one caret.
    pub length: Option<NonZeroU32>,
    /// Pre-rendered `{error:#}` snippet: the ±3-line source window with gutter line numbers and a
    /// `^^^` caret line under the offending span, already formatted at construction. Empty for
    /// synthetic locations (no source text) or when there is no line to point at. Display just
    /// prints this — it never re-computes it. Build it with [`Location::in_text`].
    pub snippet: String,
}

/// Convert a 1-based `usize` position into the compact [`NonZeroU32`] storage.
///
/// Returns `None` for zero (treated as "no position") and clamps values larger
/// than [`u32::MAX`] to `u32::MAX` rather than overflowing.
fn position(value: usize) -> Option<NonZeroU32> {
    NonZeroU32::new(u32::try_from(value).unwrap_or(u32::MAX))
}

impl Location {
    /// Build a location from the full originating [`Source`], with no source snippet.
    ///
    /// Use [`Location::in_text`] instead when the source text is on hand, so `{error:#}` can render
    /// a caret window; this constructor leaves [`snippet`](Self::snippet) empty.
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
            snippet: String::new(),
        }
    }

    /// Build a location from the originating [`Source`] and its raw `text`, pre-rendering the
    /// `{error:#}` [`snippet`](Self::snippet): the offending line plus three lines of context on
    /// either side, each with a gutter line number, and a `^` caret line under the `column`/`length`
    /// span. When `line` is `None` (e.g. single-line input with no position) the snippet is left
    /// empty. The window is clamped to the file's bounds.
    pub fn in_text(
        source: Source,
        text: &str,
        line: Option<usize>,
        column: Option<usize>,
        length: Option<usize>,
    ) -> Self {
        let mut snippet = String::new();
        if let Some(line_number) = line {
            let highlight = length.unwrap_or(1).max(1);
            let lines: Vec<&str> = text.split('\n').collect();
            let offending = line_number.saturating_sub(1);
            let start = offending.saturating_sub(3);
            let end = (offending + 4).min(lines.len());
            let gutter_width = end.to_string().len();
            let mut rows: Vec<String> = Vec::new();
            for (offset, line_text) in lines[start..end].iter().enumerate() {
                let display_line = start + offset + 1;
                let number = display_line.to_string();
                let pad = gutter_width.saturating_sub(number.len());
                let mut row = String::from("  ");
                for _ in 0..pad {
                    row.push(' ');
                }
                row.push_str(&number);
                row.push_str(" | ");
                row.push_str(line_text);
                rows.push(row);
                if display_line == line_number {
                    let mut caret = String::from("  ");
                    for _ in 0..pad + number.len() + 1 {
                        caret.push(' ');
                    }
                    caret.push_str("| ");
                    if let Some(column_number) = column {
                        for _ in 1..column_number {
                            caret.push(' ');
                        }
                    }
                    for _ in 0..highlight {
                        caret.push('^');
                    }
                    rows.push(caret);
                }
            }
            snippet = rows.join("\n");
        }
        Self {
            source,
            line: line.and_then(position),
            column: column.and_then(position),
            length: length.and_then(position),
            snippet,
        }
    }

    /// Build a location from a bare source name and resource (a synthetic [`Source`] with no
    /// options), for origins that do not come from parsing a real source string. No snippet.
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

    /// Builder: set the UTF-8 character span length used for error underlines.
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
    /// A boolean value.
    Bool,
    /// A signed integer value.
    Int,
    /// A floating-point value.
    Float,
    /// A string value.
    String,
    /// A list of values.
    List,
    /// A map of keys to values.
    Map,
    /// The absence of a value.
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
///
/// Backed by a `Vec` of `(key, value)` pairs rather than a hash map, so insertion order is
/// preserved for display and iteration. Lookups scan from the end, so a repeated key shadows
/// its earlier insertion without removing it.
///
/// ```rust
/// use tanzim_value::{Location, Map, Value, LocatedValue};
///
/// let mut map = Map::new();
/// let location = Location::at("env", "", None, None, None);
/// map.insert(
///     "port".to_string(),
///     LocatedValue::new(Value::Int(8080), location.clone()),
/// );
/// map.insert(
///     "host".to_string(),
///     LocatedValue::new(Value::String("localhost".to_string()), location),
/// );
///
/// assert_eq!(map.len(), 2);
/// assert!(map.contains_key("port"));
/// assert_eq!(map.get("port").unwrap().value().as_int(), Some(8080));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Map {
    entries: Vec<(String, LocatedValue)>,
}

impl Map {
    /// Create an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries, including any shadowed (repeated-key) entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when the map has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `true` when `key` is present (checking the most recently inserted occurrence).
    pub fn contains_key(&self, key: &str) -> bool {
        for index in (0..self.entries.len()).rev() {
            if self.entries[index].0 == key {
                return true;
            }
        }
        false
    }

    /// The value for `key`, or `None` if absent. When `key` was inserted more than once, this
    /// returns the most recently inserted value.
    pub fn get(&self, key: &str) -> Option<&LocatedValue> {
        for index in (0..self.entries.len()).rev() {
            if self.entries[index].0 == key {
                return Some(&self.entries[index].1);
            }
        }
        None
    }

    /// Mutable access to the value for `key`, or `None` if absent.
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

    /// Insert `value` under `key`, removing (and returning) any prior value for that key.
    pub fn insert(&mut self, key: String, value: LocatedValue) -> Option<LocatedValue> {
        let old = self.remove(&key);
        self.entries.push((key, value));
        old
    }

    /// Remove and return the value for `key`, if present.
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

    /// All entries in insertion order, including any shadowed (repeated-key) entries.
    pub fn entries(&self) -> &[(String, LocatedValue)] {
        &self.entries
    }

    /// Mutable access to all entries, for in-place edits, reordering, or removal.
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
///
/// The tree shape produced by parsing configuration input: booleans, integers, floats, strings,
/// lists, maps, and null. Use the `is_*`/`as_*`/`into_*` family of methods to inspect or extract
/// a specific variant without a `match`.
///
/// ```rust
/// use tanzim_value::Value;
///
/// let value: Value = 8080isize.into();
/// assert!(value.is_int());
/// assert_eq!(value.as_int(), Some(8080));
///
/// let value: Value = "localhost".into();
/// assert_eq!(value.as_string().map(String::as_str), Some("localhost"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A boolean value.
    Bool(bool),
    /// A signed integer value.
    Int(isize),
    /// A floating-point value.
    Float(f64),
    /// A string value.
    String(String),
    /// A list of located values.
    List(Vec<LocatedValue>),
    /// A map of keys to located values.
    Map(Map),
    /// The absence of a value.
    Null,
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Value::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
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
    /// Create an empty comment (no before-lines, no inline comment).
    pub fn new() -> Self {
        Self::default()
    }

    /// Comment lines preceding the key; empty when none.
    pub fn before(&self) -> &[String] {
        &self.before
    }

    /// Mutable access to the before-lines.
    pub fn before_mut(&mut self) -> &mut Vec<String> {
        &mut self.before
    }

    /// Inline comment on the same line as the value; `None` when absent.
    pub fn after(&self) -> Option<&str> {
        self.after.as_deref()
    }

    /// Mutable access to the inline after-comment.
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
///
/// ```rust
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let location = Location::at("env", "PORT", None, None, None);
/// let located = LocatedValue::new(Value::Int(8080), location);
///
/// assert_eq!(located.value().as_int(), Some(8080));
/// assert_eq!(located.location().source_name(), "env");
/// assert_eq!(format!("{located}"), "8080");
/// ```
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

    /// The wrapped [`Value`].
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Mutable access to the wrapped [`Value`].
    pub fn value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// Consume this located value, discarding location and comment, and return the [`Value`].
    pub fn into_value(self) -> Value {
        self.value
    }

    /// Builder: replace the wrapped value.
    pub fn with_value(mut self, value: impl Into<Value>) -> Self {
        self.value = value.into();
        self
    }

    /// In-place setter for the wrapped value.
    pub fn set_value(&mut self, value: impl Into<Value>) {
        self.value = value.into();
    }

    // --- location ---

    /// The value's [`Location`].
    pub fn location(&self) -> &Location {
        &self.location
    }

    /// Mutable access to the [`Location`].
    pub fn location_mut(&mut self) -> &mut Location {
        &mut self.location
    }

    /// Builder: replace the location.
    pub fn with_location(mut self, location: impl Into<Location>) -> Self {
        self.location = location.into();
        self
    }

    /// In-place setter for the location.
    pub fn set_location(&mut self, location: impl Into<Location>) {
        self.location = location.into();
    }

    // --- comment ---

    /// The value's [`Comment`] (empty when none was attached).
    pub fn comment(&self) -> &Comment {
        &self.comment
    }

    /// Mutable access to the [`Comment`].
    pub fn comment_mut(&mut self) -> &mut Comment {
        &mut self.comment
    }

    /// Builder: replace the comment.
    pub fn with_comment(mut self, comment: Comment) -> Self {
        self.comment = comment;
        self
    }

    /// In-place setter for the comment.
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
    /// Create an empty [`Value::Map`].
    pub fn new_map() -> Self {
        Self::Map(Map::new())
    }

    /// Create an empty [`Value::List`].
    pub fn new_list() -> Self {
        Self::List(Vec::new())
    }

    /// Create an empty [`Value::String`].
    pub fn new_string() -> Self {
        Self::String(String::new())
    }

    /// `true` if this is a [`Value::Bool`].
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// The boolean, if this is a [`Value::Bool`].
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Consume and return the boolean, if this is a [`Value::Bool`].
    pub fn into_bool(self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the boolean, if this is a [`Value::Bool`].
    pub fn bool_mut(&mut self) -> Option<&mut bool> {
        match self {
            Self::Bool(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`Value::Int`].
    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int(_))
    }

    /// The integer, if this is a [`Value::Int`].
    pub fn as_int(&self) -> Option<isize> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// Consume and return the integer, if this is a [`Value::Int`].
    pub fn into_int(self) -> Option<isize> {
        match self {
            Self::Int(value) => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the integer, if this is a [`Value::Int`].
    pub fn int_mut(&mut self) -> Option<&mut isize> {
        match self {
            Self::Int(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`Value::Float`].
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    /// The float, if this is a [`Value::Float`].
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Consume and return the float, if this is a [`Value::Float`].
    pub fn into_float(self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the float, if this is a [`Value::Float`].
    pub fn float_mut(&mut self) -> Option<&mut f64> {
        match self {
            Self::Float(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`Value::String`].
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// The string, if this is a [`Value::String`].
    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Consume and return the string, if this is a [`Value::String`].
    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the string, if this is a [`Value::String`].
    pub fn string_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`Value::List`].
    pub fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// The list, if this is a [`Value::List`].
    pub fn as_list(&self) -> Option<&Vec<LocatedValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// Consume and return the list, if this is a [`Value::List`].
    pub fn into_list(self) -> Option<Vec<LocatedValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the list, if this is a [`Value::List`].
    pub fn list_mut(&mut self) -> Option<&mut Vec<LocatedValue>> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`Value::Map`].
    pub fn is_map(&self) -> bool {
        matches!(self, Self::Map(_))
    }

    /// The map, if this is a [`Value::Map`].
    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// Consume and return the map, if this is a [`Value::Map`].
    pub fn into_map(self) -> Option<Map> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the map, if this is a [`Value::Map`].
    pub fn map_mut(&mut self) -> Option<&mut Map> {
        match self {
            Self::Map(value) => Some(value),
            _ => None,
        }
    }

    /// `true` if this is a [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// This value's [`ValueType`].
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
