use crate::Location;
use std::fmt::{self, Display, Formatter};

/// Error while deserializing configuration input.
///
/// [`Display`] is one line by default; use `{error:#}` for source context and caret.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    InvalidUtf8 {
        location: Location,
    },
    UnsupportedNull {
        text: String,
        location: Location,
    },
    UnsupportedType {
        text: String,
        location: Location,
        found: &'static str,
    },
    Parse {
        text: String,
        location: Option<Location>,
        message: String,
    },
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
            Self::UnsupportedNull { location, .. } => {
                write!(
                    f,
                    "{}",
                    located_message(
                        location,
                        "null values are not supported in configuration input",
                    )
                )?;
            }
            Self::UnsupportedType {
                location, found, ..
            } => {
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
                ..
            } => write!(f, "{}", located_message(location, message))?,
            Self::Parse { message, .. } => write!(f, "{message}")?,
        }

        if !f.alternate() {
            return Ok(());
        }

        let (text, location) = match self {
            Self::UnsupportedNull { text, location, .. }
            | Self::UnsupportedType { text, location, .. } => (text.as_str(), location),
            Self::Parse {
                text,
                location: Some(location),
                ..
            } => (text.as_str(), location),
            _ => return Ok(()),
        };

        let line_number = location.line.map(|line| line.get() as usize);
        let column = location.column.map(|column| column.get() as usize);
        let highlight = location
            .length
            .map_or(1, |length| length.get() as usize)
            .max(1);

        if let Some(line_number) = line_number {
            let lines: Vec<&str> = text.split('\n').collect();
            let start = if line_number > 1 { line_number - 2 } else { 0 };
            let end = if line_number + 1 < lines.len() {
                line_number + 1
            } else {
                lines.len()
            };
            let gutter_width = end.to_string().len();
            let mut line_index = start;
            while line_index < end {
                let display_line = line_index + 1;
                let line_text = display_line.to_string();
                write!(f, "\n  ")?;
                for _ in 0..gutter_width.saturating_sub(line_text.len()) {
                    write!(f, " ")?;
                }
                write!(f, "{line_text} | ")?;
                write!(f, "{}", lines[line_index])?;
                if display_line == line_number {
                    write!(f, "\n  ")?;
                    for _ in 0..gutter_width.saturating_sub(line_text.len()) {
                        write!(f, " ")?;
                    }
                    for _ in 0..line_text.len() + 1 {
                        write!(f, " ")?;
                    }
                    write!(f, "| ")?;
                    if let Some(column_number) = column {
                        for _ in 1..column_number {
                            write!(f, " ")?;
                        }
                    }
                    for _ in 0..highlight {
                        write!(f, "^")?;
                    }
                }
                line_index += 1;
            }
        } else {
            write!(f, "\n  {text}")?;
            if let Some(column_number) = column {
                write!(f, "\n  ")?;
                for _ in 1..column_number {
                    write!(f, " ")?;
                }
                for _ in 0..highlight {
                    write!(f, "^")?;
                }
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

    #[test]
    fn default_display_is_single_line() {
        let error = Error::UnsupportedNull {
            text: "foo: bar\nbaz: ~\n".to_string(),
            location: Location::at("file", "config.yaml", Some(2), Some(7), None),
        };
        let message = error.to_string();
        assert!(!message.contains('\n'));
        assert!(!message.contains('^'));
        assert!(message.contains("file:config.yaml:2:7"));
    }

    #[test]
    fn alternate_display_underlines_token() {
        let error = Error::UnsupportedNull {
            text: "foo: bar\nbaz: null\n".to_string(),
            location: Location::at("file", "config.yaml", Some(2), Some(6), Some(4)),
        };
        let message = format!("{error:#}");
        assert!(message.contains("^^^^"));
        assert!(message.contains("baz: null"));
    }

    #[test]
    fn alternate_display_aligns_gutter_pipe() {
        let error = Error::UnsupportedNull {
            text: "foo: bar\n\nbaz:\n\n  qux: ~\n".to_string(),
            location: Location::at("file", "config.yaml", Some(5), Some(8), None),
        };
        let message = format!("{error:#}");
        let source_line = message
            .lines()
            .find(|line| line.contains("qux: ~"))
            .expect("source line");
        let underline_line = message
            .lines()
            .find(|line| line.contains('^'))
            .expect("underline line");
        let source_pipe = source_line.find('|').expect("source pipe");
        let underline_pipe = underline_line.find('|').expect("underline pipe");
        assert_eq!(source_pipe, underline_pipe);
    }
}
