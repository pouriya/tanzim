use crate::Meta;
use std::fmt::{self, Display, Formatter};
use tanzim_value::{Location, ValueType};

/// One step in the path from the validated root to the offending value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// A map key (`static_map` / `dynamic_map`).
    Key(String),
    /// A list index.
    Index(usize),
}

/// What went wrong while validating a value.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// Wrong variant and no coercion applies.
    Type {
        expected: ValueType,
        found: ValueType,
    },
    /// Right variant family but the contents cannot be coerced to the target.
    NotConvertible { target: ValueType, found: ValueType },
    /// A semantic format check failed (host, email, uuid, …).
    Format { expected: &'static str },
    /// Numeric value below the inclusive minimum.
    BelowMin { value: String, min: String },
    /// Numeric value above the inclusive maximum.
    AboveMax { value: String, max: String },
    /// String/list/map shorter than the minimum length.
    TooShort { len: usize, min: usize },
    /// String/list/map longer than the maximum length.
    TooLong { len: usize, max: usize },
    /// String did not match the required pattern.
    PatternMismatch { pattern: String },
    /// A duplicate item was found in a list required to be unique.
    Duplicate { index: usize },
    /// A required key was missing from a map.
    MissingKey { key: String },
    /// A key not declared in the schema was present in a map.
    UnknownKey { key: String },
    /// A value was not in the allow-list (`Enum`).
    NotAllowed { value: String },
    /// Neither alternative of an `Either` accepted the value.
    Either {
        first: Box<Error>,
        second: Box<Error>,
    },
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            Self::NotConvertible { target, found } => {
                write!(f, "cannot convert {found} to {target}")
            }
            Self::Format { expected } => write!(f, "invalid {expected}"),
            Self::BelowMin { value, min } => write!(f, "{value} is below the minimum {min}"),
            Self::AboveMax { value, max } => write!(f, "{value} is above the maximum {max}"),
            Self::TooShort { len, min } => {
                write!(f, "length {len} is below the minimum {min}")
            }
            Self::TooLong { len, max } => write!(f, "length {len} is above the maximum {max}"),
            Self::PatternMismatch { pattern } => {
                write!(f, "does not match pattern `{pattern}`")
            }
            Self::Duplicate { index } => write!(f, "duplicate item at index {index}"),
            Self::MissingKey { key } => write!(f, "missing required key `{key}`"),
            Self::UnknownKey { key } => write!(f, "unknown key `{key}`"),
            Self::NotAllowed { value } => write!(f, "`{value}` is not an allowed value"),
            Self::Either { first, second } => {
                write!(f, "no alternative matched: ({first}) or ({second})")
            }
        }
    }
}

/// A validation failure, carrying a breadcrumb path and (when known) the source
/// [`Location`] of the offending value.
///
/// [`Display`] is one line by default; use `{error:#}` for the location's caret view.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    /// Path from the validated root to the offending value (root-first).
    pub path: Vec<Segment>,
    /// Source location, filled in by the enclosing value that owns it.
    ///
    /// Boxed to keep [`Error`] small enough to return by value (`clippy::result_large_err`).
    pub location: Option<Box<Location>>,
    /// The failing validator's human-facing metadata (name/description/examples/default).
    ///
    /// Boxed for the same size reason as `location`; filled in by the validator that failed
    /// (innermost wins).
    pub meta: Option<Box<Meta>>,
}

impl Error {
    /// Build a path-less, location-less error for the value currently being validated.
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            path: Vec::new(),
            location: None,
            meta: None,
        }
    }

    /// Attach `location` unless one is already set (the innermost owner wins).
    pub fn with_location(mut self, location: &Location) -> Self {
        if self.location.is_none() {
            self.location = Some(Box::new(location.clone()));
        }
        self
    }

    /// Attach the failing validator's [`Meta`] unless one is already set (innermost wins).
    pub fn with_meta(mut self, meta: &Meta) -> Self {
        if self.meta.is_none() {
            self.meta = Some(Box::new(meta.clone()));
        }
        self
    }

    /// The failing validator's name, if known.
    pub fn name(&self) -> Option<&str> {
        self.meta.as_ref().map(|meta| meta.name.as_str())
    }

    /// The failing validator's default value, if it declared one.
    pub fn default_value(&self) -> Option<&tanzim_value::Value> {
        self.meta.as_ref().and_then(|meta| meta.default.as_ref())
    }

    /// Record that this error happened under map key `key`, whose value lives at `location`.
    pub fn under_key(mut self, key: &str, location: &Location) -> Self {
        self.path.insert(0, Segment::Key(key.to_string()));
        self.with_location(location)
    }

    /// Record that this error happened under list index `index`, whose item lives at `location`.
    pub fn under_index(mut self, index: usize, location: &Location) -> Self {
        self.path.insert(0, Segment::Index(index));
        self.with_location(location)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(meta) = &self.meta
            && !meta.name.is_empty()
        {
            write!(f, "{}: ", meta.name)?;
        }
        if !self.path.is_empty() {
            // Root-first breadcrumb: dotted map keys, `[index]` for list items.
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
            write!(f, ": ")?;
        }
        write!(f, "{}", self.kind)?;
        if let Some(location) = &self.location {
            write!(f, " at {location}")?;
        }
        if f.alternate() {
            if let Some(meta) = &self.meta {
                if let Some(description) = &meta.description {
                    write!(f, "\n  {description}")?;
                }
                for (value, note) in &meta.examples {
                    match note {
                        Some(note) => write!(f, "\n  example: {value} ({note})")?,
                        None => write!(f, "\n  example: {value}")?,
                    }
                }
            }
            // Echo the offending value's pre-rendered source snippet (gutter + caret), computed at
            // parse time and stored on the `Location`.
            if let Some(location) = &self.location
                && !location.snippet.is_empty()
            {
                write!(f, "\n{}", location.snippet)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use tanzim_value::Location;

    #[test]
    fn nested_error_renders_path_and_innermost_location() {
        let leaf_loc = Location::at("file", "config.yaml", Some(3), Some(9), None);
        let outer_loc = Location::at("file", "config.yaml", Some(2), Some(1), None);
        let error = Error::new(ErrorKind::Type {
            expected: ValueType::Int,
            found: ValueType::String,
        })
        .under_key("port", &leaf_loc)
        .under_index(0, &outer_loc)
        .under_key("servers", &outer_loc);

        let message = error.to_string();
        assert!(message.starts_with("servers[0].port: expected integer, found string"));
        // innermost (leaf) location wins
        assert!(message.contains("config.yaml:3:9"));
    }

    #[test]
    fn alternate_display_shows_caret_snippet() {
        let text = "name: app\nport: nope\n";
        let location = tanzim_value::Location::in_text(
            tanzim_source::Source::named("file").with_resource("config.yaml"),
            text,
            Some(2),
            Some(7),
            Some(4),
        );
        let error = Error::new(ErrorKind::Type {
            expected: ValueType::Int,
            found: ValueType::String,
        })
        .with_location(&location);

        // Default display stays a single located line, no caret.
        let plain = error.to_string();
        assert!(!plain.contains('\n'));
        assert!(!plain.contains('^'));

        // Alternate display echoes the pre-rendered source window with a caret.
        let alternate = format!("{error:#}");
        assert!(alternate.contains("port: nope"), "{alternate}");
        assert!(alternate.contains("^^^^"), "{alternate}");
    }
}
