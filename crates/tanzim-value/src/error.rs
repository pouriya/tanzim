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
