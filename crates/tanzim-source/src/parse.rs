//! Parse and format configuration source strings.
//!
//! Format: `SOURCE [(OPTIONS)] [?] [:RESOURCE]` — see crate README for rules.
//!
//! Use [`parse`] or [`Source::parse`] to parse; [`Source`] [`Display`] writes the canonical form.

use crate::{OptionValue, Options, Source};
use std::fmt::{self, Display, Formatter};

/// Error while parsing a configuration source string.
///
/// Format: `SOURCE [(OPTIONS)] [?] [:RESOURCE]` — see the crate README for rules.
///
/// [`Display`] is one line by default; use `{error:#}` for the input snippet and caret.
///
/// # Examples
///
/// ```rust
/// use tanzim_source::{Source, ParseError};
///
/// let error: ParseError = Source::parse("").unwrap_err();
/// assert_eq!(
///     error.to_string(),
///     "invalid configuration source at column 1: configuration source is required"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No source identifier (empty input or invalid start).
    MissingSource {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Input ended before a required token.
    UnexpectedEnd {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
        /// What was expected at this position.
        expected: &'static str,
    },
    /// Unexpected character at the current position.
    UnexpectedChar {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
        /// The character that was found instead.
        found: char,
        /// What was expected at this position.
        expected: &'static str,
    },
    /// Option or map key is not a valid identifier.
    InvalidIdentifier {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
        /// The invalid identifier text that was found.
        found: String,
    },
    /// Option or map key is empty.
    EmptyKey {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Option value is empty; use `""` for an empty string.
    EmptyValue {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Invalid escape sequence inside a quoted string.
    InvalidEscape {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Quoted string has no closing `"`.
    UnclosedString {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// List has no closing `]`.
    UnclosedList {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Map or options block has no closing `)`.
    UnclosedMap {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Comma with no following entry.
    TrailingComma {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
    },
    /// Token looks like a number but is not valid.
    InvalidNumber {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
        /// The invalid numeric token that was found.
        found: String,
    },
    /// Non-empty input after a complete configuration source.
    TrailingInput {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
        /// The unparsed remainder of the input.
        rest: String,
    },
    /// The reserved `on_error` option is malformed (bad shape, unknown stage, or bad value).
    InvalidOnError {
        /// The full input string being parsed.
        input: String,
        /// Byte offset into `input` where the error occurred.
        at: usize,
        /// Description of what is wrong with the `on_error` option.
        message: String,
    },
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (input, at, message) = match self {
            Self::MissingSource { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source is required".to_string(),
            ),
            Self::UnexpectedEnd {
                input,
                at,
                expected,
                ..
            } => (
                input.as_str(),
                *at,
                format!("configuration source: expected {expected}, found end of input"),
            ),
            Self::UnexpectedChar {
                input,
                at,
                found,
                expected,
                ..
            } => (
                input.as_str(),
                *at,
                format!("configuration source: expected {expected}, found `{found}`"),
            ),
            Self::InvalidIdentifier {
                input, at, found, ..
            } => (
                input.as_str(),
                *at,
                format!("configuration source: invalid identifier `{found}`"),
            ),
            Self::EmptyKey { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source option key cannot be empty".to_string(),
            ),
            Self::EmptyValue { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source option value cannot be empty; use \"\"".to_string(),
            ),
            Self::InvalidEscape { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source: invalid escape sequence in string".to_string(),
            ),
            Self::UnclosedString { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source: unclosed string".to_string(),
            ),
            Self::UnclosedList { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source: unclosed list".to_string(),
            ),
            Self::UnclosedMap { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source: unclosed map".to_string(),
            ),
            Self::TrailingComma { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source: trailing comma".to_string(),
            ),
            Self::InvalidNumber {
                input, at, found, ..
            } => (
                input.as_str(),
                *at,
                format!("configuration source: invalid number `{found}`"),
            ),
            Self::TrailingInput {
                input, at, rest, ..
            } => (
                input.as_str(),
                *at,
                format!("configuration source: unexpected trailing input `{rest}`"),
            ),
            Self::InvalidOnError { input, at, message } => (
                input.as_str(),
                *at,
                format!("configuration source: {message}"),
            ),
        };
        write!(
            f,
            "invalid configuration source at column {}: {}",
            at + 1,
            message
        )?;
        if f.alternate() {
            write!(f, "\n  {}\n  ", input)?;
            for _ in 0..at {
                write!(f, " ")?;
            }
            write!(f, "^")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

/// Parse a `SOURCE[(OPTIONS)][:RESOURCE]` configuration source string.
///
/// Equivalent to [`Source::parse`]. See the [crate-level documentation](crate) for the format
/// and rules.
///
/// # Examples
///
/// ```rust
/// use tanzim_source::parse;
///
/// let source = parse("env(prefix=APP_)")?;
/// assert_eq!(source.source(), "env");
/// assert_eq!(source.resource(), "");
///
/// let file = parse("file:app.toml")?;
/// assert_eq!(file.source(), "file");
/// assert_eq!(file.resource(), "app.toml");
/// # Ok::<(), tanzim_source::ParseError>(())
/// ```
///
/// A malformed source string returns a [`ParseError`]. Use `{error:#}` to render the input
/// snippet with a caret pointing at the offending column:
///
/// ```rust
/// use tanzim_source::parse;
///
/// let error = parse("file(bad").unwrap_err();
/// println!("{error:#}");
/// ```
///
/// which prints:
///
/// ```text
/// invalid configuration source at column 9: configuration source: expected option value after `=`, found end of input
///   file(bad
///          ^
/// ```
pub fn parse(input: &str) -> Result<Source, ParseError> {
    Parser::new(input).parse()
}

impl Display for Source {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source())?;
        if !self.options().is_empty() {
            write!(f, "(")?;
            for (index, (key, value)) in self.options().entries().iter().enumerate() {
                if index > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{key}=")?;
                write_option_value(f, value)?;
            }
            write!(f, ")")?;
        }
        if self.resource_colon() || !self.resource().is_empty() {
            write!(f, ":{}", self.resource())?;
        }
        Ok(())
    }
}

fn write_option_value(f: &mut Formatter<'_>, value: &OptionValue) -> fmt::Result {
    match value {
        OptionValue::Bool(value) => write!(f, "{value}"),
        OptionValue::Integer(value) => write!(f, "{value}"),
        OptionValue::Float(value) => {
            if value.is_finite() && value.fract() == 0.0 {
                write!(f, "{value:.1}")
            } else {
                write!(f, "{value}")
            }
        }
        OptionValue::String(value) => {
            let needs_quotes = value.is_empty()
                || !value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
                || value.eq_ignore_ascii_case("true")
                || value.eq_ignore_ascii_case("false")
                || is_int_token(value)
                || is_float_token(value);
            if needs_quotes {
                write!(f, "\"")?;
                for ch in value.chars() {
                    match ch {
                        '"' => write!(f, "\\\"")?,
                        '\\' => write!(f, "\\\\")?,
                        '\n' => write!(f, "\\n")?,
                        '\r' => write!(f, "\\r")?,
                        '\t' => write!(f, "\\t")?,
                        ch => write!(f, "{ch}")?,
                    }
                }
                write!(f, "\"")
            } else {
                write!(f, "{value}")
            }
        }
        OptionValue::List(values) => {
            write!(f, "[")?;
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    write!(f, ",")?;
                }
                write_option_value(f, item)?;
            }
            write!(f, "]")
        }
        OptionValue::Map(options) => {
            write!(f, "(")?;
            for (index, (key, item)) in options.entries().iter().enumerate() {
                if index > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{key}=")?;
                write_option_value(f, item)?;
            }
            write!(f, ")")
        }
    }
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn owned_input(&self) -> String {
        self.input.to_string()
    }

    fn parse(mut self) -> Result<Source, ParseError> {
        let source = self.parse_source()?;
        let options_at = self.pos;
        let options = if self.peek() == Some('(') {
            self.parse_options_block()?
        } else {
            Options::default()
        };
        let (resource_colon, resource) = if self.peek() == Some(':') {
            self.bump();
            let resource = self.input[self.pos..].to_string();
            self.pos = self.input.len();
            (true, resource)
        } else {
            (false, String::new())
        };
        if self.pos < self.input.len() {
            return Err(ParseError::TrailingInput {
                input: self.owned_input(),
                at: self.pos,
                rest: self.input[self.pos..].to_string(),
            });
        }
        if let Some(message) = validate_on_error(&options) {
            return Err(ParseError::InvalidOnError {
                input: self.owned_input(),
                at: options_at,
                message,
            });
        }
        Ok(Source {
            source,
            options,
            resource,
            resource_colon,
        })
    }

    fn parse_source(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        if !self
            .peek()
            .is_some_and(|ch| is_ident_char(ch) && !ch.is_ascii_digit())
        {
            if self.pos >= self.input.len() {
                return Err(ParseError::MissingSource {
                    input: self.owned_input(),
                    at: self.pos,
                });
            }
            let found = self.peek().unwrap();
            return Err(ParseError::UnexpectedChar {
                input: self.owned_input(),
                at: self.pos,
                found,
                expected: "source identifier",
            });
        }
        while self.peek().is_some_and(is_ident_char) {
            self.bump();
        }
        if self.pos == start {
            return Err(ParseError::MissingSource {
                input: self.owned_input(),
                at: self.pos,
            });
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_options_block(&mut self) -> Result<Options, ParseError> {
        self.expect_char('(', "opening `(` for options")?;
        let mut options = Options::default();
        if self.peek() == Some(')') {
            self.bump();
            return Ok(options);
        }
        loop {
            let key = self.parse_key()?;
            self.expect_char('=', "option value after `=`")?;
            let value = self.parse_value()?;
            options.insert(key, value);
            match self.peek() {
                Some(',') => {
                    self.bump();
                    if matches!(self.peek(), Some(')' | ']' | ',')) {
                        return Err(ParseError::TrailingComma {
                            input: self.owned_input(),
                            at: self.pos,
                        });
                    }
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                None => {
                    return Err(ParseError::UnclosedMap {
                        input: self.owned_input(),
                        at: self.pos,
                    });
                }
                Some(found) => {
                    return Err(ParseError::UnexpectedChar {
                        input: self.owned_input(),
                        at: self.pos,
                        found,
                        expected: "`,` or `)`",
                    });
                }
            }
        }
        Ok(options)
    }

    fn parse_map_value(&mut self) -> Result<OptionValue, ParseError> {
        self.expect_char('(', "opening `(` for map")?;
        let mut options = Options::default();
        if self.peek() == Some(')') {
            self.bump();
            return Ok(OptionValue::Map(options));
        }
        loop {
            let key = self.parse_key()?;
            self.expect_char('=', "map value after `=`")?;
            let value = self.parse_value()?;
            options.insert(key, value);
            match self.peek() {
                Some(',') => {
                    self.bump();
                    if matches!(self.peek(), Some(')' | ']' | ',')) {
                        return Err(ParseError::TrailingComma {
                            input: self.owned_input(),
                            at: self.pos,
                        });
                    }
                }
                Some(')') => {
                    self.bump();
                    break;
                }
                None => {
                    return Err(ParseError::UnclosedMap {
                        input: self.owned_input(),
                        at: self.pos,
                    });
                }
                Some(found) => {
                    return Err(ParseError::UnexpectedChar {
                        input: self.owned_input(),
                        at: self.pos,
                        found,
                        expected: "`,` or `)`",
                    });
                }
            }
        }
        Ok(OptionValue::Map(options))
    }

    fn parse_list_value(&mut self) -> Result<OptionValue, ParseError> {
        self.expect_char('[', "opening `[` for list")?;
        let mut values = Vec::new();
        if self.peek() == Some(']') {
            self.bump();
            return Ok(OptionValue::List(values));
        }
        loop {
            values.push(self.parse_value()?);
            match self.peek() {
                Some(',') => {
                    self.bump();
                    if matches!(self.peek(), Some(']' | ',')) {
                        return Err(ParseError::TrailingComma {
                            input: self.owned_input(),
                            at: self.pos,
                        });
                    }
                }
                Some(']') => {
                    self.bump();
                    break;
                }
                None => {
                    return Err(ParseError::UnclosedList {
                        input: self.owned_input(),
                        at: self.pos,
                    });
                }
                Some(found) => {
                    return Err(ParseError::UnexpectedChar {
                        input: self.owned_input(),
                        at: self.pos,
                        found,
                        expected: "`,` or `]`",
                    });
                }
            }
        }
        Ok(OptionValue::List(values))
    }

    fn parse_key(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        if !self
            .peek()
            .is_some_and(|ch| is_ident_char(ch) && !ch.is_ascii_digit())
        {
            if self.peek() == Some('=') {
                return Err(ParseError::EmptyKey {
                    input: self.owned_input(),
                    at: self.pos,
                });
            }
            let found = self
                .peek()
                .map(|ch| ch.to_string())
                .unwrap_or_else(|| "end of input".to_string());
            return if self.peek().is_some() {
                Err(ParseError::UnexpectedChar {
                    input: self.owned_input(),
                    at: self.pos,
                    found: self.peek().unwrap(),
                    expected: "option key",
                })
            } else {
                Err(ParseError::InvalidIdentifier {
                    input: self.owned_input(),
                    at: self.pos,
                    found,
                })
            };
        }
        while self.peek().is_some_and(is_ident_char) {
            self.bump();
        }
        if self.pos == start {
            return Err(ParseError::EmptyKey {
                input: self.owned_input(),
                at: self.pos,
            });
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_value(&mut self) -> Result<OptionValue, ParseError> {
        match self.peek() {
            Some('"') => Ok(OptionValue::String(self.parse_quoted_string()?)),
            Some('[') => self.parse_list_value(),
            Some('(') => self.parse_map_value(),
            Some('=') | Some(',') | Some(')') | Some(']') | Some(':') | Some('?') | None => {
                Err(ParseError::EmptyValue {
                    input: self.owned_input(),
                    at: self.pos,
                })
            }
            Some(_) => {
                let token = self.parse_unquoted_token()?;
                let at = self.pos - token.len();
                let owned_input = self.input.to_string();
                if token.eq_ignore_ascii_case("true") {
                    Ok(OptionValue::Bool(true))
                } else if token.eq_ignore_ascii_case("false") {
                    Ok(OptionValue::Bool(false))
                } else if token.contains('.') {
                    if !is_float_token(&token) {
                        Err(ParseError::InvalidNumber {
                            input: owned_input,
                            at,
                            found: token,
                        })
                    } else {
                        token.parse::<f64>().map(OptionValue::Float).map_err(|_| {
                            ParseError::InvalidNumber {
                                input: owned_input,
                                at,
                                found: token,
                            }
                        })
                    }
                } else if is_int_token(&token) {
                    token.parse::<i64>().map(OptionValue::Integer).map_err(|_| {
                        ParseError::InvalidNumber {
                            input: owned_input,
                            at,
                            found: token,
                        }
                    })
                } else {
                    Ok(OptionValue::String(token))
                }
            }
        }
    }

    fn parse_unquoted_token(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            self.bump();
        }
        if self.pos == start {
            let found = self.peek().unwrap();
            return Err(ParseError::UnexpectedChar {
                input: self.owned_input(),
                at: self.pos,
                found,
                expected: "value",
            });
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_quoted_string(&mut self) -> Result<String, ParseError> {
        self.expect_char('"', "opening `\"` for string")?;
        let start = self.pos;
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.bump();
                return Ok(value);
            }
            if ch == '\\' {
                self.bump();
                let escaped = self.peek().ok_or(ParseError::UnclosedString {
                    input: self.owned_input(),
                    at: start,
                })?;
                value.push(match escaped {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => {
                        return Err(ParseError::InvalidEscape {
                            input: self.owned_input(),
                            at: self.pos - 1,
                        });
                    }
                });
                self.bump();
                continue;
            }
            self.bump();
            value.push(ch);
        }
        Err(ParseError::UnclosedString {
            input: self.owned_input(),
            at: start,
        })
    }

    fn expect_char(&mut self, expected: char, message: &'static str) -> Result<(), ParseError> {
        match self.peek() {
            Some(found) if found == expected => {
                self.bump();
                Ok(())
            }
            Some(found) => Err(ParseError::UnexpectedChar {
                input: self.owned_input(),
                at: self.pos,
                found,
                expected: message,
            }),
            None => Err(ParseError::UnexpectedEnd {
                input: self.owned_input(),
                at: self.pos,
                expected: message,
            }),
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

fn is_int_token(token: &str) -> bool {
    let Some(body) = token.strip_prefix('-').or(Some(token)) else {
        return false;
    };
    !body.is_empty() && body.chars().all(|ch| ch.is_ascii_digit())
}

fn is_float_token(token: &str) -> bool {
    let token = token.strip_prefix('-').unwrap_or(token);
    let Some((whole, fraction)) = token.split_once('.') else {
        return false;
    };
    !whole.is_empty()
        && !fraction.is_empty()
        && whole.chars().all(|ch| ch.is_ascii_digit())
        && fraction.chars().all(|ch| ch.is_ascii_digit())
}

/// Validate the reserved `on_error` option, returning a message when it is malformed.
///
/// Shape: `on_error=(<stage>=<policy>,…)` where `<stage>` is `load`/`parse`/`validate` and
/// `<policy>` is `skip`/`fail` (case-insensitive). Absent option → `Ok`.
fn validate_on_error(options: &Options) -> Option<String> {
    let value = options.get("on_error")?;
    let OptionValue::Map(map) = value else {
        return Some(format!(
            "`on_error` must be a map like `(load=skip)`, found {}",
            value.type_name()
        ));
    };
    for (stage, policy) in map.iter() {
        if !matches!(stage, "load" | "parse" | "validate") {
            return Some(format!(
                "unknown `on_error` stage `{stage}`; expected load, parse, or validate"
            ));
        }
        match policy {
            OptionValue::String(text)
                if text.eq_ignore_ascii_case("skip") || text.eq_ignore_ascii_case("fail") => {}
            _ => {
                return Some(format!(
                    "`on_error` policy for `{stage}` must be `skip` or `fail`, found `{policy}`"
                ));
            }
        }
    }
    None
}
