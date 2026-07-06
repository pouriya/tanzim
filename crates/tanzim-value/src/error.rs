use crate::Location;
use std::fmt::{self, Display, Formatter};

/// Error while deserializing configuration input.
///
/// [`Display`] is one line by default; use `{error:#}` for source context and caret.
///
/// [`Location`] is boxed so the whole [`Error`] stays small enough to return by value without
/// tripping `clippy::result_large_err` (a [`Location`] now carries the full originating
/// [`tanzim_source::Source`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    InvalidUtf8 {
        location: Box<Location>,
    },
    UnsupportedType {
        location: Box<Location>,
        found: &'static str,
    },
    Parse {
        location: Option<Box<Location>>,
        message: String,
    },
    /// A value could not be deserialized into the requested type (`serde` Cargo feature).
    ///
    /// The offending node's [`Location`] is stamped on by the deserializer (see
    /// [`Error::or_location`]); because every parsed node's `Location` already carries its
    /// pre-rendered source [`snippet`](Location::snippet), `{error:#}` renders a caret underline
    /// without any post-hoc source lookup.
    #[cfg(feature = "serde")]
    Deserialize {
        message: String,
        location: Option<Box<Location>>,
    },
}

impl Error {
    /// Stamp `location` onto a [`Error::Deserialize`] that has none yet, so errors bubbling up from
    /// a leaf get the nearest enclosing node's position (and its pre-rendered snippet). Errors that
    /// already carry a location (or are not deserialize errors) are returned unchanged.
    #[cfg(feature = "serde")]
    pub(crate) fn or_location(mut self, location: &Location) -> Self {
        if let Self::Deserialize {
            location: slot @ None,
            ..
        } = &mut self
        {
            *slot = Some(Box::new(location.clone()));
        }
        self
    }
}

fn located_message(location: &Location, message: &str) -> String {
    format!("{message} at {location}")
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 { location } => {
                write!(f, "invalid utf-8 in configuration input from {location}")?;
            }
            Self::UnsupportedType { location, found } => {
                write!(
                    f,
                    "{}",
                    located_message(
                        location,
                        &format!("unsupported configuration input type `{found}`"),
                    )
                )?;
            }
            Self::Parse {
                location: Some(location),
                message,
            } => write!(f, "{}", located_message(location, message))?,
            Self::Parse { message, .. } => write!(f, "{message}")?,
            #[cfg(feature = "serde")]
            Self::Deserialize {
                message,
                location: Some(location),
            } => write!(
                f,
                "{}",
                located_message(
                    location,
                    &format!("failed to deserialize configuration: {message}"),
                )
            )?,
            #[cfg(feature = "serde")]
            Self::Deserialize { message, .. } => {
                write!(f, "failed to deserialize configuration: {message}")?
            }
        }

        // Alternate form appends the offending value's pre-rendered source snippet (gutter + caret),
        // computed once at parse time and stored on the `Location` — nothing is recomputed here.
        if f.alternate() {
            let location = match self {
                Self::InvalidUtf8 { location } | Self::UnsupportedType { location, .. } => {
                    Some(location.as_ref())
                }
                Self::Parse {
                    location: Some(location),
                    ..
                } => Some(location.as_ref()),
                #[cfg(feature = "serde")]
                Self::Deserialize {
                    location: Some(location),
                    ..
                } => Some(location.as_ref()),
                _ => None,
            };
            if let Some(location) = location
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
    use crate::Location;
    use tanzim_source::Source;

    fn source() -> Source {
        Source::named("file").with_resource("config.toml")
    }

    #[test]
    fn default_display_is_single_line() {
        let error = Error::UnsupportedType {
            location: Box::new(Location::at("file", "config.toml", Some(2), Some(7), None)),
            found: "datetime",
        };
        let message = error.to_string();
        assert!(!message.contains('\n'));
        assert!(!message.contains('^'));
        assert!(message.contains("file:config.toml:2:7"));
    }

    #[test]
    fn alternate_display_underlines_token() {
        let text = "foo: bar\nbaz: datetime\n";
        let error = Error::UnsupportedType {
            location: Box::new(Location::in_text(source(), text, Some(2), Some(6), Some(8))),
            found: "datetime",
        };
        let message = format!("{error:#}");
        assert!(message.contains("^^^^"));
        assert!(message.contains("baz: datetime"));
    }

    #[test]
    fn alternate_display_aligns_gutter_pipe() {
        let text = "foo: bar\n\nbaz:\n\n  qux: datetime\n";
        let error = Error::UnsupportedType {
            location: Box::new(Location::in_text(source(), text, Some(5), Some(8), None)),
            found: "datetime",
        };
        let message = format!("{error:#}");
        let source_line = message
            .lines()
            .find(|line| line.contains("qux: datetime"))
            .expect("source line");
        let underline_line = message
            .lines()
            .find(|line| line.contains('^'))
            .expect("underline line");
        let source_pipe = source_line.find('|').expect("source pipe");
        let underline_pipe = underline_line.find('|').expect("underline pipe");
        assert_eq!(source_pipe, underline_pipe);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserialize_error_renders_caret_without_attach_step() {
        // A Deserialize error stamped with a node Location that already carries a snippet renders a
        // caret in alternate mode with no post-hoc source attachment.
        let text = "name: app\nport: nope\n";
        let location = Location::in_text(source(), text, Some(2), Some(7), Some(4));
        let error = Error::Deserialize {
            message: "invalid type: string, expected u16".to_string(),
            location: None,
        }
        .or_location(&location);
        let plain = error.to_string();
        assert!(!plain.contains('\n'));
        assert!(!plain.contains('^'));
        let alternate = format!("{error:#}");
        assert!(alternate.contains("port: nope"));
        assert!(alternate.contains("^^^^"));
    }
}
