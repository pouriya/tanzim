use std::fmt::{Debug, Display, Formatter};
use std::num::NonZeroU32;

/// Source and optional position of a configuration value.
///
/// Positions are 1-based and stored as [`NonZeroU32`] so that the whole
/// [`Location`] (and therefore [`crate::Error`]) stays small enough to return by
/// value without triggering `clippy::result_large_err`. Construct via
/// [`Location::at`], which accepts ordinary `usize` positions and discards any
/// out-of-range or zero value as "absent".
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub source_name: String,
    pub resource: String,
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
    pub fn at(
        source_name: &str,
        resource: &str,
        line: Option<usize>,
        column: Option<usize>,
        length: Option<usize>,
    ) -> Self {
        Self {
            source_name: source_name.to_string(),
            resource: resource.to_string(),
            line: line.and_then(position),
            column: column.and_then(position),
            length: length.and_then(position),
        }
    }

    pub fn with_length(mut self, length: usize) -> Self {
        self.length = position(length);
        self
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.resource.is_empty() {
            write!(f, "{}", self.source_name)?;
        } else {
            write!(f, "{}:{}", self.source_name, self.resource)?;
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
        if f.alternate() {
            writeln!(f, "{{")?;
            for index in 0..self.entries.len() {
                if index > 0 {
                    writeln!(f, ",")?;
                }
                let (key, value) = &self.entries[index];
                write!(f, "  {key:?}:")?;
                writeln!(f)?;
                write!(f, "  {value:#}")?;
            }
            writeln!(f)?;
            write!(f, "}}")
        } else {
            write!(f, "{{")?;
            for index in 0..self.entries.len() {
                if index > 0 {
                    write!(f, ", ")?;
                }
                let (key, value) = &self.entries[index];
                write!(f, "{key:?}: {value}")?;
            }
            write!(f, "}}")
        }
    }
}

/// Dynamically typed configuration value (six variants, no null).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(isize),
    Float(f64),
    String(String),
    List(Vec<LocatedValue>),
    Map(Map),
}

/// A [`Value`] with its [`Location`].
///
/// [`Display`] is compact by default; use `{value:#}` for a multiline dump with
/// `@source:resource:line:column` on the first line.
#[derive(Debug, Clone, PartialEq)]
pub struct LocatedValue {
    pub value: Value,
    pub location: Location,
}

impl Display for LocatedValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            writeln!(f, "@{}", self.location)?;
            write!(f, "{:#}", self.value)
        } else {
            write!(f, "{}", self.value)
        }
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

    pub fn type_name(&self) -> ValueType {
        match self {
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::Float(_) => ValueType::Float,
            Self::String(_) => ValueType::String,
            Self::List(_) => ValueType::List,
            Self::Map(_) => ValueType::Map,
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
                if f.alternate() {
                    writeln!(f, "[")?;
                    for (index, value) in values.iter().enumerate() {
                        if index > 0 {
                            writeln!(f, ",")?;
                        }
                        write!(f, "  {value:#}")?;
                    }
                    writeln!(f)?;
                    write!(f, "]")
                } else {
                    write!(f, "[")?;
                    let mut first = true;
                    for value in values {
                        if !first {
                            write!(f, ", ")?;
                        }
                        write!(f, "{value}")?;
                        first = false;
                    }
                    write!(f, "]")
                }
            }
            Self::Map(value) => {
                if f.alternate() {
                    write!(f, "{value:#}")
                } else {
                    write!(f, "{value}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn located_string(text: &str) -> LocatedValue {
        LocatedValue {
            value: Value::String(text.to_string()),
            location: Location::at("file", "test", None, None, None),
        }
    }

    #[test]
    fn last_key_wins() {
        let mut map = Map::new();
        map.insert("foo".to_string(), located_string("first"));
        map.insert("foo".to_string(), located_string("second"));
        assert_eq!(map.get("foo").unwrap().value.as_string().unwrap(), "second");
    }

    #[test]
    fn default_display_is_compact() {
        let value = LocatedValue {
            value: Value::String("hello".to_string()),
            location: Location::at("file", "config.yaml", Some(2), Some(5), None),
        };
        let message = value.to_string();
        assert!(!message.contains('\n'));
        assert!(!message.starts_with('@'));
        assert_eq!(message, "\"hello\"");
    }

    #[test]
    fn alternate_display_shows_location_and_multiline() {
        let value = LocatedValue {
            value: Value::String("hello".to_string()),
            location: Location::at("file", "config.yaml", Some(2), Some(5), None),
        };
        let message = format!("{value:#}");
        assert!(message.starts_with("@file:config.yaml:2:5\n"));
        assert!(message.contains("\"hello\""));
    }
}
