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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No source identifier (empty input or invalid start).
    MissingSource { input: String, at: usize },
    /// Input ended before a required token.
    UnexpectedEnd {
        input: String,
        at: usize,
        expected: &'static str,
    },
    /// Unexpected character at the current position.
    UnexpectedChar {
        input: String,
        at: usize,
        found: char,
        expected: &'static str,
    },
    /// Option or map key is not a valid identifier.
    InvalidIdentifier {
        input: String,
        at: usize,
        found: String,
    },
    /// Option or map key is empty.
    EmptyKey { input: String, at: usize },
    /// Option value is empty; use `""` for an empty string.
    EmptyValue { input: String, at: usize },
    /// Invalid escape sequence inside a quoted string.
    InvalidEscape { input: String, at: usize },
    /// Quoted string has no closing `"`.
    UnclosedString { input: String, at: usize },
    /// List has no closing `]`.
    UnclosedList { input: String, at: usize },
    /// Map or options block has no closing `)`.
    UnclosedMap { input: String, at: usize },
    /// Comma with no following entry.
    TrailingComma { input: String, at: usize },
    /// Token looks like a number but is not valid.
    InvalidNumber {
        input: String,
        at: usize,
        found: String,
    },
    /// Non-empty input after a complete configuration source.
    TrailingInput {
        input: String,
        at: usize,
        rest: String,
    },
    /// Skip marker `?` appears before `(...)` options (`source?(...)` is invalid).
    SkipMarkerBeforeOptions { input: String, at: usize },
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (input, at, message) = match self {
            Self::MissingSource { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source is required".to_string(),
            ),
            Self::UnexpectedEnd { input, at, expected, .. } => (
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
            Self::InvalidIdentifier { input, at, found, .. } => (
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
            Self::InvalidNumber { input, at, found, .. } => (
                input.as_str(),
                *at,
                format!("configuration source: invalid number `{found}`"),
            ),
            Self::TrailingInput { input, at, rest, .. } => (
                input.as_str(),
                *at,
                format!("configuration source: unexpected trailing input `{rest}`"),
            ),
            Self::SkipMarkerBeforeOptions { input, at, .. } => (
                input.as_str(),
                *at,
                "configuration source: skip marker `?` must come after options `(...)`; use `source(...)?` not `source?(...)`"
                    .to_string(),
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

/// Parse a configuration source string.
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
        if self.ignore_errors() {
            write!(f, "?")?;
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
        if self.peek() == Some('?') && self.input[self.pos..].starts_with("?(") {
            return Err(ParseError::SkipMarkerBeforeOptions {
                input: self.owned_input(),
                at: self.pos,
            });
        }
        let options = if self.peek() == Some('(') {
            self.parse_options_block()?
        } else {
            Options::default()
        };
        let ignore_errors = if self.peek() == Some('?') {
            self.bump();
            true
        } else {
            false
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
        Ok(Source {
            source,
            options,
            resource,
            ignore_errors,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OptionValue;

    fn parsed(input: &str) -> Source {
        parse(input).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn parses_documented_examples() {
        let env = parsed("env");
        assert_eq!(env.source(), "env");
        assert!(env.options().is_empty());
        assert_eq!(env.resource(), "");
        assert!(!env.ignore_errors());
        assert!(!env.resource_colon());

        let env_opts = parsed("env(prefix=APP_)");
        assert_eq!(
            env_opts.options().get("prefix"),
            Some(&OptionValue::String("APP_".into()))
        );

        let file = parsed("file:/x/y/z");
        assert_eq!(file.resource(), "/x/y/z");
        assert!(!file.ignore_errors());

        let file_skip = parsed("file?:.env");
        assert!(file_skip.ignore_errors());
        assert_eq!(file_skip.resource(), ".env");

        let http = parsed(
            r#"http(headers=(Authorization="TOKEN"),timeout=3s)?:https://domain.tld/my/config.yml"#,
        );
        assert_eq!(http.source(), "http");
        assert!(http.ignore_errors());
        assert_eq!(http.resource(), "https://domain.tld/my/config.yml");
        assert_eq!(
            http.options().get("timeout"),
            Some(&OptionValue::String("3s".into()))
        );
    }

    #[test]
    fn round_trips_examples() {
        for input in [
            "env",
            "env(prefix=APP_)",
            "file:/x/y/z",
            "file?:.env",
            "file?",
            "env:",
        ] {
            let source = parsed(input);
            assert_eq!(source.to_string(), input, "round-trip failed for `{input}`");
        }

        let http = parsed(
            r#"http(headers=(Authorization="TOKEN"),timeout=3s)?:https://domain.tld/my/config.yml"#,
        );
        assert_eq!(parsed(&http.to_string()), http);
    }

    #[test]
    fn parses_bool_case_insensitive() {
        let source = parsed("env(on=TRUE,off=false)");
        assert_eq!(source.options().get("on"), Some(&OptionValue::Bool(true)));
        assert_eq!(source.options().get("off"), Some(&OptionValue::Bool(false)));
    }

    #[test]
    fn rejects_question_mark_before_options() {
        let error = parse(r#"env?(kv=salam):oops"#).unwrap_err();
        assert!(matches!(error, ParseError::SkipMarkerBeforeOptions { .. }));
        assert!(error.to_string().contains("configuration source:"));
    }

    #[test]
    fn parses_complex_options_before_skip_marker() {
        let source = parsed(r#"env(kv=salam,h=(o=b,z=[1,2,3.14,""]))?:oops"#);
        assert!(source.ignore_errors());
        assert_eq!(source.resource(), "oops");
        assert_eq!(
            source.options().get("kv"),
            Some(&OptionValue::String("salam".into()))
        );
    }

    #[test]
    fn rejects_invalid_forms() {
        assert!(matches!(parse(""), Err(ParseError::MissingSource { .. })));
        assert!(matches!(
            parse("env(a=)"),
            Err(ParseError::EmptyValue { .. })
        ));
        assert!(matches!(
            parse("env(a=1,)"),
            Err(ParseError::TrailingComma { .. })
        ));
        assert!(matches!(
            parse("env(a=.5)"),
            Err(ParseError::InvalidNumber { .. })
        ));
        assert!(matches!(
            parse("env(a=+5)"),
            Err(ParseError::UnexpectedChar { .. })
        ));
        assert!(matches!(
            parse("env()oops"),
            Err(ParseError::TrailingInput { .. })
        ));
    }

    #[test]
    fn parse_error_alternate_includes_caret() {
        let error = parse("env(prefix=)").unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("column"));
        assert!(message.contains('^'));
        assert!(message.contains('\n'));
    }

    #[test]
    fn parse_error_default_is_single_line() {
        let error = parse("env(prefix=)").unwrap_err();
        let message = error.to_string();
        assert!(!message.contains('^'));
        assert!(!message.contains('\n'));
    }
}
